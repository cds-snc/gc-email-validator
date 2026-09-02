use std::process::{Command, Output};

use gc_email_validator::classify_email;
use serde_json::Value;

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gc-email-validator"))
        .args(arguments)
        .output()
        .expect("CLI should run")
}

#[test]
fn outputs_the_same_classification_json_as_the_library() {
    let output = run(&["person@statcan.gc.ca"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected = serde_json::to_value(classify_email("person@statcan.gc.ca").unwrap()).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn a_non_government_classification_is_successful() {
    let output = run(&["attacker@foo.statcan.gc.ca"]);
    assert!(output.status.success());

    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["isGovernmentOfCanada"], false);
}

#[test]
fn invalid_email_is_json_on_stderr_with_a_nonzero_exit() {
    let output = run(&["not-an-email"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let response: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(response["error"]["code"], "invalidEmail");
}

#[test]
fn pretty_prints_and_reports_version() {
    let pretty = run(&["--pretty", "person@canada.ca"]);
    assert!(pretty.status.success());
    assert!(String::from_utf8(pretty.stdout).unwrap().contains("\n  \""));

    let version = run(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        concat!("gc-email-validator ", env!("CARGO_PKG_VERSION"))
    );
}
