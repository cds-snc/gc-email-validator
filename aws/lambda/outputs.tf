output "api_endpoint" {
  description = "Base Lambda Function URL."
  value       = aws_lambda_function_url.api.function_url
}

output "classification_endpoint" {
  description = "Full classification endpoint."
  value       = "${trimsuffix(aws_lambda_function_url.api.function_url, "/")}/v1/email-domain-classifications"
}

output "lambda_function_name" {
  description = "Deployed Lambda function name."
  value       = aws_lambda_function.validator.function_name
}
