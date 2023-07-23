// WIP

// Server doesn't need to validate tag or otherwise worry about it, but can
// just return unchanged?
// Is a task per invocation undeseriable?
// - in particular, having to lock TcpStream

use std::{future::Future, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

use crate::{Header, Message};

pub struct Replied(());

pub struct Replier {
    stream: Arc<Mutex<TcpStream>>,
    tag: u16,
}

impl Replier {
    // XXX trait for just reply messages?
    async fn reply<'a, T: Message<'a>>(self, message: T) -> Replied {
        let header = Header::for_message(&message, self.tag);
        let mut stream = self.stream.lock().await;
        header.async_write(&mut *stream).await;
        // XXX send message on socket
        Replied(())
    }
}

pub struct TokioServer {
    listener: TcpListener,
}

impl TokioServer {
    // XXX distinguish connections?
    async fn serve<Fut: Future<Output = Replied>, F: Fn(Replier) -> Fut + Sync>() {}
}

// Handle connections similarly to how a web server would?
// - best practice for tokio web server?
