// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Pistonite

use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use cu::pre::*;
use tungstenite::{Error as WsError, Message, WebSocket};

#[derive(clap::Parser)]
pub struct Cli {
    /// The port to open at
    #[clap(short, long, default_value = "8881")]
    pub port: u16,
    #[clap(flatten)]
    pub flags: cu::cli::Flags,
}

pub fn run(cli: Cli) -> cu::Result<()> {
    // use 0.0.0.0 to allow computers in the same network to send to us
    // (which is the whole point of this tool)
    let address = format!("0.0.0.0:{}", cli.port);
    let server = cu::check!(TcpListener::bind(&address), "failed to bind to {address}")?;
    cu::info!("server started on {address}");
    let (ws_send, ws_recv) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    // ctrl-c handler
    {
        let running = Arc::clone(&running);
        let attempted = AtomicBool::new(false);
        let client_address = format!("ws://127.0.0.1:{}", cli.port);
        if let Err(e) = cu::cli::add_global_ctrlc_handler(move || {
            // CAS probably not needed, just in case :)
            if attempted
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                cu::warn!("requesting graceful termination... Ctrl-C again to force exit");
                running.store(false, Ordering::Release);
                // make a new connection to unblock server thread
                if let Err(e) = tungstenite::client::connect(&client_address) {
                    cu::error!("failed to make new connection to server: {e:?}");
                    cu::hint!("Ctrl-C again to force exit");
                }
                return;
            }
            cu::warn!("force exitting...");
            std::process::exit(19937);
        }) {
            cu::warn!("failed to set Ctrl-C handler, graceful termination is not possible: {e:?}");
        }
    }

    // accepting thread
    let accepting_thread = {
        let running = Arc::clone(&running);
        std::thread::spawn(move || {
            let mut id = 1;
            for stream in server.incoming() {
                let stream = match stream {
                    Err(e) => {
                        cu::error!("failed to accept new connection: {e:?}");
                        continue;
                    }
                    Ok(x) => x,
                };
                let ws = match tungstenite::accept(stream) {
                    Err(e) => {
                        cu::error!("failed to accept new websocket connection: {e:?}");
                        continue;
                    }
                    Ok(x) => x,
                };
                let _ = ws_send.send(Conn { id, ws });
                id += 1;
                if !running.load(Ordering::Acquire) {
                    return;
                }
            }
        })
    };

    // main loop
    let mut connections = vec![];
    let mut idle_ms = 50;
    let max_idle_ms = 2000;
    loop {
        // receiving queue
        if let Ok(conn) = ws_recv.try_recv() {
            connections.push(conn);
        }
        let closed = !running.load(Ordering::Acquire);
        let mut worked = false;
        let mut closed_index = connections.len();
        for (i, conn) in connections.iter_mut().enumerate() {
            let id = conn.id;
            if closed {
                match conn.ws.close(None) {
                    Ok(_) | Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {}
                    Err(e) => {
                        cu::error!("failed to close connection {id}: {e:?}");
                    }
                }
            }
            match conn.ws.read() {
                Ok(Message::Text(bytes)) => {
                    set_clipboard_bytes(id, bytes.as_ref());
                    worked = true;
                }
                Ok(Message::Binary(bytes)) => {
                    set_clipboard_bytes(id, bytes.as_ref());
                    worked = true;
                }
                Ok(msg) => {
                    cu::debug!("[{id}] received: {msg:?}");
                    worked = true;
                }
                Err(WsError::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                    cu::debug!("[{id}] idle: would block");
                }
                Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                    closed_index = i;
                }
                Err(WsError::Io(e))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionReset
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionAborted
                    ) =>
                {
                    closed_index = i;
                }
                Err(e) => {
                    cu::error!("[{id}] read error: {e:?}");
                    // send a close frame
                    let _ = conn.ws.close(None);
                }
            }
        }
        if closed_index != connections.len() {
            let conn = connections.swap_remove(closed_index);
            if running.load(Ordering::Acquire) {
                // don't print closed message after closing
                cu::info!("[{}] closed", conn.id);
            }
        }
        if connections.is_empty() && closed {
            break;
        }
        if !worked {
            std::thread::sleep(Duration::from_millis(idle_ms));
            // linear back off
            idle_ms = max_idle_ms.min(idle_ms + 50);
        } else {
            idle_ms = 50;
        }
    }
    let _ = accepting_thread.join();
    cu::info!("server closed");
    cu::lv::disable_print_time();
    Ok(())
}

fn set_clipboard_bytes(id: usize, bytes: &[u8]) {
    if let Err(e) = set_clipboard_bytes_internal(id, bytes) {
        cu::error!("[{id}] failed to set clipboard: {e:?}");
    }
}
fn set_clipboard_bytes_internal(id: usize, mut bytes: &[u8]) -> cu::Result<()> {
    cu::debug!("[{id}] received {} bytes", bytes.len());
    let mut line_count = 0;
    let mut utf8_content = String::new();
    loop {
        if bytes.is_empty() {
            break;
        }
        match bytes.iter().position(|x| *x == 0) {
            Some(null_i) => {
                if null_i == 0 {
                    utf8_content.push('\n');
                    bytes = &bytes[1..];
                }
                line_count += 1;
                match str::from_utf8(&bytes[..null_i]) {
                    Ok(s) => {
                        utf8_content.push_str(s);
                        utf8_content.push('\n');
                    }
                    Err(e) => {
                        cu::error!("[{id}] failed to decode line {line_count}: {e:?}");
                    }
                }
                bytes = &bytes[null_i + 1..];
            }
            None => {
                line_count += 1;
                match str::from_utf8(bytes) {
                    Ok(s) => {
                        utf8_content.push_str(s);
                    }
                    Err(e) => {
                        cu::error!("[{id}] failed to decode line {line_count}: {e:?}");
                    }
                }
                break;
            }
        }
    }
    cu::debug!("[{id}] decoded {} bytes, copying...", utf8_content.len());
    if let Err(ec) = clipboard_win::set_clipboard(clipboard_win::formats::Unicode, &utf8_content) {
        cu::bail!("failed to set clipboard: error code: {ec}");
    }
    cu::info!(
        "[{id}] copied {line_count} lines ({} bytes)",
        utf8_content.len()
    );
    Ok(())
}

struct Conn {
    id: usize,
    ws: WebSocket<TcpStream>,
}
