// http://man.cat-v.org/plan_9/5/intro

use std::{error, fmt, io, str};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Utf8(str::Utf8Error),
    MessageLength,
    UnrecognizedTag(u32),
    UnexpectedType(u8),
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self) // XXX
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<str::Utf8Error> for Error {
    fn from(error: str::Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

// Defined by fcall.h
#[repr(u8)]
pub enum MessageType {
    TVersion = 100,
    RVersion = 101,
    TAuth = 102,
    RAuth = 103,
    TAttach = 104,
    RAttach = 105,
    // Terror (106) is invalid
    RError = 107,
    TFlush = 108,
    RFlush = 109,
    TWalk = 110,
    RWalk = 111,
    TOpen = 112,
    ROpen = 113,
    TCreate = 114,
    RCreate = 115,
    TRead = 116,
    RRead = 117,
    TWrite = 118,
    RWrite = 119,
    TClunk = 120,
    RClunk = 121,
    TRemove = 122,
    RRemove = 123,
    TStat = 124,
    RStat = 125,
    TWstat = 126,
    RWstat = 127,
}

#[derive(Clone, Debug, Default)]
pub struct Qid([u8; 13]);

#[derive(Clone, Debug, Default)]
pub struct Fid(u32);

pub trait Message<'a>: Sized {
    const TYPE: MessageType;
    /// Parse message pody
    fn parse(body: &'a [u8]) -> Result<Self, Error>;
    /// Byte length of serialized message body
    fn size(&self) -> usize;
    /// Write serialized message body
    fn write<T: io::Write>(&self, writer: T) -> io::Result<()>;
}

macro_rules! impl_empty_message {
    ($type:ident, $id:path) => {
        impl<'a> Message<'a> for $type {
            const TYPE: MessageType = $id;

            fn parse(body: &'a [u8]) -> Result<Self, Error> {
                if body.is_empty() {
                    Ok($type)
                } else {
                    Err(Error::MessageLength)
                }
            }

            fn size(&self) -> usize {
                0
            }

            fn write<T: io::Write>(&self, _: T) -> io::Result<()> {
                Ok(())
            }
        }
    };
}

pub trait TMessage<'a>: Message<'a> {
    type RMessage<'b>: Message<'b>;
}

#[derive(Clone, Debug, Default)]
pub struct TVersion<'a> {
    pub msize: u32,
    pub version: &'a str,
}

impl<'a> Message<'a> for TVersion<'a> {
    const TYPE: MessageType = MessageType::TVersion;

    fn parse(_body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 2 + self.version.len()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write(&self.msize.to_le_bytes())?;
        writer.write(&(self.version.len() as u16).to_le_bytes())?;
        writer.write(self.version.as_bytes())?;
        Ok(())
    }
}

impl<'a> TMessage<'a> for TVersion<'a> {
    type RMessage<'b> = RVersion<'b>;
}

#[derive(Clone, Debug, Default)]
pub struct RVersion<'a> {
    pub msize: u32,
    pub version: &'a str,
}

impl<'a> Message<'a> for RVersion<'a> {
    const TYPE: MessageType = MessageType::RVersion;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() < 6 {
            return Err(Error::MessageLength);
        }
        let msize = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let version_len = u16::from_le_bytes([body[4], body[5]]);
        if body.len() != 6 + usize::from(version_len) {
            return Err(Error::MessageLength);
        }
        let version = str::from_utf8(&body[6..])?;
        Ok(Self { msize, version })
    }

    fn size(&self) -> usize {
        4 + 2 + self.version.len()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write(&self.msize.to_le_bytes())?;
        writer.write(&(self.version.len() as u16).to_le_bytes())?;
        writer.write(self.version.as_bytes())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct TAuth<'a> {
    pub afid: Fid,
    pub uname: &'a str,
    pub aname: &'a str,
}

#[derive(Clone, Debug, Default)]
pub struct RAuth {
    pub aqid: Qid,
}

#[derive(Clone, Debug, Default)]
pub struct RError<'a> {
    pub ename: &'a str,
}

#[derive(Clone, Debug, Default)]
pub struct TAttach<'a> {
    pub fid: Fid,
    pub afid: Fid,
    pub uname: &'a str,
    pub aname: &'a str,
}

#[derive(Clone, Debug, Default)]
pub struct RAttach {
    pub qid: Qid,
}

#[derive(Clone, Debug, Default)]
pub struct TWalk {
    pub fid: Fid,
    pub newfid: Fid,
    pub nwname: u16,
    // XXX ?
}

#[derive(Clone, Debug, Default)]
pub struct RWalk {
    pub nwqid: u16,
    // XXX
}

#[derive(Clone, Debug, Default)]
pub struct TOpen {
    pub fid: Fid,
    pub mode: u8,
}

#[derive(Clone, Debug, Default)]
pub struct ROpen {
    pub qid: Qid,
    pub iounit: u32,
}

#[derive(Clone, Debug, Default)]
pub struct TCreate<'a> {
    pub fid: Fid,
    pub name: &'a str,
    pub perm: u32,
    pub mode: u8,
}

#[derive(Clone, Debug, Default)]
pub struct RCreate {
    pub qid: Qid,
    pub iounit: u32,
}

#[derive(Clone, Debug, Default)]
pub struct TRead {
    pub fid: Fid,
    pub offset: u64,
    pub count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct RRead<'a> {
    pub data: &'a [u8],
}

#[derive(Clone, Debug, Default)]
pub struct TWrite<'a> {
    pub fid: Fid,
    pub offset: u64,
    pub data: &'a [u8],
}

#[derive(Clone, Debug, Default)]
pub struct RWrite {
    pub count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct TClunk {
    pub fid: Fid,
}

#[derive(Clone, Debug, Default)]
pub struct RClunk;

impl_empty_message!(RClunk, MessageType::RClunk);

#[derive(Clone, Debug, Default)]
pub struct TRemove {
    pub fid: Fid,
}

#[derive(Clone, Debug, Default)]
pub struct RRemove;

impl_empty_message!(RRemove, MessageType::RRemove);

#[derive(Clone, Debug, Default)]
pub struct TStat {
    pub fid: Fid,
}

#[derive(Clone, Debug, Default)]
pub struct RStat<'a> {
    pub stat: &'a [u8],
}

#[derive(Clone, Debug, Default)]
pub struct TWStat<'a> {
    pub fid: Fid,
    pub stat: &'a [u8],
}

#[derive(Clone, Debug, Default)]
pub struct RWStat;

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
    ) -> io::Result<Req::RMessage<'_>> {
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
        assert!(resp_tag == tag); // XXX assert
        assert!(type_ == Req::RMessage::TYPE as u8); // XXX assert
        self.buffer.resize(size as usize - 7, 0); // XXX efficiency
                                                  // XXX handle error return
        self.transport.read_exact(&mut self.buffer)?;
        // XXX unwrap
        Ok(Req::RMessage::parse(&self.buffer).unwrap())
    }
}
