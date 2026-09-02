use std::{env, ffi::OsString, process::ExitCode};

use gc_email_validator::classify_email;
use serde::Serialize;

const EXIT_ERROR: u8 = 1;
const EXIT_USAGE: u8 = 2;

#[derive(Debug)]
struct Arguments {
    email: String,
    pretty: bool,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody<'a> {
    code: &'static str,
    message: &'a str,
}

enum Action {
    Classify(Arguments),
    Help,
    Version,
}

fn main() -> ExitCode {
    let action = match parse_arguments(env::args_os().skip(1)) {
        Ok(action) => action,
        Err(message) => return report_error("invalidArguments", &message, EXIT_USAGE),
    };

    match action {
        Action::Help => {
            print_help();
            ExitCode::SUCCESS
        }
        Action::Version => {
            println!("gc-email-validator {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Action::Classify(arguments) => match classify_email(&arguments.email) {
            Ok(classification) => match serialize(&classification, arguments.pretty) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(error) => report_error("serializationError", &error.to_string(), EXIT_ERROR),
            },
            Err(error) => report_error("invalidEmail", &error.to_string(), EXIT_USAGE),
        },
    }
}

fn parse_arguments(arguments: impl Iterator<Item = OsString>) -> Result<Action, String> {
    let mut email = None;
    let mut pretty = false;
    let mut parse_options = true;

    for argument in arguments {
        let argument = argument
            .into_string()
            .map_err(|_| "arguments must be valid UTF-8".to_owned())?;

        if parse_options && argument == "--" {
            parse_options = false;
        } else if parse_options && matches!(argument.as_str(), "-h" | "--help") {
            return Ok(Action::Help);
        } else if parse_options && matches!(argument.as_str(), "-V" | "--version") {
            return Ok(Action::Version);
        } else if parse_options && argument == "--pretty" {
            pretty = true;
        } else if parse_options && argument.starts_with('-') {
            return Err(format!("unknown option {argument:?}"));
        } else if email.replace(argument).is_some() {
            return Err("provide exactly one email address".to_owned());
        }
    }

    let email = email.ok_or_else(|| "provide exactly one email address".to_owned())?;
    Ok(Action::Classify(Arguments { email, pretty }))
}

fn serialize(value: &impl Serialize, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
}

fn report_error(code: &'static str, message: &str, exit_code: u8) -> ExitCode {
    let envelope = ErrorEnvelope {
        error: ErrorBody { code, message },
    };
    match serde_json::to_string(&envelope) {
        Ok(json) => eprintln!("{json}"),
        Err(_) => eprintln!("{{\"error\":{{\"code\":\"serializationError\"}}}}"),
    }
    ExitCode::from(exit_code)
}

fn print_help() {
    println!(
        "gc-email-validator {version}\n\
         Classify an email address against the embedded Government of Canada domain dataset.\n\n\
         Usage: gc-email-validator [OPTIONS] <EMAIL>\n\n\
         Options:\n\
           --pretty       Pretty-print the JSON response\n\
           -h, --help     Print help\n\
           -V, --version  Print version",
        version = env!("CARGO_PKG_VERSION")
    );
}
