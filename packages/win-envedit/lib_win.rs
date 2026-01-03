// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::io::ErrorKind;

use cu::pre::*;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_WRITE};
use winreg::{HKEY, RegKey};

static SYSTEM_PATH: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
static USER_PATH: &str = "Environment";

/// Get system environment variable. Not set is returned as empty
pub fn get_system(key: &str) -> cu::Result<String> {
    cu::check!(
        get_from_key_path(key, HKEY_LOCAL_MACHINE, SYSTEM_PATH),
        "failed to get system environment variable '{key}'"
    )
}

/// Get user environment variable. Not set is returned as empty
pub fn get_user(key: &str) -> cu::Result<String> {
    cu::check!(
        get_from_key_path(key, HKEY_CURRENT_USER, USER_PATH),
        "failed to get user environment variable '{key}'"
    )
}

/// Set system environment variable.
pub fn set_system(key: &str, value: &str) -> cu::Result<()> {
    cu::check!(
        set_from_key_path(key, HKEY_LOCAL_MACHINE, SYSTEM_PATH, value),
        "failed to set system environment variable '{key}'"
    )
}

/// Set user environment variable.
pub fn set_user(key: &str, value: &str) -> cu::Result<()> {
    cu::check!(
        set_from_key_path(key, HKEY_CURRENT_USER, USER_PATH, value),
        "failed to set user environment variable '{key}'"
    )
}

fn get_from_key_path(name: &str, key: HKEY, subpath: &str) -> cu::Result<String> {
    let reg_key = cu::check!(
        RegKey::predef(key).open_subkey(subpath),
        "open_subkey failed"
    )?;
    match reg_key.get_value(name) {
        Ok(value) => Ok(value),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok("".to_string()),
        Err(e) => {
            cu::rethrow!(e, "failed to get reg key value");
        }
    }
}

fn set_from_key_path(name: &str, key: HKEY, subpath: &str, value: &str) -> cu::Result<()> {
    let reg_key = cu::check!(
        RegKey::predef(key).open_subkey_with_flags(subpath, KEY_WRITE),
        "failed to open_subkey with write flag"
    )?;
    cu::check!(
        reg_key.set_value(name, &value),
        "failed to set reg key value"
    )
}
