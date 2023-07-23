use std::io;

use crate::{Error, Message, RError, TMessage};

struct Header {
    size: u32,
    type_: u8,
    tag: u16,
}

impl Header {
    #[inline]
    fn from_array(header: [u8; 7]) -> Self {
        Self {
            size: u32::from_le_bytes([header[0], header[1], header[2], header[3]]),
            type_: header[4],
            tag: u16::from_le_bytes([header[5], header[6]]),
        }
    }

    #[inline]
    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write(&self.size.to_le_bytes())?;
        writer.write(&[self.type_])?;
        writer.write(&self.tag.to_le_bytes())?;
        Ok(())
    }
}

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
        let header = Header {
            size: 7 + request.size() as u32,
            type_: Req::TYPE as u8,
            tag,
        };
        header.write(&mut self.transport)?;
        request.write(&mut self.transport)?;

        let mut header_buf = [0; 7];
        self.transport.read_exact(&mut header_buf)?;
        let reply_header = Header::from_array(header_buf);
        if reply_header.tag != tag {
            return Err(Error::UnrecognizedTag(reply_header.tag));
        }
        self.buffer.resize(reply_header.size as usize - 7, 0); // XXX efficiency
                                                               // XXX handle error return
        self.transport.read_exact(&mut self.buffer)?;
        if reply_header.type_ == Req::RMessage::TYPE as u8 {
            Req::RMessage::parse(&self.buffer)
        } else if reply_header.type_ == RError::TYPE as u8 {
            Err(Error::Protocol(
                RError::parse(&self.buffer)?.ename.to_string(),
            ))
        } else {
            Err(Error::UnexpectedType(reply_header.type_))
        }
    }
}
