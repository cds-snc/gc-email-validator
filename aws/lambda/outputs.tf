output "api_endpoint" {
  description = "Base API custom-domain endpoint."
  value       = "https://${var.api_domain_name}"
}

output "classification_endpoint" {
  description = "Full classification endpoint."
  value       = "https://${var.api_domain_name}/v1/email-domain-classifications"
}

output "lambda_function_name" {
  description = "Deployed Lambda function name."
  value       = aws_lambda_function.validator.function_name
}

output "certificate_arn" {
  description = "ARN of the ACM certificate requested in the deployment region."
  value       = aws_acm_certificate.api.arn
}

output "certificate_status" {
  description = "Current ACM certificate status."
  value       = aws_acm_certificate.api.status
}

output "certificate_dns_validation_records" {
  description = "CNAME records to create at the external DNS provider to validate the ACM certificate."
  value = {
    for option in aws_acm_certificate.api.domain_validation_options : option.domain_name => {
      name  = option.resource_record_name
      type  = option.resource_record_type
      value = option.resource_record_value
    }
  }
}

output "custom_domain_dns_target" {
  description = "CNAME target for the public API hostname."
  value = {
    name  = var.api_domain_name
    type  = "CNAME"
    value = aws_apigatewayv2_domain_name.api.domain_name_configuration[0].target_domain_name
  }
}
