terraform {
  source = "../../aws//lambda"
}

include "root" {
  path = find_in_parent_folders("root.hcl")
}

inputs = {
  # Keep the Lambda deployment as a small ZIP built by cargo-lambda. No ECR
  # repository or container-image lifecycle is required for this service.
  lambda_zip_path = get_env(
    "TG_LAMBDA_ZIP_PATH",
    abspath("${get_terragrunt_dir()}/../../build/lambda.zip")
  )

  api_domain_name      = get_env("TG_API_DOMAIN_NAME", "validate-email.cdssandbox.xyz")
  enable_custom_domain = get_env("TG_ENABLE_CUSTOM_DOMAIN", "false") == "true"
}
