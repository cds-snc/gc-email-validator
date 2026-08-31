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
- `TG_ENABLE_CUSTOM_DOMAIN` (default `false`; set to `true` only after ACM
  validation completes)

To establish a new backend, export the account ID, then run:

```shell
cd terragrunt/lambda
terragrunt backend bootstrap
```

Backend bootstrap is a local, privileged operation. The external deployment
role must be granted access to this state bucket before CI deployment is
enabled.

The first application apply creates the Regional HTTP API and requests its ACM
certificate in `ca-central-1`. Read the `certificate_dns_validation_records`
output and create the reported CNAME with the external DNS provider. After ACM
reports the certificate as `ISSUED`, set `TG_ENABLE_CUSTOM_DOMAIN=true` and
apply again. The second apply creates the API Gateway custom-domain mapping and
outputs `custom_domain_dns_target`, which is the CNAME target for the public
hostname.
