# GC email validator

A small, low-latency AWS Lambda API that classifies the domain portion of an
email address against a reviewed Government of Canada domain dataset. The
request path performs no DNS lookups, database reads, or network calls: the
domain table is generated at build time and compiled into the Rust binary.

## API

`POST /v1/email-domain-classifications`

```json
{
  "email": "person@statcan.gc.ca"
}
```

Example response:

```json
{
  "isGovernmentOfCanada": true,
  "isGovernmentControlledNamespace": true,
  "domain": "statcan.gc.ca",
  "matchedDomain": "statcan.gc.ca",
  "matchType": "recognizedDomain",
  "organization": {
    "gcOrgId": 2505,
    "nameEn": "Statistics Canada",
    "nameFr": "Statistique Canada"
  },
  "datasetVersion": "sha256:..."
}
```

The service deliberately returns two related values:

- `isGovernmentOfCanada` is true only when the exact domain is present in the
  reviewed, compiled list, unless a parent domain has been explicitly approved
  to include its subdomains.
- `isGovernmentControlledNamespace` is true for any syntactically valid domain
  beneath a configured namespace such as `gc.ca` or `canada.ca`, even if that
  specific domain is not in the current email-domain list.

This is a domain classification, not proof that an address or sender exists.
The service accepts common unquoted internet email syntax, converts international
domain names to ASCII, rejects display names and ambiguous syntax, and never
returns the submitted local part.

`GET /health` returns the loaded dataset version and rule count.

## Data model and provenance

The generated list uses three inputs:

1. [Government of Canada organization concordance and metadata](https://open.canada.ca/data/en/dataset/57180b36-3428-4a7f-afe3-2161a6b44ec5) supplies the authoritative organization IDs, current/active status, and bilingual names.
2. [`gcorg-resolver`](https://github.com/cds-snc/gcorg-resolver) maintains snapshots of those two official files and supplies the curated domain-like aliases. The refresh process downloads all three CSVs from one immutable repository commit, ensuring they form an internally consistent snapshot.
3. [`data/domain-policy.yaml`](data/domain-policy.yaml) defines controlled namespace roots, exact exclusions, and explicitly reviewed domains outside those roots.

The compiler accepts an alias only when its organization appears in the
concordance and is not explicitly marked terminated (`t`) in the organization
metadata. Blank or unspecified statuses remain eligible. Domains under `gc.ca`
and `canada.ca` are eligible as exact domains by default; their unlisted child
domains are not inherited. The `canada.ca` root is recognized, while `gc.ca`
is deliberately classified only as a controlled namespace. Any external domain
must appear both in the upstream alias data and in the local policy for the
same organization ID. This avoids treating every organization website as an
email domain.

Source snapshots and their SHA-256 hashes are checked in under `data/upstream`.
[`data/manifest.json`](data/manifest.json) records the resulting version and
provenance. [`crates/api/src/generated_domains.rs`](crates/api/src/generated_domains.rs)
is deterministic generated code.

## Development

The supported development environment is the checked-in devcontainer. In VS
Code or another devcontainer-compatible editor, open this directory and choose
**Reopen in Container**. It installs pinned Rust, Terraform, Terragrunt,
cargo-lambda, Zig, `jq`, and the packaging tools.

After the container starts:

```shell
make check
make infrastructure-check
```

To replace the bootstrap dataset or refresh an existing snapshot:

```shell
make refresh
make check
```

The refresh resolves the current `gcorg-resolver` commit once, downloads all
three CSVs from that immutable commit into a temporary directory, verifies
hashes in the compiler, and installs the inputs and generated outputs only after
compilation succeeds. The original Open Government URL remains recorded as
provenance rather than serving as a separate build input.

To build the Lambda zip:

```shell
make deploy-package
```

To build and run the command-line executable locally:

```shell
make cli
./target/release/gc-email-validator person@statcan.gc.ca
```

The CLI writes the same classification JSON as the API to standard output:

```json
{
  "isGovernmentOfCanada": true,
  "isGovernmentControlledNamespace": true,
  "domain": "statcan.gc.ca",
  "matchedDomain": "statcan.gc.ca",
  "matchType": "recognizedDomain",
  "organization": {
    "gcOrgId": 2293,
    "nameEn": "Statistics Canada",
    "nameFr": "Statistique Canada"
  },
  "datasetVersion": "sha256:..."
}
```

Valid non-Government classifications also exit successfully because the
classification itself succeeded. Invalid input writes a JSON error to standard
error and exits with status 2. Use `--pretty` for formatted output.

Version tags publish static Linux binaries for x86-64 and ARM64 to
[GitHub Releases](https://github.com/cds-snc/gc-email-validator/releases),
along with `SHA256SUMS`. The archives can be unpacked into an Alpine, Debian,
Ubuntu, distroless, or scratch-based container without installing Rust or
shared runtime libraries. Each executable contains the dataset version shown
in its JSON output; upgrade the pinned release to receive data updates.

For local Lambda emulation:

```shell
cargo lambda watch --package gc-email-validator --bin gc-email-validator-lambda
cargo lambda invoke gc-email-validator-lambda --data-file events/classify.json
```

## CI/CD

The workflows follow least-privilege GitHub token and immutable-action-pin
conventions:

- `CI` runs formatting, Clippy, tests, deterministic generation checks, and
  Terraform module and Terragrunt configuration validation on pull requests and
  `main`.
- `Refresh domain data` runs daily and opens or updates a pull request only when
  the compiled dataset changes. Each changed dataset also increments the CLI
  package patch version and updates `Cargo.lock`. Reviewers can inspect the
  domain, provenance, and version diff before merging.
- `Release CLI` publishes static, checksummed Linux x86-64 and ARM64 binaries
  when a semantic version tag matching the Cargo package version is pushed.
- `Deploy` runs only after a successful `CI` run on `main`, or manually. It
  builds an ARM64 `provided.al2023` Lambda ZIP and applies the
  `terragrunt/lambda` unit through GitHub OIDC. It does not create an ECR
  repository or deploy a Lambda container image. The protected `production`
  GitHub environment can require reviewer approval.

Before enabling deployment, create a GitHub environment named `production` and
configure these environment variables:

| Variable | Example | Purpose |
| --- | --- | --- |
| `AWS_ACCOUNT_ID` | local AWS account ID | Account allow-listed by the generated provider |
| `AWS_REGION` | `ca-central-1` | AWS deployment and state region |
| `AWS_ROLE_ARN` | external provisioning output | OIDC deployment role |
| `TF_STATE_BUCKET` | Terragrunt backend name | versioned Terraform state bucket |

No AWS secrets are stored in GitHub.

## Terragrunt backend bootstrap

The remote-state bucket is bootstrapped directly from the `remote_state` block
in [`terragrunt/root.hcl`](terragrunt/root.hcl). The pinned Terragrunt CLI calls
this operation `terragrunt backend bootstrap`. It enables encryption,
versioning, native S3 state locking, public-access protection, and applies the
configured bucket tags. This command is intentionally local-only because it
establishes the backend and trust boundary that CI later consumes.

Authenticate locally with a privileged AWS role and run:

```shell
export TG_AWS_ACCOUNT_ID="$(aws sts get-caller-identity --query Account --output text)"
# Optional when using a non-default bucket name:
# export TG_STATE_BUCKET="your-globally-unique-state-bucket"

cd terragrunt/lambda
terragrunt backend bootstrap
```

The GitHub OIDC provider and deployment role are created by an external
process. That role must be allowed to read and write objects in the selected
state bucket and deploy the service resources. In addition to the existing
Lambda, IAM-role, CloudWatch Logs, and Lambda permission-policy actions, it
needs permission to manage API Gateway v2 APIs, integrations, routes, stages,
custom domains, and API mappings, plus the ACM certificate lifecycle. Put its
ARN, the account ID, state bucket name, and region into the protected GitHub
environment described above. CI and the deployment workflow never create
identity infrastructure or run backend bootstrap.

The reusable application module in [`aws/lambda`](aws/lambda) creates one ARM64
Lambda ZIP deployment, a Regional API Gateway HTTP API, an ACM certificate in
`ca-central-1`, one bounded-retention log group, and least-privilege runtime
IAM. [`terragrunt/root.hcl`](terragrunt/root.hcl) generates the account-specific
AWS provider and S3 backend, while
[`terragrunt/lambda/terragrunt.hcl`](terragrunt/lambda/terragrunt.hcl) connects
the module to the locally built Lambda ZIP. Its default 128 MB memory, no
database, and no NAT gateway keep the design inexpensive at low traffic and
horizontally scalable at Lambda limits.

### Custom-domain activation

The validated ACM certificate is attached to the Regional API Gateway custom
domain on the next apply. Afterward, read the DNS target:

```shell
cd terragrunt/lambda
terragrunt output custom_domain_dns_target
```

Create the reported CNAME for `validate-email.cdssandbox.xyz` at the DNS
provider. The default `execute-api` endpoint is disabled, so requests use only
the custom hostname.

The certificate and API Gateway endpoint are both Regional resources in
`ca-central-1`; no `us-east-1` provider or CloudFront distribution is required.

After the bootstrap exists, an authenticated local plan can be run with:

```shell
export TG_AWS_ACCOUNT_ID="123456789012"
export TG_STATE_BUCKET="gc-email-validator-123456789012-ca-central-1-tfstate"
make deploy-package
cd terragrunt/lambda
terragrunt run plan
```

## Privacy and logging

API Gateway access logging is not enabled, and the Rust handler does not log
request payloads or email addresses. Keep that property when adding
observability.

## Limitations

- Upstream aliases are curated evidence, not a formal inventory of every email
  domain used by every federal organization.
- The status is organization-level; an old alias can remain upstream even when
  its organization is not explicitly marked terminated.
  Suspicious or retired domains should be placed in `excluded_domains` while an
  upstream correction is pursued.
- Crown corporations and other entities using domains outside `gc.ca` and
  `canada.ca` require an explicit local policy review.
