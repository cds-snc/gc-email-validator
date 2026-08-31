# Terragrunt units

`root.hcl` generates the AWS provider and encrypted, lock-enabled S3 backend
shared by the deployable units. Terragrunt creates that bucket when an operator
runs `terragrunt backend bootstrap` locally; CI requires the backend to exist
and is not permitted to bootstrap it.

`lambda/terragrunt.hcl` deploys the reusable Terraform module in `aws/lambda`
and points it at the ZIP produced by `cargo lambda build`.

The GitHub OIDC provider and deployment role are provisioned externally. This
repository only consumes that role through the `AWS_ROLE_ARN` GitHub
environment variable.

Terragrunt reads these environment variables:

- `TG_AWS_ACCOUNT_ID`
- `TG_AWS_REGION` (default `ca-central-1`)
- `TG_ENVIRONMENT` (default `production`)
- `TG_STATE_BUCKET`
- `TG_LAMBDA_ZIP_PATH` (default `build/lambda.zip`)
- `TG_API_DOMAIN_NAME` (default `validate-email.cdssandbox.xyz`)

To establish a new backend, export the account ID, then run:

```shell
cd terragrunt/lambda
terragrunt backend bootstrap
```

Backend bootstrap is a local, privileged operation. The external deployment
role must be granted access to this state bucket before CI deployment is
enabled.

The application apply creates the Regional HTTP API and its custom-domain
mapping using the validated ACM certificate in `ca-central-1`. The
`custom_domain_dns_target` output is the CNAME target for the public hostname.
