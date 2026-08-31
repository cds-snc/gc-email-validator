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

- `isGovernmentOfCanada` is true only when an exact domain or label-boundary
  parent domain is present in the reviewed, compiled list.
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

The compiler accepts an alias only when its organization is active and appears
in the concordance. Domains under `gc.ca` and `canada.ca` are eligible by
default. Any external domain must appear both in the upstream alias data and in
the local policy for the same organization ID. This avoids treating every
organization website as an email domain.

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

For local Lambda emulation:

```shell
cargo lambda watch --package gc-email-validator
cargo lambda invoke gc-email-validator --data-file events/classify.json
```

## CI/CD

The workflows follow least-privilege GitHub token and immutable-action-pin
conventions:

- `CI` runs formatting, Clippy, tests, deterministic generation checks, and
  Terraform module and Terragrunt configuration validation on pull requests and
  `main`.
- `Refresh domain data` runs daily and opens or updates a pull request only when
  the compiled dataset changes. Reviewers can inspect the domain and provenance
  diff before merging.
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
Lambda, IAM-role, and CloudWatch Logs permissions, it needs the Lambda Function
URL configuration actions (`CreateFunctionUrlConfig`, `GetFunctionUrlConfig`,
`UpdateFunctionUrlConfig`, `DeleteFunctionUrlConfig`) and permission-policy
actions (`AddPermission`, `RemovePermission`) on this service's Lambda
functions. Put its ARN, the account ID, state bucket name, and region into the
protected GitHub environment described above. CI and the deployment workflow
never create identity infrastructure or run backend bootstrap.

The reusable application module in [`aws/lambda`](aws/lambda) creates one ARM64
Lambda with a public Function URL, one bounded-retention log group, and
least-privilege runtime IAM. [`terragrunt/root.hcl`](terragrunt/root.hcl) generates the
account-specific AWS provider and S3 backend, while
[`terragrunt/lambda/terragrunt.hcl`](terragrunt/lambda/terragrunt.hcl) connects
the module to the locally built Lambda ZIP. Its default 128 MB memory, no
database, no NAT gateway, and no API Gateway make the design inexpensive at low
traffic and horizontally scalable at Lambda limits. The Function URL uses
`NONE` authorization and is therefore public. Set reserved Lambda concurrency
when an account-level concurrency cap is required; Function URLs do not provide
API Gateway-style rate throttling, usage plans, WAF integration, or custom
domains.

After the bootstrap exists, an authenticated local plan can be run with:

```shell
export TG_AWS_ACCOUNT_ID="123456789012"
export TG_STATE_BUCKET="gc-email-validator-123456789012-ca-central-1-tfstate"
make deploy-package
cd terragrunt/lambda
terragrunt run plan
```

## Privacy and logging

The Function URL does not add a separate API access-log group. The Rust handler
does not log request payloads or email addresses. Keep that property when adding
observability.

## Limitations

- Upstream aliases are curated evidence, not a formal inventory of every email
  domain used by every federal organization.
- The active flag is organization-level; an old alias can remain upstream.
  Suspicious or retired domains should be placed in `excluded_domains` while an
  upstream correction is pursued.
- Crown corporations and other entities using domains outside `gc.ca` and
  `canada.ca` require an explicit local policy review.
