//! One Scope window per session.
//!
//! Opening a package file with Scope launches `scope-gpui <file>`. When Scope is
//! already running, the new process hands the path to the running window over
//! a Unix socket in the private `$XDG_RUNTIME_DIR` and exits, so the install
//! preview appears in the existing window. The receiver treats every path as
//! untrusted input: it only opens a preview, which validates the file.

use std::io::{Read as _, Write as _};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::channel::mpsc;

const MAX_MESSAGE: u64 = 4096;

/// A request from another launch: a package file to preview, or just "show".
pub type Request = Option<PathBuf>;

fn socket_path() -> Option<PathBuf> {
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
    runtime
        .is_absolute()
        .then(|| runtime.join("scope-gpui.sock"))
}

/// The file Scope was asked to open, if any.
pub fn requested_file() -> Option<PathBuf> {
    std::env::args_os()
        .skip(1)
        .find(|arg| !arg.as_bytes().starts_with(b"-"))
        .map(PathBuf::from)
        .and_then(|path| std::path::absolute(path).ok())
}

/// Hand the request to a running Scope. `true` means this process can exit.
pub fn forward(request: &Request) -> bool {
    let Some(socket) = socket_path() else {
        return false;
    };
    let Ok(mut stream) = UnixStream::connect(&socket) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let bytes = request
        .as_deref()
        .map(|path| path.as_os_str().as_bytes().to_vec())
        .unwrap_or_default();
    stream.write_all(&bytes).is_ok()
}

/// Accept requests from later launches. `None` when no runtime dir exists.
pub fn listen() -> Option<mpsc::UnboundedReceiver<Request>> {
    let socket = socket_path()?;
    // A socket file left by a crashed instance refuses connections.
    if UnixStream::connect(&socket).is_err() {
        let _ = std::fs::remove_file(&socket);
    }
    let listener = UnixListener::bind(&socket).ok()?;
    let (tx, rx) = mpsc::unbounded();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if let Some(request) = read_request(stream) {
                if tx.unbounded_send(request).is_err() {
                    break;
                }
            }
        }
    });
    Some(rx)
}

fn read_request(stream: UnixStream) -> Option<Request> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut bytes = Vec::new();
    stream.take(MAX_MESSAGE + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_MESSAGE {
        return None;
    }
    if bytes.is_empty() {
        return Some(None);
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    Path::new(&path).is_absolute().then_some(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_and_relative_paths_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("s.sock");
        let listener = UnixListener::bind(&socket).unwrap();

        for (sent, expected) in [
            (
                b"/home/u/Downloads/App.AppImage".to_vec(),
                Some(Some(PathBuf::from("/home/u/Downloads/App.AppImage"))),
            ),
            (Vec::new(), Some(None)),
            (b"relative/App.AppImage".to_vec(), None),
        ] {
            let mut client = UnixStream::connect(&socket).unwrap();
            client.write_all(&sent).unwrap();
            drop(client);
            let (stream, _) = listener.accept().unwrap();
            assert_eq!(read_request(stream), expected);
        }
    }
}
