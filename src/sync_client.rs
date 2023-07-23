use std::io;

use crate::{Error, Message, RError, TMessage};

/// Simple client that sends a command then blocks until it gets a reply
pub struct SyncClient<T: io::Read + io::Write> {
    transport: T,
    buffer: Vec<u8>,
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
        self.transport
            .write(&(7 + request.size() as u32).to_le_bytes())?;
        self.transport.write(&[Req::TYPE as u8])?;
        self.transport.write(&tag.to_le_bytes())?;
        request.write(&mut self.transport)?;

        let mut header = [0; 7];
        self.transport.read_exact(&mut header)?;
        let size = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let type_ = header[4];
        let resp_tag = u16::from_le_bytes([header[5], header[6]]);
        if resp_tag != tag {
            return Err(Error::UnrecognizedTag(resp_tag));
        }
        self.buffer.resize(size as usize - 7, 0); // XXX efficiency
                                                  // XXX handle error return
        self.transport.read_exact(&mut self.buffer)?;
        if type_ == Req::RMessage::TYPE as u8 {
            Req::RMessage::parse(&self.buffer)
        } else if type_ == RError::TYPE as u8 {
            Err(Error::Protocol(
                RError::parse(&self.buffer)?.ename.to_string(),
            ))
        } else {
            Err(Error::UnexpectedType(type_))
        }
    }
}
