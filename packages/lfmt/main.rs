// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::path::{Path, PathBuf};

use cu::pre::*;
use ignore::WalkBuilder as IgnoreWalkBuilder;

/// Line end formatter
#[derive(clap::Parser)]
struct Cli {
    /// Paths to check or format. Directores are searched recursively
    ///
    /// Non-UTF-8 files are skipped (although will still be read to check if they
    /// are UTF-8)
    ///
    /// If no patterns are specified, current directory is used.
    /// Note that ignore files such as `.gitignore` are respected
    /// unless --no-ignore is used. `.lfmtignore` is always respected
    paths: Vec<String>,

    /// Specify the line ending to use
    ///
    /// In either mode, single CR ('\r') will be treated as a line ending
    /// and converted to either LF or CRLF
    #[clap(short = 'n', long, default_value = "lf")]
    end: LineEnd,

    /// Check, don't format (a.k.a, dry run)
    ///
    /// In quiet mode (-q), the list of files that need formatting
    /// will be printed without other formatting characters.
    /// In quietquiet mode (-qq), all output will be suppressed, only
    /// failure return
    #[clap(short, long)]
    check: bool,

    /// Don't respect ignore files such as `.ignore` or `.gitignore` (using the `ignore` crate)
    ///
    /// `.lfmtignore is always respected reguardless of this setting`
    #[clap(short = 'N', long)]
    no_ignore: bool,

    #[clap(flatten)]
    flags: cu::cli::Flags,

    #[clap(skip)]
    quieter_check: bool,
}
impl Cli {
    pub fn preprocess(&mut self) {
        if self.check {
            let level = self.flags.verbose as i8 - self.flags.quiet as i8;
            if level == -1 {
                // force no output
                self.flags.quiet = self.flags.verbose + 2;
                self.quieter_check = true;
            }
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum LineEnd {
    Lf,
    Crlf,
}

#[cu::cli(flags = "flags", preprocess = Cli::preprocess)]
async fn main(args: Cli) -> cu::Result<()> {
    let end = args.end;
    let check = args.check;
    let quieter_check = args.quieter_check;

    let mut paths_iter = args.paths.into_iter();
    let mut builder = match paths_iter.next() {
        Some(x) => {
            let mut builder = IgnoreWalkBuilder::new(Path::new(&x).normalize()?);
            for path in paths_iter {
                builder.add(Path::new(&path).normalize()?);
            }
            builder
        }
        None => {
            let cwd = Path::new(".").normalize()?;
            IgnoreWalkBuilder::new(cwd)
        }
    };

    if args.no_ignore {
        builder
            .ignore(false)
            .git_global(false)
            .git_ignore(false)
            .git_exclude(false);
    } else {
        builder.require_git(true);
    }
    builder.add_custom_ignore_filename(".lfmtignore");
    let walk = builder.build();

    let pool = cu::co::pool(-1);
    let mut handles = vec![];

    let mut main_error = false;
    let mut check_error = false;
    for entry in walk {
        match entry {
            Err(e) => process_message(
                Err(format!("failed to read dir entry: {e}")),
                quieter_check,
                &mut main_error,
                &mut check_error,
            ),
            Ok(e) => {
                let handle = pool.spawn(async move { process_file(e.path(), end, check).await });
                handles.push(handle)
            }
        }
    }

    let mut set = cu::co::set(handles);
    while let Some(message) = set.next().await {
        if let Ok(message) = message {
            process_message(message, quieter_check, &mut main_error, &mut check_error);
        }
    }

    if main_error {
        if quieter_check {
            // using stderr to still display the error
            eprintln!("encounted errors while processing the files");
            std::process::exit(1);
        }
        cu::bail!("encounted errors while processing the files");
    }

    if check {
        if check_error {
            if quieter_check {
                std::process::exit(1);
            }
            cu::bailfyi!("found files that need formatting");
        }
        cu::info!("check ok");
    }

    Ok(())
}

type Message = Result<Option<Event>, String>;
fn process_message(
    message: Message,
    quieter_check: bool,
    main_error: &mut bool,
    check_error: &mut bool,
) {
    let event = match message {
        Err(msg) => {
            if quieter_check {
                // using stderr to still display the error
                eprintln!("{msg}");
            } else {
                cu::error!("{msg}");
            }
            *main_error = true;
            return;
        }
        Ok(None) => return,
        Ok(Some(x)) => x,
    };
    match event.kind {
        EventKind::Skipped => {
            cu::trace!("skipped {}", event.path.try_to_rel().display());
        }
        EventKind::NoChange => {
            cu::debug!("ok {}", event.path.try_to_rel().display());
        }
        EventKind::Formatted => {
            cu::info!("formatted {}", event.path.try_to_rel().display());
        }
        EventKind::NeedsFormat => {
            if quieter_check {
                println!("{}", event.path.try_to_rel().display());
            } else {
                cu::warn!("not formatted: {}", event.path.try_to_rel().display());
            }
            *check_error = true;
        }
    }
}

struct Event {
    path: PathBuf,
    kind: EventKind,
}

enum EventKind {
    /// path is skipped (empty file or not UTF-8)
    Skipped,
    /// path is already formatted after checking
    NoChange,
    /// formatted result written to file
    Formatted,
    /// the file needs formatting, but result is not written to file
    NeedsFormat,
}

async fn process_file(path: &Path, end: LineEnd, check: bool) -> Message {
    if !path.is_file() {
        return Ok(None);
    }
    let event_kind = match process_file_internal(path, end, check).await {
        Ok(e) => e,
        Err(e) => return Err(format!("error {}: {:?}", path.try_to_rel().display(), e)),
    };
    Ok(Some(Event {
        path: path.to_path_buf(),
        kind: event_kind,
    }))
}

/// Process a single file
async fn process_file_internal(path: &Path, end: LineEnd, check: bool) -> cu::Result<EventKind> {
    let bytes = cu::fs::co_read(path).await?;
    let len = bytes.len();
    if len == 0 {
        return Ok(EventKind::Skipped);
    }
    let utf8 = match str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => {
            return Ok(EventKind::Skipped);
        }
    };
    let has_trailing_newline = matches!(bytes[len - 1], b'\n' | b'\r');

    let mut lines = utf8.lines();
    let mut formatted = String::new();
    if let Some(first) = lines.next() {
        formatted.push_str(first);
        for line in lines {
            for line in line.split('\r') {
                match end {
                    LineEnd::Lf => formatted.push('\n'),
                    LineEnd::Crlf => formatted.push_str("\r\n"),
                }
                formatted.push_str(line);
            }
        }
    }
    if has_trailing_newline {
        match end {
            LineEnd::Lf => formatted.push('\n'),
            LineEnd::Crlf => formatted.push_str("\r\n"),
        }
    }
    if utf8 == formatted {
        return Ok(EventKind::NoChange);
    }

    if check {
        return Ok(EventKind::NeedsFormat);
    }

    cu::fs::co_write(path, formatted).await?;
    Ok(EventKind::Formatted)
}
