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
}

