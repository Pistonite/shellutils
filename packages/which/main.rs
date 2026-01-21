// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// which - shows the full patah of (shell) commands
#[derive(Debug, Clone, Parser)]
struct Cli {
    /// Name of the program to expand
    pub programname: String,
    /// Get all matches
    #[clap(short, long)]
    pub all: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.all {
        match which::which_all_global(&cli.programname) {
            Ok(x) => x.for_each(print_path),
            Err(e) => return print_error(&cli.programname, e),
        }
    } else {
        match which::which_global(&cli.programname) {
            Ok(path) => print_path(path),
            Err(e) => return print_error(&cli.programname, e),
        }
    }

    ExitCode::SUCCESS
}

fn print_error(programname: &str, e: which::Error) -> ExitCode {
    let paths = std::env::var("PATH").unwrap_or_default();

    let mut error_string = format!(": {e}");
    // clear the error string for the most common error for same output
    // as unix
    if error_string == ": cannot find binary path" {
        error_string.clear()
    }

    eprintln!("which: no {programname} in ({paths}){error_string}");
    ExitCode::FAILURE
}

fn print_path(path: PathBuf) {
    println!("{}", path.display())
}
