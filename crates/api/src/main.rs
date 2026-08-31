use gc_email_validator::handle_request;
use lambda_http::{Error, run, service_fn};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handle_request)).await
}
