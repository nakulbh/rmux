//! PTY session management for the Tauri backend.
//!
//! Wraps `portable-pty` into a session that can be spawned, written to,
//! resized, and polled for output. A background thread reads PTY output
//! and pushes it into an internal buffer that the frontend polls via
//! `read_terminal`.

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;

/// Errors that can occur during PTY operations.
#[derive(Error, Debug)]
pub enum PtySessionError {
    #[error("Failed to open PTY: {0}")]
    OpenPty(#[source] anyhow::Error),
    #[error("Failed to spawn child process: {0}")]
    SpawnProcess(#[source] anyhow::Error),
    #[error("Failed to write to PTY: {0}")]
    WriteError(#[source] std::io::Error),
    #[error("Failed to resize PTY: {0}")]
    ResizeError(#[source] anyhow::Error),
    #[error("Failed to acquire PTY I/O: {0}")]
    IoSetup(#[source] anyhow::Error),
    #[error("PTY is closed")]
    Closed,
}

/// The result type for PTY session operations.
pub type PtySessionResult<T> = Result<T, PtySessionError>;

/// Shared output buffer for a PTY session.
#[derive(Debug, Clone)]
pub struct PtyOutputBuffer {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl PtyOutputBuffer {
    fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(Vec::new())) }
    }

    /// Append bytes to the buffer.
    pub fn push(&self, data: &[u8]) {
        if let Ok(mut buf) = self.inner.lock() {
            buf.extend_from_slice(data);
        }
    }

    /// Take all buffered bytes, clearing the buffer.
    pub fn take(&self) -> Vec<u8> {
        self.inner.lock().map_or_else(|_| Vec::new(), |mut buf| std::mem::take(&mut *buf))
    }
}

/// A single PTY session with background reader thread.
pub struct PtySession {
    /// The master PTY (for resize).
    master: Box<dyn MasterPty + Send>,
    /// Writer for PTY input.
    writer: Option<Box<dyn Write + Send>>,
    /// Cloned child killer for signaling.
    child_killer: Box<dyn ChildKiller + Send>,
    /// Shared output buffer populated by the background reader.
    output: PtyOutputBuffer,
    /// Whether the session has been closed.
    closed: bool,
}

impl PtySession {
    /// Spawn a new shell in a PTY with the given dimensions.
    pub fn spawn(cols: u16, rows: u16) -> PtySessionResult<Self> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            #[cfg(unix)]
            {
                "/bin/sh".to_string()
            }
            #[cfg(not(unix))]
            {
                "cmd.exe".to_string()
            }
        });

        let pty_system = portable_pty::native_pty_system();
        let pty_size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        let pair = pty_system.openpty(pty_size).map_err(PtySessionError::OpenPty)?;

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");

        #[cfg(unix)]
        {
            if let Ok(home) = std::env::var("HOME") {
                cmd.cwd(home);
            }
        }

        let child = pair.slave.spawn_command(cmd).map_err(PtySessionError::SpawnProcess)?;
        let child_killer = child.clone_killer();

        let reader = pair.master.try_clone_reader().map_err(PtySessionError::IoSetup)?;
        let writer = pair.master.take_writer().map_err(PtySessionError::IoSetup)?;

        let output = PtyOutputBuffer::new();
        let output_clone = output.clone();

        // Spawn a background thread to read PTY output.
        thread::Builder::new()
            .name("rmux-pty-reader".to_string())
            .spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => output_clone.push(&buf[..n]),
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| PtySessionError::IoSetup(anyhow::anyhow!(e)))?;

        Ok(Self {
            master: pair.master,
            writer: Some(writer),
            child_killer,
            output,
            closed: false,
        })
    }

    /// Write input bytes to the PTY.
    pub fn write(&mut self, data: &[u8]) -> PtySessionResult<()> {
        if self.closed {
            return Err(PtySessionError::Closed);
        }
        if let Some(ref mut writer) = self.writer {
            writer.write_all(data).map_err(PtySessionError::WriteError)?;
            writer.flush().map_err(PtySessionError::WriteError)?;
        }
        Ok(())
    }

    /// Read all buffered output from the PTY.
    pub fn read(&mut self) -> Vec<u8> {
        self.output.take()
    }

    /// Resize the PTY to new dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16) -> PtySessionResult<()> {
        if self.closed {
            return Err(PtySessionError::Closed);
        }
        let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        self.master.resize(size).map_err(PtySessionError::ResizeError)?;
        Ok(())
    }

    /// Kill the child process and mark the session as closed.
    pub fn close(&mut self) -> std::io::Result<()> {
        self.closed = true;
        self.writer = None;
        self.child_killer.kill()
    }

    /// Check if the session is still open.
    pub fn is_open(&self) -> bool {
        !self.closed
    }
}

impl std::fmt::Debug for PtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtySession")
            .field("open", &self.is_open())
            .finish()
    }
}
