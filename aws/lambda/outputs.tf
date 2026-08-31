output "api_endpoint" {
  description = "Base URL of the HTTP API."
  value       = aws_apigatewayv2_api.validator.api_endpoint
}

output "classification_endpoint" {
  description = "Full classification endpoint."
  value       = "${aws_apigatewayv2_api.validator.api_endpoint}/v1/email-domain-classifications"
}

output "lambda_function_name" {
  description = "Deployed Lambda function name."
  value       = aws_lambda_function.validator.function_name
}

