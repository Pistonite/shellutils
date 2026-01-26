// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(mut input) = std::env::args().nth(1) else {
        eprintln!("usage: n <number>");
        return ExitCode::FAILURE;
    };
    input.make_ascii_lowercase();
    let (sign_i, sign_f) = match input.strip_prefix('-') {
        Some(_) => (-1i64, -1f64),
        None => (1i64, 1f64),
    };
    input.retain(|c| !matches!(c, ' ' | '_' | ',' | '-' | '+'));

    if let Err(e) = main_internal(sign_f, sign_i, &input) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn main_internal(sign_f: f64, sign_i: i64, input: &str) -> Result<(), String> {
    if input.contains('.') {
        print_float_info(sign_f * parse_f64(input)?);
        return Ok(());
    }
    if input.contains(['a', 'b', 'c', 'd', 'e', 'f']) {
        if let Some(hex) = input.strip_prefix("x") {
            print_int_info(sign_i * parse_i64(hex, 16)?);
            return Ok(());
        }
        if let Some(hex) = input.strip_prefix("0x") {
            print_int_info(sign_i * parse_i64(hex, 16)?);
            return Ok(());
        }
    }
    if let Some(bin) = input.strip_prefix("0b") {
        print_int_info(sign_i * parse_i64(bin, 2)?);
        return Ok(());
    }
    print_int_info(sign_i * parse_i64(input, 10)?);
    Ok(())
}

fn parse_f64(input: &str) -> Result<f64, String> {
    input
        .parse::<f64>()
        .map_err(|e| format!("failed to parse float: {e}"))
}

fn parse_i64(input: &str, radix: u32) -> Result<i64, String> {
    i64::from_str_radix(input, radix)
        .map_err(|e| format!("failed to parse integer with radix {radix}: {e}"))
}

fn print_int_info(n: i64) {
    let u64_val = n as u64;
    let u32_val = u64_val as u32;
    let i64_val = n;
    let i32_val = u32_val as i32;
    let f32_val = f32::from_bits(u32_val);
    let f64_val = f64::from_bits(u64_val);
    print_info(u32_val, u64_val, i32_val, i64_val, f32_val, f64_val);
}

fn print_float_info(n: f64) {
    let f32_val = n as f32;
    let u32_val = f32_val.to_bits();
    let u64_val = n.to_bits();
    let i32_val = u32_val as i32;
    let i64_val = u64_val as i64;
    print_info(u32_val, u64_val, i32_val, i64_val, f32_val, n);
}

fn print_info(u32_val: u32, u64_val: u64, i32_val: i32, i64_val: i64, f32_val: f32, f64_val: f64) {
    if u32_val as i64 == i32_val as i64 {
        print_line("Decimal-32", u32_val);
    } else {
        print_line("Signed-32", i32_val);
        print_line("Unsigned-32", u32_val);
    }

    if u64_val as i128 == i64_val as i128 {
        print_line("Decimal-64", u64_val);
    } else {
        print_line("Signed-64", i64_val);
        print_line("Unsigned-64", u64_val);
    }

    if u32_val as u64 == u64_val {
        print_line("Hex", format!("0x{:x}", u32_val));
        print_line("Binary", group(&format!("{:b}", u32_val), 4));
    } else {
        print_line("Hex-32", format!("0x{:x}", u32_val));
        print_line("Hex-64", format!("0x{:x}", u64_val));
        print_line("Binary-32", group(&format!("{:b}", u32_val), 4));
        print_line("Binary-64", group(&format!("{:b}", u64_val), 4));
    }

    if f32_val as f64 == f64_val {
        print_line("IEEE-754", FloatDisplay(f32_val));
    } else {
        print_line("Float-32", FloatDisplay(f32_val));
        print_line("Float-64", FloatDisplay(f64_val));
    }
}

struct FloatDisplay<T>(T);

macro_rules! impl_float_display {
    ($($t:ty),*) => {
        $(impl std::fmt::Display for FloatDisplay<$t> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut normal = format!("{}", self.0);
                if !normal.contains('.') {
                    normal.push_str(".0");
                }
                if normal.len() > 32 {
                    write!(f, "{:e}", self.0)
                } else {
                    write!(f, "{}", normal)
                }
            }
        })*
    };
}

impl_float_display!(f32, f64);

fn print_line(label: &str, value: impl std::fmt::Display) {
    println!("{:<16}: {}", label, value);
}

fn group(s: &str, n: usize) -> String {
    let padding = (n - (s.len() % n)) % n;
    let padded = format!("{:0>width$}", s, width = s.len() + padding);
    padded
        .as_bytes()
        .chunks(n)
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(" ")
}
