// http://man.cat-v.org/plan_9/5/intro

use std::{error, fmt, io};

#[derive(Debug)]
enum Error {
    Io(io::Error),
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

pub trait Message: Sized {
    const TYPE: MessageType;
    /// Parse message pody
    fn parse(body: &[u8]) -> Result<Self, ()>;
    /// Byte length of serialized message body
    fn size(&self) -> usize;
    /// Write serialized message body
    fn write<T: io::Write>(&self, writer: T) -> io::Result<usize>;
}

pub trait TMessage: Message {
    type RMessage<'l>: Message;
}

#[derive(Clone, Debug, Default)]
pub struct TVersion<'a> {
    pub msize: u32,
    pub version: &'a str
}

#[derive(Clone, Debug, Default)]
pub struct RVersion<'a> {
    pub msize: u32,
    pub version: &'a str
}

#[derive(Clone, Debug, Default)]
pub struct TAuth<'a> {
    pub afid: Fid,
    pub uname: &'a str,
    pub aname: &'a str,
}

#[derive(Clone, Debug, Default)]
pub struct RAuth {
    pub aqid: Qid
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
    pub qid: Qid
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
    pub data: &'a [u8]
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

#[derive(Clone, Debug, Default)]
pub struct TRemove {
    pub fid: Fid,
}

#[derive(Clone, Debug, Default)]
pub struct RRemove;

#[derive(Clone, Debug, Default)]
pub struct TStat {
    pub fid: Fid,
}

#[derive(Clone, Debug, Default)]
pub struct RStat<'a> {
    pub stat: &'a [u8]
}

#[derive(Clone, Debug, Default)]
pub struct TWStat<'a> {
    pub fid: Fid,
    pub stat: &'a [u8]
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
    pub fn send<Req: TMessage>(&mut self, tag: u16, request: Req) -> io::Result<Req::RMessage<'_>> {
        self.transport.write(&(7 +  request.size() as u32).to_le_bytes())?;
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
        self.buffer.resize(size as usize, 0); // XXX efficiency
        // XXX handle error return
        self.transport.read_exact(&mut self.buffer)?;
        // XXX unwrap
        Ok(Req::RMessage::parse(&self.buffer).unwrap())
    }
}
