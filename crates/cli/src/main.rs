#![forbid(unsafe_code)]

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    if command == "--help" || command == "-h" {
        println!("{}", usage());
        return Ok(());
    }
    if command != "scan-image" {
        return Err(usage());
    }
    let path = args.next().ok_or_else(usage)?;
    let pretty = match args.next() {
        None => false,
        Some(flag) if flag == "--pretty" => true,
        Some(_) => return Err(usage()),
    };
    if args.next().is_some() {
        return Err(usage());
    }

    let report = um_cli::scan_image_path(Path::new(&path)).map_err(|error| error.to_string())?;
    let stdout = io::stdout();
    let mut locked = stdout.lock();
    if pretty {
        serde_json::to_writer_pretty(&mut locked, &report)
    } else {
        serde_json::to_writer(&mut locked, &report)
    }
    .map_err(|error| format!("report serialization failed: {error}"))?;
    locked
        .write_all(b"\n")
        .map_err(|error| format!("report output failed: {error}"))
}

fn usage() -> String {
    "usage: undelete-master scan-image <path.img|path.dd|path.raw|path.bin> [--pretty]".to_string()
}
