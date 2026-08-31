variable "account_id" {
  description = "AWS account ID allowed by the generated Terragrunt provider."
  type        = string
  default     = "000000000000"

  validation {
    condition     = can(regex("^[0-9]{12}$", var.account_id))
    error_message = "account_id must contain exactly 12 digits."
  }
}

variable "aws_region" {
  description = "AWS region for all service resources."
  type        = string
  default     = "ca-central-1"
}

variable "service_name" {
  description = "Stable service name used in AWS resource names."
  type        = string
  default     = "gc-email-validator"

  validation {
    condition     = can(regex("^[a-z0-9-]{3,40}$", var.service_name))
    error_message = "service_name must contain 3-40 lowercase letters, numbers, or hyphens."
  }
}

variable "environment" {
  description = "Deployment environment name."
  type        = string
  default     = "production"

  validation {
    condition     = can(regex("^[a-z0-9-]{2,20}$", var.environment))
    error_message = "environment must contain 2-20 lowercase letters, numbers, or hyphens."
  }
}

variable "lambda_zip_path" {
  description = "Absolute or module-relative path to the cargo-lambda zip archive."
  type        = string
  default     = "../../../build/lambda.zip"
}

variable "lambda_memory_mb" {
  description = "Lambda memory allocation. 128 MB is sufficient for this in-memory lookup."
  type        = number
  default     = 128

  validation {
    condition     = var.lambda_memory_mb >= 128 && var.lambda_memory_mb <= 10240
    error_message = "lambda_memory_mb must be between 128 and 10240."
  }
}

variable "lambda_timeout_seconds" {
  description = "Maximum Lambda execution time."
  type        = number
  default     = 3

  validation {
    condition     = var.lambda_timeout_seconds >= 1 && var.lambda_timeout_seconds <= 30
    error_message = "lambda_timeout_seconds must be between 1 and 30."
  }
}

variable "reserved_concurrency" {
  description = "Reserved Lambda concurrency. Use -1 for unreserved account concurrency."
  type        = number
  default     = -1
}

variable "log_retention_days" {
  description = "CloudWatch log retention."
  type        = number
  default     = 30
}

variable "api_throttle_rate" {
  description = "Steady-state requests per second allowed by the API stage."
  type        = number
  default     = 100
}

variable "api_throttle_burst" {
  description = "Short API request burst allowance."
  type        = number
  default     = 200
}

variable "cors_allow_origins" {
  description = "Browser origins allowed to call the API. Leave empty for server-to-server only."
  type        = list(string)
  default     = []
}
