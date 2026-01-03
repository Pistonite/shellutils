// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

#[cfg(not(windows))]
compile_error!("this package can only be installed on windows");
#[cfg(windows)]
mod main_win;
#[cfg(windows)]
#[cu::cli(flags = "flags")]
fn main(cli: main_win::Cli) -> cu::Result<()> {
    main_win::run(cli)
}
