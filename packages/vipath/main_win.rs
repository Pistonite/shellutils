// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cu::pre::*;

#[derive(clap::Parser)]
pub struct Cli {
    /// Check, don't edit
    #[clap(short, long)]
    pub check: bool,
    #[clap(flatten)]
    pub flags: cu::cli::Flags,
}

pub fn run(cli: Cli) -> cu::Result<()> {
    cu::lv::disable_print_time();
    let path = cu::check!(temp_file_path(), "failed to determine temporary file path")?;
    // clean up previous temp file
    if path.is_file() {
        let applied = cu::check!(apply_file(&path), "failed to apply previous temporary file")?;
        if applied {
            cu::error!("please restart the terminal process and run `vipath -c`");
            return Ok(());
        } else {
            cu::fs::remove(&path)?;
        }
    }
    if cli.check {
        cu::info!("OK");
        return Ok(());
    }

    let content = cu::check!(parse_env(), "failed to parse PATH environment variables")?;
    cu::check!(
        cu::fs::write(&path, content),
        "failed to write PATH to temporary file"
    )?;

    cu::check!(
        viopen::open(&path),
        "unable to open temporary file in editor"
    )?;
    cu::check!(apply_file(&path), "failed to apply temporary file")?;
    Ok(())
}

fn apply_file(path: &Path) -> cu::Result<bool> {
    let content = cu::fs::read_string(path)?;
    let (system_paths, user_paths) = parse_path_file(&content)?;

    let current_system_paths = win_envedit::get_system("PATH")?;
    let current_user_paths = win_envedit::get_user("PATH")?;
    let mut applied = false;
    if system_paths != current_system_paths {
        cu::debug!("applying system={system_paths}");
        win_envedit::set_system("PATH", &system_paths)?;
        applied = true;
    }
    if user_paths != current_user_paths {
        cu::debug!("applying user={user_paths}");
        win_envedit::set_user("PATH", &user_paths)?;
        applied = true;
    }
    if applied {
        cu::warn!("PATH is updated, restart the terminal process and run `vipath -c`");
    }
    Ok(applied)
}

/// Parse path file into SYSTEM and USER paths
fn parse_path_file(content: &str) -> cu::Result<(String, String)> {
    let mut system_paths = vec![];
    let mut user_paths = vec![];
    let mut is_system = true;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            continue;
        }
        match line {
            "@SYSTEM" => is_system = true,
            "@USER" => is_system = false,
            _ => {
                let paths = if is_system {
                    &mut system_paths
                } else {
                    &mut user_paths
                };
                for p in line.split(';') {
                    let p = p.trim();
                    if !paths.contains(&p) {
                        paths.push(p);
                    }
                }
            }
        }
    }
    Ok((system_paths.join(";"), user_paths.join(";")))
}

fn parse_env() -> cu::Result<String> {
    let current_system_paths = win_envedit::get_system("PATH")?;
    let current_user_paths = win_envedit::get_user("PATH")?;
    cu::debug!("current system={current_system_paths}");
    cu::debug!("current user={current_user_paths}");
    let out = format!(
        r#"
# Temporary file for editing PATH
# Put one path per line, or multiple in the same line separated by ;
# Lines starting with # will be ignored
# @SYSTEM and @USER marks sections for SYSTEM path and USER path
# Duplicates will be removed

# -------------------------------
@SYSTEM
# -------------------------------
{}


# -------------------------------
@USER
# -------------------------------
{}

    "#,
        clean_path(&current_system_paths).join("\n"),
        clean_path(&current_user_paths).join("\n"),
    );

    Ok(out)
}

fn clean_path(x: &str) -> Vec<&str> {
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for s in x.split(';') {
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        if !seen.insert(s) {
            continue;
        }
        out.push(s);
    }
    out
}

fn temp_file_path() -> cu::Result<PathBuf> {
    let mut parent = cu::fs::current_exe()?.parent_abs()?;
    parent.push("vipath.temp");
    Ok(parent)
}
