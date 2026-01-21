// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use cu::pre::*;
fn main() -> cu::Result<()> {
    let mut args = std::env::args();
    // executable name
    let _ = args.next();
    // only support one file name for now
    let file = cu::check!(args.next(), "need file name")?;
    viopen::open(&file)
}
