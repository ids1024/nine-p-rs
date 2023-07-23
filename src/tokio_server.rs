// WIP

// Server doesn't need to validate tag or otherwise worry about it, but can
// just return unchanged?

use tokio::net::{TcpListener, TcpStream};

pub struct TokioServer {
    listener: TcpListener,
}

// Handle connections similarly to how a web server would?
// - best practice for tokio web server?
