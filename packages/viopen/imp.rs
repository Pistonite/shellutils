// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::path::Path;

use cu::pre::*;
pub fn open_internal(editor: &str, file: &Path) -> cu::Result<()> {
    let file_str = get_checked_file_path(file)?;
    let editor = find_editor(editor)?;
    cu::check!(
        spawn_editor(editor.clone(), file_str.clone()),
        "failed to spawn editor '{editor:?}' for file '{file_str}'"
    )
}

#[cfg(feature = "coroutine")]
pub async fn co_open_internal(editor: &str, file: &Path) -> cu::Result<()> {
    let file_str = get_checked_file_path(file)?;
    let editor = find_editor(editor)?;
    cu::check!(
        co_spawn_editor(editor.clone(), file_str.clone()).await,
        "failed to spawn editor '{editor:?}' for file '{file_str}'"
    )
}

#[cu::context("failed to check the file path to edit")]
fn get_checked_file_path(file: &Path) -> cu::Result<String> {
    let file_str = if file.is_absolute() {
        file.as_utf8()?.to_string()
    } else {
        file.normalize()?.into_utf8()?
    };
    Ok(file_str)
}

fn spawn_editor(mut editor: EditorConfig, file: String) -> cu::Result<()> {
    cu::trace!("spawning editor: {} for file {}", editor.executable, file);
    #[cfg(windows)]
    {
        if editor.executable_lower.ends_with(".cmd") {
            editor.args.push(file);
            return spawn_cmd_with_powershell(editor.inherit, &editor.executable, &editor.args);
        }
        if editor.executable_lower.ends_with("notepad.exe") {
            return spawn_notepad_with_powershell(&editor.executable, &file);
        }
    }
    editor.args.push(file);
    if editor.inherit {
        configure_command_io!(__inherit__ Path::new(&editor.executable).command().args(editor.args))
            .wait_nz()
    } else {
        configure_command_io!(Path::new(&editor.executable).command().args(editor.args)).wait_nz()
    }
}

#[cfg(feature = "coroutine")]
async fn co_spawn_editor(mut editor: EditorConfig, file: String) -> cu::Result<()> {
    cu::trace!("spawning editor: {}", editor.executable);
    #[cfg(windows)]
    {
        if editor.executable_lower.ends_with(".cmd") {
            editor.args.push(file);
            return co_spawn_cmd_with_powershell(editor.inherit, &editor.executable, &editor.args)
                .await;
        }
        if editor.executable_lower.ends_with("notepad.exe") {
            return co_spawn_notepad_with_powershell(&editor.executable, &file).await;
        }
    }
    editor.args.push(file);
    if editor.inherit {
        configure_command_io!(__inherit__ Path::new(&editor.executable).command().args(editor.args))
            .co_wait_nz()
            .await
    } else {
        configure_command_io!(Path::new(&editor.executable).command().args(editor.args))
            .co_wait_nz()
            .await
    }
}

fn find_editor(editor: &str) -> cu::Result<EditorConfig> {
    let editor = find_editor_internal(editor)?;
    if editor.executable_lower.contains("notepad++") {
        // has issues
        cu::bail!("notepad++ is not supported");
    }
    Ok(editor)
}

fn find_editor_internal(editor: &str) -> cu::Result<EditorConfig> {
    if !editor.is_empty() {
        if let Ok(args) = parse_editor(editor) {
            return Ok(args);
        }
    }

    // common ones - vi/emacs/code/subl
    if let Some(x) = find_executable("nvim") {
        return Ok(EditorConfig::inherit(x, vec![]));
    }
    if let Some(x) = find_executable("emacs") {
        return Ok(EditorConfig::inherit(x, vec![]));
    }
    if let Some(x) = find_executable("code") {
        return Ok(EditorConfig::dont_inherit(x, vec!["-w".to_string()]));
    }
    if let Some(x) = find_executable("subl") {
        return Ok(EditorConfig::dont_inherit(x, vec!["-w".to_string()]));
    }
    if let Some(x) = find_executable("vi") {
        return Ok(EditorConfig::inherit(x, vec![]));
    }
    if let Some(x) = find_executable("vim") {
        return Ok(EditorConfig::inherit(x, vec![]));
    }
    if let Some(x) = find_executable("xemacs") {
        return Ok(EditorConfig::inherit(x, vec![]));
    }
    if let Some(x) = find_executable("nano") {
        return Ok(EditorConfig::inherit(x, vec![]));
    }

    // less common variants
    if cfg!(windows) {
        for x in ["vscode", "vsc", "sublime"] {
            if let Some(x) = find_executable(x) {
                return Ok(EditorConfig::dont_inherit(x, vec!["-w".to_string()]));
            }
        }
    }

    for x in ["nvi", "neovim", "elvis", "vile", "v", "stevie"] {
        if let Some(x) = find_executable(x) {
            return Ok(EditorConfig::inherit(x, vec![]));
        }
    }
    for x in ["mg", "uemacs", "jove", "zile"] {
        if let Some(x) = find_executable(x) {
            return Ok(EditorConfig::inherit(x, vec![]));
        }
    }

    if cfg!(not(windows)) {
        for x in ["vscode", "vsc", "sublime"] {
            if let Some(x) = find_executable(x) {
                return Ok(EditorConfig::dont_inherit(x, vec!["-w".to_string()]));
            }
        }
    } else {
        if let Some(x) = find_executable("notepad.exe") {
            return Ok(EditorConfig::dont_inherit(x, vec![]));
        }
    }

    cu::bail!("failed to find compatible editor, please set the EDITOR environment variable");
}

fn parse_editor(editor: &str) -> cu::Result<EditorConfig> {
    // quick check
    if editor == "viopen" {
        cu::bail!("ignoring EDITOR=viopen");
    }
    let args = cu::check!(shell_words::split(editor), "failed to split editor command")?;

    // +4 for additional args that will be added, like the file path
    let mut new_args = Vec::with_capacity(args.len() + 4);
    let mut args_iter = args.into_iter();
    let executable = cu::check!(args_iter.next(), "no executable found")?;
    let executable = cu::which(&executable)?;
    if let Some(x) = executable.file_stem() {
        if x == "viopen" {
            cu::bail!("ignoring EDITOR=viopen");
        }
    }
    let executable = executable.into_utf8()?;
    let editor_type = guess_editor_type(&executable);

    match editor_type {
        EditorType::Notepad => {
            new_args.extend(args_iter);
            Ok(EditorConfig::dont_inherit(executable, new_args))
        }
        EditorType::Terminal => {
            new_args.extend(args_iter);
            Ok(EditorConfig::inherit(executable, new_args))
        }
        EditorType::WFlagOrWaitFlag => {
            let mut found_wait_flag = false;
            for arg in args_iter {
                if !found_wait_flag {
                    let l = arg.trim().to_lowercase();
                    if l.as_bytes() == b"-w" || l.as_bytes() == b"--wait" {
                        found_wait_flag = true;
                    }
                }
                new_args.push(arg);
            }
            if !found_wait_flag {
                new_args.push("-w".to_string());
            }
            Ok(EditorConfig::dont_inherit(executable, new_args))
        }
    }
}

#[derive(Debug, Clone)]
struct EditorConfig {
    inherit: bool,
    executable: String,
    executable_lower: String,
    args: Vec<String>,
}
impl EditorConfig {
    fn inherit(executable: impl Into<String>, args: Vec<String>) -> Self {
        let s = executable.into();
        Self {
            inherit: true,
            executable_lower: s.to_lowercase(),
            executable: s,
            args,
        }
    }
    fn dont_inherit(executable: impl Into<String>, args: Vec<String>) -> Self {
        let s = executable.into();
        Self {
            inherit: false,
            executable_lower: s.to_lowercase(),
            executable: s,
            args,
        }
    }
}

fn guess_editor_type(executable: &str) -> EditorType {
    let mut file_name = match executable.rfind(['/', '\\']) {
        None => executable,
        Some(i) => &executable[i + 1..],
    };
    while let Some(i) = file_name.rfind('.') {
        file_name = &file_name[..i];
    }
    let file_name = file_name.trim().to_lowercase();
    match file_name.as_bytes() {
        b"code" | b"vscode" | b"vsc" | b"sublime" | b"subl" => EditorType::WFlagOrWaitFlag,
        b"notepad" => EditorType::Notepad,
        _ => EditorType::Terminal,
    }
}

enum EditorType {
    Terminal,
    WFlagOrWaitFlag,
    Notepad,
}

fn find_executable(executable: &str) -> Option<String> {
    let path = cu::which(executable).ok()?;
    match path.as_utf8() {
        Err(_) => {
            cu::trace!(
                "not using editor '{}' because it's not utf-8",
                path.display()
            );
            None
        }
        Ok(x) => Some(x.to_string()),
    }
}

#[cfg(windows)]
fn spawn_cmd_with_powershell(inherit: bool, executable: &str, args: &[String]) -> cu::Result<()> {
    let argument_list = format_powershell_argument_list(args)?;
    // executable does not need to be escaped because windows paths can't contain quote so we are safe
    // (the input string must be a validated path)
    if inherit {
        make_powershell_command!(__inherit__ executable, argument_list).wait_nz()
    } else {
        make_powershell_command!(executable, argument_list).wait_nz()
    }
}

#[cfg(windows)]
#[cfg(all(windows, feature = "coroutine"))]
async fn co_spawn_cmd_with_powershell(
    inherit: bool,
    executable: &str,
    args: &[String],
) -> cu::Result<()> {
    let argument_list = format_powershell_argument_list(args)?;
    // executable does not need to be escaped because windows paths can't contain quote so we are safe
    // (the input string must be a validated path)
    if inherit {
        make_powershell_command!(__inherit__ executable, argument_list)
            .co_wait_nz()
            .await
    } else {
        make_powershell_command!(executable, argument_list)
            .co_wait_nz()
            .await
    }
}

#[cfg(windows)]
fn spawn_notepad_with_powershell(executable: &str, file: &str) -> cu::Result<()> {
    // both executable and file are valid windows paths
    make_powershell_wait_command!(executable, file).wait_nz()
}

#[cfg(all(windows, feature = "coroutine"))]
async fn co_spawn_notepad_with_powershell(executable: &str, file: &str) -> cu::Result<()> {
    // both executable and file are valid windows paths
    make_powershell_wait_command!(executable, file)
        .co_wait_nz()
        .await
}

#[cfg(windows)]
fn format_powershell_argument_list(args: &[String]) -> cu::Result<String> {
    let mut argument_list = String::new();
    let mut args_iter = args.iter();
    if let Some(first) = args_iter.next() {
        if first.contains(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '`' | '$')) {
            cu::bail!("the editor command contains illegal characters that cannot be spawned");
        }
        argument_list.push('"');
        argument_list.push_str(first);
        argument_list.push('"');
        for s in args_iter {
            if s.contains(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '`' | '$')) {
                cu::bail!("the editor command contains illegal characters that cannot be spawned");
            }
            argument_list.push_str(" \"");
            argument_list.push_str(s);
            argument_list.push('"');
        }
    }
    Ok(argument_list)
}

mod macros {
    macro_rules! configure_command_io {
        (__inherit__ $command:expr) => {{ { $command }.name("viopen").all_inherit() }};
        ($command:expr) => {{
            #[cfg(feature = "print")]
            {
                { $command }
                    .name("viopen")
                    .stdout(cu::lv::T)
                    .stderr(cu::lv::T)
                    .stdin_null()
            }
            #[cfg(not(feature = "print"))]
            {
                { $command }
                    .name("viopen")
                    .stdout_null()
                    .stderr_null()
                    .stdin_null()
            }
        }};
    }
    pub(crate) use configure_command_io;

    #[cfg(windows)]
    macro_rules! make_powershell_command {
        (__inherit__ $executable:ident, $arg:ident) => {{
            let script = format!("& \"{}\" {}", $executable, $arg);
            configure_command_io!(__inherit__ std::path::Path::new("powershell.exe").command().args([
                "-NoLogo",
                "-NoProfile",
                "-Command",
                &script
            ]))
        }};
        ($executable:ident, $arg:ident) => {{
            let script = format!("& \"{}\" {}", $executable, $arg);
            configure_command_io!(std::path::Path::new("powershell.exe").command().args([
                "-NoLogo",
                "-NoProfile",
                "-Command",
                &script
            ]))
        }};
    }
    #[cfg(windows)]
    pub(crate) use make_powershell_command;

    #[cfg(windows)]
    macro_rules! make_powershell_wait_command {
        ($executable:ident, $arg:ident) => {{
            let script = format!(
                "Start-Process -FilePath \"{}\" -ArgumentList \"{}\" -Wait",
                $executable, $arg
            );
            configure_command_io!(std::path::Path::new("powershell.exe").command().args([
                "-NoLogo",
                "-NoProfile",
                "-Command",
                &script
            ]))
        }};
    }
    #[cfg(windows)]
    pub(crate) use make_powershell_wait_command;
}
use macros::*;
