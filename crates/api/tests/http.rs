use gc_email_validator::handle_request;
use lambda_http::{Body, http::Request, http::StatusCode};
use serde_json::Value;

#[tokio::test]
async fn post_returns_a_classification_without_echoing_the_address() {
    let request = Request::builder()
        .method("POST")
        .uri("/v1/email-domain-classifications")
        .header("content-type", "application/json")
        .body(Body::Text(
            r#"{"email":"private.person@statcan.gc.ca"}"#.to_owned(),
        ))
        .unwrap();

    let response = handle_request(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let Body::Text(body) = response.body() else {
        panic!("expected a text response")
    };
    assert!(!body.contains("private.person"));

    let value: Value = serde_json::from_str(body).unwrap();
    assert_eq!(value["isGovernmentOfCanada"], true);
    assert_eq!(value["domain"], "statcan.gc.ca");
    assert_eq!(value["matchedDomain"], "statcan.gc.ca");
}

#[tokio::test]
async fn malformed_json_is_a_safe_client_error() {
    let request = Request::builder()
        .method("POST")
        .uri("/v1/email-domain-classifications")
        .body(Body::Text("not-json".to_owned()))
        .unwrap();

    let response = handle_request(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn health_reports_the_loaded_dataset() {
    let request = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::Empty)
        .unwrap();

    let response = handle_request(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
