// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::path::Path;

mod imp;

#[inline(always)]
pub fn open(path: impl AsRef<Path>) -> cu::Result<()> {
    imp::open_internal(&cu::env_var("EDITOR").unwrap_or_default(), path.as_ref())
}

#[inline(always)]
pub fn open_with(editor: impl AsRef<str>, path: impl AsRef<Path>) -> cu::Result<()> {
    imp::open_internal(editor.as_ref(), path.as_ref())
}

#[inline(always)]
#[cfg(feature = "coroutine")]
pub async fn co_open(path: impl AsRef<Path>) -> cu::Result<()> {
    imp::co_open_internal(&cu::env_var("EDITOR").unwrap_or_default(), path.as_ref()).await
}

#[inline(always)]
#[cfg(feature = "coroutine")]
pub async fn co_open_with(editor: impl AsRef<str>, path: impl AsRef<Path>) -> cu::Result<()> {
    imp::co_open_internal(editor.as_ref(), path.as_ref()).await
}
