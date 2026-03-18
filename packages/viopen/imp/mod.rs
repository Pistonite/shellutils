// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::path::Path;

use cu::pre::*;

mod editor_config;
use editor_config::EditorConfig;

pub fn open_internal(editor: &str, file: &Path) -> cu::Result<()> {
    let editor = EditorConfig::find(editor)?;
    let file_str = editor.get_checked_file_path(file)?;
    cu::check!(
        spawn_editor(editor, file_str.clone()),
        "failed to spawn editor for path '{file_str}'"
    )
}

#[cfg(feature = "coroutine")]
pub async fn co_open_internal(editor: &str, file: &Path) -> cu::Result<()> {
    let editor = EditorConfig::find(editor)?;
    let file_str = editor.get_checked_file_path(file)?;
    cu::check!(
        co_spawn_editor(editor, file_str.clone()).await,
        "failed to spawn editor for path '{file_str}'"
    )
}

fn spawn_editor(mut editor: EditorConfig, path: String) -> cu::Result<()> {
    cu::trace!("spawning editor: {:?} for path {}", editor, path);
    #[cfg(windows)]
    {
        if editor.executable_lower.ends_with(".cmd") {
            editor.args.push(path);
            configure_and_spawn_powershell! {
                inherit: editor.inherit,
                executable: editor.executable,
                args: &editor.args,
                command => { command.wait()?; }
            }
            return Ok(());
        }
        if editor.executable_lower.ends_with("notepad.exe") {
            configure_and_spawn_powershell_for_path_and_wait! {
                executable: editor.executable,
                path: path,
                command => { command.wait()?; }
            }
            return Ok(());
        }
    }
    editor.args.push(path);
    configure_and_spawn! {
        inherit: editor.inherit,
        command: Path::new(&editor.executable).command().args(editor.args),
        command => { command.wait()?; }
    }
    Ok(())
}

#[cfg(feature = "coroutine")]
async fn co_spawn_editor(mut editor: EditorConfig, path: String) -> cu::Result<()> {
    cu::trace!("spawning editor: {:?} for path {}", editor, path);
    #[cfg(windows)]
    {
        if editor.executable_lower.ends_with(".cmd") {
            editor.args.push(path);
            configure_and_spawn_powershell! {
                inherit: editor.inherit,
                executable: editor.executable,
                args: &editor.args,
                command => { command.co_wait().await?; }
            }
            return Ok(());
        }
        if editor.executable_lower.ends_with("notepad.exe") {
            configure_and_spawn_powershell_for_path_and_wait! {
                executable: editor.executable,
                path: path,
                command => { command.co_wait().await?; }
            }
            return Ok(());
        }
    }
    editor.args.push(path);
    configure_and_spawn! {
        inherit: editor.inherit,
        command: Path::new(&editor.executable).command().args(editor.args),
        command => { command.co_wait().await?; }
    }
    Ok(())
}

mod macros {
    macro_rules! configure_and_spawn {
        (
            inherit: $inherit:expr,
            command: $configure:expr,
            $command:ident => $post:block
        ) => {{
            if { $inherit } {
                let $command = { $configure }.name("viopen").all_inherit();
                $post
            } else {
                #[cfg(feature = "print")]
                let $command = {
                    { $configure }
                        .name("viopen")
                        .stdout(cu::lv::T)
                        .stderr(cu::lv::T)
                        .stdin_null()
                };
                #[cfg(not(feature = "print"))]
                let $command = {
                    { $configure }
                        .name("viopen")
                        .stdout_null()
                        .stderr_null()
                        .stdin_null()
                };
                $post
            }
        }};
    }
    pub(crate) use configure_and_spawn;

    #[cfg(windows)]
    macro_rules! configure_and_spawn_powershell {
        (
            inherit: $inherit:expr,
            executable: $executable:expr,
            args: $args:expr,
            $command:ident => $post:block
        ) => {{
            let script = format!(
                "& \"{}\" {}",
                $executable,
                format_powershell_argument_list($args)?
            );
            configure_and_spawn! {
                inherit: $inherit,
                command: std::path::Path::new("powershell.exe").command().args([
                    "-NoLogo",
                    "-NoProfile",
                    "-Command",
                    &script
                ]),
                $command => $post
            }
        }};
    }
    #[cfg(windows)]
    pub(crate) use configure_and_spawn_powershell;

    #[cfg(windows)]
    macro_rules! configure_and_spawn_powershell_for_path_and_wait {
        (
            executable: $executable:expr,
            path: $path:expr,
            $command:ident => $post:block
        ) => {{
            let script = format!(
                "Start-Process -FilePath \"{}\" -ArgumentList \"{}\" -Wait",
                $executable, $path
            );
            configure_and_spawn! {
                inherit: false,
                command: std::path::Path::new("powershell.exe").command().args([
                    "-NoLogo",
                    "-NoProfile",
                    "-Command",
                    &script
                ]),
                $command => $post
            }
        }};
    }
    #[cfg(windows)]
    pub(crate) use configure_and_spawn_powershell_for_path_and_wait;
}
use macros::*;

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
