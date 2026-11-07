//! Windows tray activation of an already-running GUI. Connecting is the entire
//! command: there is no unversioned payload or device identity to decode.

use std::io;

use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::{Listener, prelude::*};

fn endpoint() -> io::Result<interprocess::local_socket::Name<'static>> {
    crate::transport::namespaced_endpoint("mousekey-studio-gui-tray.sock")
}

/// Listen for tray activation requests while the GUI holds its singleton lock.
///
/// # Errors
/// Returns an error if the named pipe cannot be created.
pub fn bind() -> io::Result<Listener> {
    ListenerOptions::new().name(endpoint()?).create_tokio()
}

/// Wait for a tray activation request.
///
/// # Errors
/// Returns the listener's accept error.
pub async fn accept(listener: &Listener) -> io::Result<()> {
    listener.accept().await.map(drop)
}

/// Ask the running GUI to open its configured tray destination.
///
/// # Errors
/// Returns an error when no GUI listener is available.
pub fn notify() -> io::Result<()> {
    use interprocess::local_socket::prelude::*;
    interprocess::local_socket::Stream::connect(endpoint()?).map(drop)
}
