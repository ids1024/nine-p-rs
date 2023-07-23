use std::io;

use crate::{Error, Header, Message, RError, TMessage};

/// Simple client that sends a command then blocks until it gets a reply
pub struct SyncClient<T: io::Read + io::Write> {
    transport: T,
    buffer: Vec<u8>,
}

/// Parse a reply of type `Reply`, or a `RError`.
///
/// Returns `UnexpectedType` if the message has any other type.
fn parse_reply<'a, Reply: Message<'a>>(header: &Header, body: &'a [u8]) -> Result<Reply, Error> {
    if header.type_ == Reply::TYPE as u8 {
        Reply::parse(body)
    } else if header.type_ == RError::TYPE as u8 {
        Err(Error::Protocol(RError::parse(body)?.ename.to_string()))
    } else {
        Err(Error::UnexpectedType(header.type_))
    }
}

impl<T: io::Read + io::Write> SyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            buffer: Vec::new(),
        }
    }

    // XXX return lifetime?
    // XXX for read, how could we pass a buffer to read into?
    pub fn send<'a, Req: TMessage<'a>>(
        &mut self,
        tag: u16,
        request: Req,
    ) -> Result<Req::RMessage<'_>, Error> {
        let header = Header::for_message(&request, tag);
        header.write(&mut self.transport)?;
        request.write(&mut self.transport)?;

        let mut header_buf = [0; 7];
        self.transport.read_exact(&mut header_buf)?;
        let reply_header = Header::from_array(header_buf);
        if reply_header.tag != tag {
            return Err(Error::UnrecognizedTag(reply_header.tag));
        }
        self.buffer.resize(reply_header.size as usize - 7, 0); // XXX efficiency
        self.transport.read_exact(&mut self.buffer)?;
        parse_reply(&reply_header, &self.buffer)
    }
}
