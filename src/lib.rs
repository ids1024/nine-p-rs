// http://man.cat-v.org/plan_9/5/intro

// TODO: no-copy parsing from virtio ring buffer?
// - even if we copy soon after, want to avoid copying twice. at least.

use std::{io, str};

mod error;
pub use error::Error;
mod sync_client;
pub use sync_client::SyncClient;

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
pub struct Fid(pub u32);

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

impl<'a> Message<'a> for TAuth<'a> {
    const TYPE: MessageType = MessageType::TAuth;

    fn parse(_body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 2 + self.uname.len() + 2 + self.aname.len()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write(&self.afid.0.to_le_bytes())?;
        writer.write(&(self.uname.len() as u16).to_le_bytes())?;
        writer.write(self.uname.as_bytes())?;
        writer.write(&(self.aname.len() as u16).to_le_bytes())?;
        writer.write(self.aname.as_bytes())?;
        Ok(())
    }
}

impl<'a> TMessage<'a> for TAuth<'a> {
    type RMessage<'b> = RAuth;
}

#[derive(Clone, Debug, Default)]
pub struct RAuth {
    pub aqid: Qid,
}

impl<'a> Message<'a> for RAuth {
    const TYPE: MessageType = MessageType::RAuth;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() != 13 {
            return Err(Error::MessageLength);
        }
        Ok(RAuth {
            aqid: Qid([
                body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7], body[8],
                body[9], body[10], body[11], body[12],
            ]),
        })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, _writer: T) -> io::Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug, Default)]
pub struct RError<'a> {
    pub ename: &'a str,
}

impl<'a> Message<'a> for RError<'a> {
    const TYPE: MessageType = MessageType::RError;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() < 2 {
            return Err(Error::MessageLength);
        }
        if body.len() != 2 + usize::from(u16::from_le_bytes([body[0], body[1]])) {
            return Err(Error::MessageLength);
        }
        let ename = str::from_utf8(&body[2..])?;
        Ok(RError { ename })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, _writer: T) -> io::Result<()> {
        todo!()
    }
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
