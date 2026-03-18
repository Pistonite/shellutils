// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::path::Path;

use cu::pre::*;

#[derive(Debug, Clone)]
pub struct EditorConfig {
    /// If the editor should use inherit stdio (i.e. if the editor is terminal-based
    pub inherit: bool,
    /// If the editor supports opening a directory
    pub supports_directory: bool,
    pub executable: String,
    pub executable_lower: String,
    pub args: Vec<String>,
}

impl EditorConfig {
    /// Find the editor based on input, empty to find the editor on the system
    pub fn find(editor: &str) -> cu::Result<Self> {
        let editor = Self::find_internal(editor)?;
        if editor.executable_lower.contains("notepad++") {
            // has issues
            cu::bail!("notepad++ is not supported");
        }
        Ok(editor)
    }
    fn find_internal(editor: &str) -> cu::Result<Self> {
        if !editor.is_empty() {
            match Self::resolve_from_spec(editor) {
                Ok(Some(config)) => return Ok(config),
                Ok(None) => {}
                Err(e) => {
                    cu::trace!("failed to resolve editor spec: {e:?}, finding editor on system");
                }
            }
        }
        Self::find_on_system()
    }

    fn resolve_from_spec(editor: &str) -> cu::Result<Option<Self>> {
        // quick check
        if editor.eq_ignore_ascii_case("viopen") {
            // ignore viopen since it's recursive
            return Ok(None);
        }
        let args = cu::check!(shell_words::split(editor), "failed to split editor command")?;

        // +4 for additional args that will be added, like the file path
        let mut new_args = Vec::with_capacity(args.len() + 4);
        let mut args_iter = args.into_iter();
        let executable = cu::check!(args_iter.next(), "no executable found")?;
        let executable = cu::which(&executable)?;
        if let Some(x) = executable.file_stem() {
            if x.eq_ignore_ascii_case("viopen") {
                cu::bail!("ignoring EDITOR=viopen");
            }
        }
        let executable = executable.into_utf8()?;
        let editor_type = EditorType::guess(&executable);

        let inherit = match editor_type {
            EditorType::Notepad => {
                new_args.extend(args_iter);
                false
            }
            EditorType::Terminal { .. } => {
                new_args.extend(args_iter);
                true
            }
            EditorType::WFlagOrWaitFlag { .. } => {
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
                false
            }
        };

        let config = Self::new(
            inherit,
            executable,
            editor_type.supports_directory(),
            new_args,
        );

        Ok(Some(config))
    }
    fn find_on_system() -> cu::Result<EditorConfig> {
        // common ones - vi/emacs/code/subl
        if let Some(x) = find_executable_full_path("nvim") {
            return Ok(Self::inherit(x, true, vec![]));
        }
        if let Some(x) = find_executable_full_path("emacs") {
            return Ok(Self::inherit(x, true, vec![]));
        }
        if let Some(x) = find_executable_full_path("code") {
            return Ok(Self::dont_inherit(x, true, vec!["-w".to_string()]));
        }
        if let Some(x) = find_executable_full_path("subl") {
            return Ok(Self::dont_inherit(x, true, vec!["-w".to_string()]));
        }
        if let Some(x) = find_executable_full_path("vi") {
            return Ok(Self::inherit(x, true, vec![]));
        }
        if let Some(x) = find_executable_full_path("vim") {
            return Ok(Self::inherit(x, true, vec![]));
        }
        if let Some(x) = find_executable_full_path("xemacs") {
            return Ok(Self::inherit(x, true, vec![]));
        }
        if let Some(x) = find_executable_full_path("nano") {
            return Ok(Self::inherit(x, false, vec![]));
        }

        // less common variants
        if cfg!(windows) {
            for x in ["vscode", "vsc", "sublime"] {
                if let Some(x) = find_executable_full_path(x) {
                    return Ok(Self::dont_inherit(x, true, vec!["-w".to_string()]));
                }
            }
        }

        for x in ["nvi", "neovim", "elvis", "vile", "v", "stevie"] {
            if let Some(x) = find_executable_full_path(x) {
                return Ok(Self::inherit(x, true, vec![]));
            }
        }
        for x in ["mg", "uemacs", "jove", "zile"] {
            if let Some(x) = find_executable_full_path(x) {
                return Ok(Self::inherit(x, true, vec![]));
            }
        }

        if cfg!(not(windows)) {
            for x in ["vscode", "vsc", "sublime"] {
                if let Some(x) = find_executable_full_path(x) {
                    return Ok(Self::dont_inherit(x, true, vec!["-w".to_string()]));
                }
            }
        } else {
            if let Some(x) = find_executable_full_path("notepad.exe") {
                return Ok(Self::dont_inherit(x, false, vec![]));
            }
        }

        cu::bail!("failed to find compatible editor, please set the EDITOR environment variable");
    }

    fn inherit(executable: impl Into<String>, supports_directory: bool, args: Vec<String>) -> Self {
        Self::new(true, executable, supports_directory, args)
    }
    fn dont_inherit(
        executable: impl Into<String>,
        supports_directory: bool,
        args: Vec<String>,
    ) -> Self {
        Self::new(false, executable, supports_directory, args)
    }
    fn new(
        inherit: bool,
        executable: impl Into<String>,
        supports_directory: bool,
        args: Vec<String>,
    ) -> Self {
        let s = executable.into();
        Self {
            inherit,
            supports_directory,
            executable_lower: s.to_lowercase(),
            executable: s,
            args,
        }
    }

    #[cu::context("failed to check the file path to edit")]
    pub fn get_checked_file_path(&self, file: &Path) -> cu::Result<String> {
        let file_str = if file.is_absolute() {
            file.as_utf8()?.to_string()
        } else {
            file.normalize()?.into_utf8()?
        };
        if Path::new(&file_str).is_dir() && !self.supports_directory {
            cu::bail!(
                "editor '{}' does not support editing directory: '{}' is a directory",
                self.executable,
                file_str
            );
        }
        Ok(file_str)
    }
}

pub enum EditorType {
    Terminal { supports_directory: bool },
    WFlagOrWaitFlag { supports_directory: bool },
    Notepad,
}
impl EditorType {
    fn guess(executable: &str) -> Self {
        let mut file_name = match executable.rfind(['/', '\\']) {
            None => executable,
            Some(i) => &executable[i + 1..],
        };
        while let Some(i) = file_name.rfind('.') {
            file_name = &file_name[..i];
        }
        let file_name = file_name.trim();
        for n in ["code", "vscode", "vsc", "sublime", "subl"] {
            if file_name.eq_ignore_ascii_case(n) {
                return EditorType::WFlagOrWaitFlag {
                    supports_directory: true,
                };
            }
        }
        if file_name.eq_ignore_ascii_case("notepad") {
            return EditorType::Notepad;
        }
        if file_name.eq_ignore_ascii_case("nano") {
            return EditorType::Terminal {
                supports_directory: false,
            };
        }

        EditorType::Terminal {
            supports_directory: true,
        }
    }

    pub fn supports_directory(&self) -> bool {
        match self {
            EditorType::Terminal { supports_directory } => *supports_directory,
            EditorType::WFlagOrWaitFlag { supports_directory } => *supports_directory,
            EditorType::Notepad => false,
        }
    }
}

fn find_executable_full_path(executable: &str) -> Option<String> {
    let path = cu::which(executable).ok()?;
    match path.into_utf8() {
        Err(e) => {
            cu::trace!(
                "not using editor '{executable}' because the resolved path is not utf-8: {e}",
            );
            None
        }
        Ok(x) => Some(x),
    }
}
