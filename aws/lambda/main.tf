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

resource "aws_apigatewayv2_api" "api" {
  name                         = local.resource_name
  protocol_type                = "HTTP"
  disable_execute_api_endpoint = true

  dynamic "cors_configuration" {
    for_each = length(var.cors_allow_origins) == 0 ? [] : [1]
    content {
      allow_origins = var.cors_allow_origins
      allow_methods = ["GET", "POST"]
      allow_headers = ["content-type"]
      max_age       = 3600
    }
  }
}

resource "aws_apigatewayv2_integration" "lambda" {
  api_id = aws_apigatewayv2_api.api.id

  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.validator.invoke_arn
  payload_format_version = "2.0"
  timeout_milliseconds   = var.lambda_timeout_seconds * 1000
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.api.id
  route_key = "$default"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

resource "aws_apigatewayv2_stage" "default" {
  api_id      = aws_apigatewayv2_api.api.id
  name        = "$default"
  auto_deploy = true
}

resource "aws_lambda_permission" "api_gateway" {
  statement_id  = "AllowExecutionFromApiGateway"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.validator.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.api.execution_arn}/*"
}

resource "aws_acm_certificate" "api" {
  domain_name       = var.api_domain_name
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_apigatewayv2_domain_name" "api" {
  domain_name = var.api_domain_name

  domain_name_configuration {
    certificate_arn = aws_acm_certificate.api.arn
    endpoint_type   = "REGIONAL"
    security_policy = "TLS_1_2"
  }
}

resource "aws_apigatewayv2_api_mapping" "api" {
  api_id      = aws_apigatewayv2_api.api.id
  domain_name = aws_apigatewayv2_domain_name.api.id
  stage       = aws_apigatewayv2_stage.default.id
}
