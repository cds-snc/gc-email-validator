locals {
  resource_name = "${var.service_name}-${var.environment}"
  lambda_zip = startswith(var.lambda_zip_path, "/") ? var.lambda_zip_path : abspath(
    "${path.module}/${var.lambda_zip_path}"
  )
}

resource "aws_cloudwatch_log_group" "lambda" {
  name              = "/aws/lambda/${local.resource_name}"
  retention_in_days = var.log_retention_days
}

resource "aws_iam_role" "lambda" {
  name = "${local.resource_name}-lambda"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Principal = {
        Service = "lambda.amazonaws.com"
      }
      Action = "sts:AssumeRole"
    }]
  })
}

resource "aws_iam_role_policy" "lambda_logs" {
  name = "cloudwatch-logs"
  role = aws_iam_role.lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Sid    = "WriteFunctionLogs"
      Effect = "Allow"
      Action = [
        "logs:CreateLogStream",
        "logs:PutLogEvents"
      ]
      Resource = "${aws_cloudwatch_log_group.lambda.arn}:*"
    }]
  })
}

resource "aws_lambda_function" "validator" {
  function_name = local.resource_name
  description   = "Classifies email domains against a reviewed Government of Canada domain dataset."
  role          = aws_iam_role.lambda.arn
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["arm64"]

  filename         = local.lambda_zip
  source_code_hash = filebase64sha256(local.lambda_zip)

  memory_size                    = var.lambda_memory_mb
  timeout                        = var.lambda_timeout_seconds
  reserved_concurrent_executions = var.reserved_concurrency

  depends_on = [
    aws_cloudwatch_log_group.lambda,
    aws_iam_role_policy.lambda_logs
  ]
}

resource "aws_lambda_function_url" "api" {
  function_name      = aws_lambda_function.validator.function_name
  authorization_type = "NONE"
  invoke_mode        = "BUFFERED"

  dynamic "cors" {
    for_each = length(var.cors_allow_origins) == 0 ? [] : [1]
    content {
      allow_origins = var.cors_allow_origins
      allow_methods = ["GET", "POST"]
      allow_headers = ["content-type"]
      max_age       = 3600
    }
  }
}
