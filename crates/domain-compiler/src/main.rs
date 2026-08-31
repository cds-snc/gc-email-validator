use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env, fs,
    path::{Path, PathBuf},
};

use csv::StringRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

const CONCORDANCE_FILE: &str = "gc_concordance.csv";
const ORG_INFO_FILE: &str = "gc_org_info.csv";
const ALIASES_FILE: &str = "gc_org_aliases.csv";
const METADATA_FILE: &str = "metadata.json";

#[derive(Debug, Error)]
enum CompilerError {
    #[error("missing required CSV column {column:?} in {path}")]
    MissingColumn { column: String, path: String },
    #[error("invalid integer {value:?} in column {column} of {path}")]
    InvalidInteger {
        value: String,
        column: String,
        path: String,
    },
    #[error("external policy domain {domain} was not found in aliases for gc_orgID {gc_org_id}")]
    UnverifiedExternalDomain { domain: String, gc_org_id: u32 },
    #[error("domain {domain} maps to both gc_orgID {first} and {second}")]
    ConflictingOrganizations {
        domain: String,
        first: u32,
        second: u32,
    },
    #[error("unknown argument {0}")]
    UnknownArgument(String),
    #[error("argument {0} requires a value")]
    MissingArgumentValue(String),
}

#[derive(Debug)]
struct Arguments {
    upstream_dir: PathBuf,
    policy: PathBuf,
    output: PathBuf,
    manifest: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    namespace_roots: Vec<String>,
    #[serde(default = "default_true")]
    recognize_namespace_roots: bool,
    #[serde(default)]
    excluded_domains: Vec<String>,
    #[serde(default)]
    external_domains: Vec<ExternalDomain>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalDomain {
    domain: String,
    gc_org_id: u32,
    #[serde(default = "default_true")]
    include_subdomains: bool,
}

#[derive(Debug)]
struct Organization {
    gc_org_id: u32,
    name_en: String,
    name_fr: String,
}

#[derive(Debug)]
struct Rule {
    domain: String,
    include_subdomains: bool,
    organization: Option<Organization>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceMetadata {
    source_repository: String,
    source_commit: String,
    official_dataset_url: String,
    concordance_url: String,
    org_info_url: String,
    aliases_url: String,
    concordance_sha256: String,
    org_info_sha256: String,
    aliases_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest<'a> {
    dataset_version: &'a str,
    rule_count: usize,
    namespace_roots: &'a [String],
    source: &'a SourceMetadata,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = parse_arguments()?;
    let policy_bytes = fs::read(&arguments.policy)?;
    let policy: Policy = serde_yaml::from_slice(&policy_bytes)?;

    let concordance_path = arguments.upstream_dir.join(CONCORDANCE_FILE);
    let org_info_path = arguments.upstream_dir.join(ORG_INFO_FILE);
    let aliases_path = arguments.upstream_dir.join(ALIASES_FILE);
    let metadata_path = arguments.upstream_dir.join(METADATA_FILE);

    let concordance_bytes = fs::read(&concordance_path)?;
    let org_info_bytes = fs::read(&org_info_path)?;
    let aliases_bytes = fs::read(&aliases_path)?;
    let metadata_bytes = fs::read(&metadata_path)?;
    let metadata: SourceMetadata = serde_json::from_slice(&metadata_bytes)?;

    verify_source_hash(
        &concordance_bytes,
        &metadata.concordance_sha256,
        CONCORDANCE_FILE,
    )?;
    verify_source_hash(&org_info_bytes, &metadata.org_info_sha256, ORG_INFO_FILE)?;
    verify_source_hash(&aliases_bytes, &metadata.aliases_sha256, ALIASES_FILE)?;

    let concordance_ids = load_concordance_ids(&concordance_path)?;
    let organizations = load_active_organizations(&org_info_path)?;
    let mut roots = normalize_policy_domains(&policy.namespace_roots)?;
    roots.sort();
    roots.dedup();
    let excluded: BTreeSet<String> = normalize_policy_domains(&policy.excluded_domains)?
        .into_iter()
        .collect();
    let external = normalize_external_domains(&policy.external_domains)?;

    let mut rules = BTreeMap::<String, Rule>::new();
    if policy.recognize_namespace_roots {
        for root in &roots {
            if !excluded.contains(root) {
                rules.insert(
                    root.clone(),
                    Rule {
                        domain: root.clone(),
                        include_subdomains: false,
                        organization: None,
                    },
                );
            }
        }
    }

    let matched_external = load_alias_rules(
        &aliases_path,
        &organizations,
        &concordance_ids,
        &roots,
        &external,
        &excluded,
        &mut rules,
    )?;

    for (domain, specification) in &external {
        if !matched_external.contains(domain) {
            return Err(CompilerError::UnverifiedExternalDomain {
                domain: domain.clone(),
                gc_org_id: specification.gc_org_id,
            }
            .into());
        }
    }

    let dataset_version = dataset_version(&[
        (&concordance_path, &concordance_bytes),
        (&org_info_path, &org_info_bytes),
        (&aliases_path, &aliases_bytes),
        (&metadata_path, &metadata_bytes),
        (&arguments.policy, &policy_bytes),
    ]);
    let generated = render_generated(&dataset_version, &roots, rules.values());
    write_if_changed(&arguments.output, generated.as_bytes())?;

    let manifest = Manifest {
        dataset_version: &dataset_version,
        rule_count: rules.len(),
        namespace_roots: &roots,
        source: &metadata,
    };
    let mut manifest_json = serde_json::to_vec_pretty(&manifest)?;
    manifest_json.push(b'\n');
    write_if_changed(&arguments.manifest, &manifest_json)?;

    println!(
        "compiled {} domains from source commit {} as {}",
        rules.len(),
        metadata.source_commit,
        dataset_version
    );
    Ok(())
}

fn parse_arguments() -> Result<Arguments, CompilerError> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut arguments = Arguments {
        upstream_dir: project_root.join("data/upstream"),
        policy: project_root.join("data/domain-policy.yaml"),
        output: project_root.join("crates/api/src/generated_domains.rs"),
        manifest: project_root.join("data/manifest.json"),
    };

    let mut values = env::args().skip(1);
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .ok_or_else(|| CompilerError::MissingArgumentValue(flag.clone()))?;
        match flag.as_str() {
            "--upstream-dir" => arguments.upstream_dir = value.into(),
            "--policy" => arguments.policy = value.into(),
            "--output" => arguments.output = value.into(),
            "--manifest" => arguments.manifest = value.into(),
            _ => return Err(CompilerError::UnknownArgument(flag)),
        }
    }
    Ok(arguments)
}

fn default_true() -> bool {
    true
}

fn load_concordance_ids(path: &Path) -> Result<BTreeSet<u32>, Box<dyn std::error::Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = normalized_headers(reader.headers()?);
    let id_index = column(&headers, "gc_orgID", path)?;
    let mut ids = BTreeSet::new();

    for row in reader.records() {
        let row = row?;
        ids.insert(parse_u32(&row, id_index, "gc_orgID", path)?);
    }
    Ok(ids)
}

fn load_active_organizations(
    path: &Path,
) -> Result<HashMap<u32, (String, String)>, Box<dyn std::error::Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = normalized_headers(reader.headers()?);
    let id_index = column(&headers, "gc_orgID", path)?;
    let status_index = column(&headers, "status_statut", path)?;
    let en_index = column(&headers, "preferred_name", path)?;
    let fr_index = column(&headers, "nom_préféré", path)?;
    let mut organizations = HashMap::new();

    for row in reader.records() {
        let row = row?;
        if row.get(status_index).unwrap_or_default().trim() != "a" {
            continue;
        }
        let id = parse_u32(&row, id_index, "gc_orgID", path)?;
        organizations.insert(
            id,
            (
                row.get(en_index).unwrap_or_default().trim().to_owned(),
                row.get(fr_index).unwrap_or_default().trim().to_owned(),
            ),
        );
    }
    Ok(organizations)
}

#[allow(clippy::too_many_arguments)]
fn load_alias_rules(
    path: &Path,
    organizations: &HashMap<u32, (String, String)>,
    concordance_ids: &BTreeSet<u32>,
    roots: &[String],
    external: &BTreeMap<String, ExternalDomain>,
    excluded: &BTreeSet<String>,
    rules: &mut BTreeMap<String, Rule>,
) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = normalized_headers(reader.headers()?);
    let name_index = column(&headers, "name", path)?;
    let id_index = column(&headers, "gc_orgID", path)?;
    let mut matched_external = BTreeSet::new();

    for row in reader.records() {
        let row = row?;
        let Some(domain) = row.get(name_index).and_then(normalize_alias_domain) else {
            continue;
        };
        if excluded.contains(&domain) {
            continue;
        }

        let gc_org_id = parse_u32(&row, id_index, "gc_orgID", path)?;
        let Some((name_en, name_fr)) = organizations.get(&gc_org_id) else {
            continue;
        };
        if !concordance_ids.contains(&gc_org_id) {
            continue;
        }

        let in_namespace = roots.iter().any(|root| is_domain_or_child(&domain, root));
        let external_specification = external.get(&domain);
        if !in_namespace && external_specification.is_none() {
            continue;
        }
        if let Some(specification) = external_specification {
            if specification.gc_org_id != gc_org_id {
                continue;
            }
            matched_external.insert(domain.clone());
        }

        let include_subdomains = external_specification
            .map(|specification| specification.include_subdomains)
            .unwrap_or(true);
        insert_rule(
            rules,
            Rule {
                domain,
                include_subdomains,
                organization: Some(Organization {
                    gc_org_id,
                    name_en: name_en.clone(),
                    name_fr: name_fr.clone(),
                }),
            },
        )?;
    }
    Ok(matched_external)
}

fn insert_rule(rules: &mut BTreeMap<String, Rule>, rule: Rule) -> Result<(), CompilerError> {
    if let Some(existing) = rules.get(&rule.domain) {
        match (&existing.organization, &rule.organization) {
            (None, _) => return Ok(()),
            (Some(first), Some(second)) if first.gc_org_id != second.gc_org_id => {
                return Err(CompilerError::ConflictingOrganizations {
                    domain: rule.domain,
                    first: first.gc_org_id,
                    second: second.gc_org_id,
                });
            }
            _ => return Ok(()),
        }
    }
    rules.insert(rule.domain.clone(), rule);
    Ok(())
}

fn normalize_policy_domains(domains: &[String]) -> Result<Vec<String>, CompilerError> {
    domains
        .iter()
        .map(|domain| {
            normalize_domain(domain).ok_or_else(|| CompilerError::MissingColumn {
                column: format!("valid domain value ({domain})"),
                path: "data/domain-policy.yaml".to_owned(),
            })
        })
        .collect()
}

fn normalize_external_domains(
    domains: &[ExternalDomain],
) -> Result<BTreeMap<String, ExternalDomain>, CompilerError> {
    let mut normalized = BTreeMap::new();
    for domain in domains {
        let name =
            normalize_domain(&domain.domain).ok_or_else(|| CompilerError::MissingColumn {
                column: format!("valid domain value ({})", domain.domain),
                path: "data/domain-policy.yaml".to_owned(),
            })?;
        normalized.insert(
            name.clone(),
            ExternalDomain {
                domain: name,
                gc_org_id: domain.gc_org_id,
                include_subdomains: domain.include_subdomains,
            },
        );
    }
    Ok(normalized)
}

fn normalize_alias_domain(value: &str) -> Option<String> {
    let mut candidate = value.trim();
    if let Some((_, domain)) = candidate.rsplit_once('@') {
        candidate = domain;
    }

    let owned_host;
    if candidate.contains("://") || candidate.contains('/') {
        let url = if candidate.contains("://") {
            Url::parse(candidate).ok()?
        } else {
            Url::parse(&format!("https://{candidate}")).ok()?
        };
        owned_host = url.host_str()?.to_owned();
        candidate = &owned_host;
    }

    normalize_domain(candidate.trim_start_matches("www."))
}

fn normalize_domain(value: &str) -> Option<String> {
    let ascii = idna::domain_to_ascii(value.trim().trim_end_matches('.')).ok()?;
    let domain = ascii.to_ascii_lowercase();
    if domain.len() > 253 || !domain.contains('.') {
        return None;
    }
    if domain.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return None;
    }
    Some(domain)
}

fn is_domain_or_child(domain: &str, root: &str) -> bool {
    domain == root
        || domain
            .strip_suffix(root)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn normalized_headers(headers: &StringRecord) -> Vec<String> {
    headers
        .iter()
        .map(|header| header.trim_start_matches('\u{feff}').trim().to_owned())
        .collect()
}

fn column(headers: &[String], name: &str, path: &Path) -> Result<usize, CompilerError> {
    headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| CompilerError::MissingColumn {
            column: name.to_owned(),
            path: path.display().to_string(),
        })
}

fn parse_u32(
    row: &StringRecord,
    index: usize,
    column_name: &str,
    path: &Path,
) -> Result<u32, CompilerError> {
    let value = row.get(index).unwrap_or_default().trim();
    value.parse().map_err(|_| CompilerError::InvalidInteger {
        value: value.to_owned(),
        column: column_name.to_owned(),
        path: path.display().to_string(),
    })
}

fn verify_source_hash(
    bytes: &[u8],
    expected: &str,
    file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(format!("{file} SHA-256 mismatch: expected {expected}, got {actual}").into());
    }
    Ok(())
}

fn dataset_version(inputs: &[(&Path, &[u8])]) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in inputs {
        hasher.update(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_bytes(),
        );
        hasher.update([0]);
        hasher.update(bytes);
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn render_generated<'a>(
    version: &str,
    roots: &[String],
    rules: impl Iterator<Item = &'a Rule>,
) -> String {
    let mut output = String::from(
        "// @generated by domain-compiler. Do not edit by hand.\n\nuse crate::DomainRule;\n\n",
    );
    output.push_str(&format!(
        "pub const DATASET_VERSION: &str = {version:?};\n\n"
    ));
    output.push_str("pub static NAMESPACE_ROOTS: &[&str] = &[\n");
    for root in roots {
        output.push_str(&format!("    {root:?},\n"));
    }
    output.push_str("];\n\npub static RULES: &[DomainRule] = &[\n");
    for rule in rules {
        output.push_str("    DomainRule {\n");
        output.push_str(&format!("        domain: {:?},\n", rule.domain));
        output.push_str(&format!(
            "        include_subdomains: {},\n",
            rule.include_subdomains
        ));
        if let Some(organization) = &rule.organization {
            output.push_str(&format!(
                "        gc_org_id: Some({}),\n        organization_en: Some({:?}),\n        organization_fr: Some({:?}),\n",
                organization.gc_org_id, organization.name_en, organization.name_fr
            ));
        } else {
            output.push_str(
                "        gc_org_id: None,\n        organization_en: None,\n        organization_fr: None,\n",
            );
        }
        output.push_str("    },\n");
    }
    output.push_str("];\n");
    output
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
    if fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_domains_from_alias_formats() {
        assert_eq!(
            normalize_alias_domain("Analyst@MAIL.StatCan.gc.ca"),
            Some("mail.statcan.gc.ca".to_owned())
        );
        assert_eq!(
            normalize_alias_domain("https://www.inspection.gc.ca/about"),
            Some("inspection.gc.ca".to_owned())
        );
        assert_eq!(normalize_alias_domain("not an internet domain"), None);
    }

    #[test]
    fn domain_boundaries_are_respected() {
        assert!(is_domain_or_child("statcan.gc.ca", "gc.ca"));
        assert!(is_domain_or_child("mail.statcan.gc.ca", "gc.ca"));
        assert!(!is_domain_or_child("evilgc.ca", "gc.ca"));
        assert!(!is_domain_or_child("gc.ca.example", "gc.ca"));
    }
}
