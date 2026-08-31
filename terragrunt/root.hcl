locals {
  account_id   = get_env("TG_AWS_ACCOUNT_ID", "283582579564")
  environment  = get_env("TG_ENVIRONMENT", "production")
  region       = get_env("TG_AWS_REGION", "ca-central-1")
  service_name = "gc-email-validator"
  state_bucket = get_env(
    "TG_STATE_BUCKET",
    "${local.service_name}-${local.account_id}-${local.region}-tfstate"
  )
}

# Values shared by every deployable unit. Keep unit-specific settings in the
# child terragrunt.hcl file.
inputs = {
  account_id   = local.account_id
  aws_region   = local.region
  environment  = local.environment
  service_name = local.service_name
}

# Generate the provider in Terragrunt's cache so reusable Terraform modules do
# not contain environment- or account-specific provider configuration.
generate "provider" {
  path      = "provider.tf"
  if_exists = "overwrite_terragrunt"
  contents  = <<-EOF
    provider "aws" {
      region              = var.aws_region
      allowed_account_ids = [var.account_id]

      default_tags {
        tags = {
          Application = var.service_name
          Environment = var.environment
          ManagedBy   = "Terragrunt"
        }
      }
    }
  EOF
}

# Terragrunt creates and maintains the shared state bucket when an operator runs
# `terragrunt backend bootstrap` locally. Deployments require it to exist.
remote_state {
  backend = "s3"
  generate = {
    path      = "backend.tf"
    if_exists = "overwrite_terragrunt"
  }
  config = {
    bucket       = local.state_bucket
    key          = "${path_relative_to_include()}/terraform.tfstate"
    region       = local.region
    encrypt      = true
    use_lockfile = true
    s3_bucket_tags = {
      Application = local.service_name
      Environment = local.environment
      ManagedBy   = "Terragrunt"
    }
  }
}
