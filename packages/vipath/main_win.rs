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
    cu::disable_print_time();
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
        start_edit_and_wait(&path),
        "unable to open temporary file in editor"
    )?;
    cu::info!("applying changes...");
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

fn start_edit_and_wait(path: &Path) -> cu::Result<()> {
    if let Ok(bin) = cu::which("nvim") {
        return start_edit_and_wait_vim(path, &bin);
    }
    if let Ok(bin) = cu::which("vim") {
        return start_edit_and_wait_vim(path, &bin);
    }
    if let Ok(bin) = cu::which("vi") {
        return start_edit_and_wait_vim(path, &bin);
    }
    if let Ok(bin) = cu::which("code") {
        return start_edit_and_wait_code(path, &bin);
    }
    if let Ok(bin) = cu::which("notepad") {
        cu::warn!("cannot find compatible editor - falling back to notepad");
        cu::warn!("please make the edit, then run `vipath -c` to apply the changes");
        cu::check!(start_edit_notepad(path, &bin), "failed to launch notepad")?;
    }
    cu::bail!("no compatible editor");
}
fn start_edit_and_wait_vim(path: &Path, vim: &Path) -> cu::Result<()> {
    cu::info!("waiting for vim to exit...");
    let _ = vim
        .command()
        .arg(path)
        .stdoe(cu::pio::inherit())
        .stdin_inherit()
        .wait()?;
    Ok(())
}
fn start_edit_and_wait_code(path: &Path, code: &Path) -> cu::Result<()> {
    cu::info!("waiting for vscode to exit...");
    let _ = code
        .command()
        .add(cu::args!["--wait", path])
        .all_null()
        .wait()?;
    Ok(())
}
fn start_edit_notepad(path: &Path, notepad: &Path) -> cu::Result<()> {
    notepad.command().arg(path).all_null().wait_nz()
}

fn temp_file_path() -> cu::Result<PathBuf> {
    let mut parent = cu::fs::current_exe()?.parent_abs()?;
    parent.push("vipath.temp");
    Ok(parent)
}
