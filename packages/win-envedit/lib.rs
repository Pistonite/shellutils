// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

#[cfg(not(windows))]
compile_error!(
    "this package only works on Windows, please add it to target.'cfg(windows)'.dependencies"
);
#[cfg(windows)]
mod lib_win;
#[cfg(windows)]
pub use lib_win::*;
