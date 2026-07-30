#![forbid(unsafe_code)]

use std::process::ExitCode;

use um_elevated_broker::{run_platform, BrokerArgs};

fn main() -> ExitCode {
    let arguments = match BrokerArgs::parse(std::env::args_os().skip(1)) {
        Ok(arguments) => arguments,
        Err(_) => return ExitCode::from(2),
    };
    match run_platform(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(1),
    }
}
