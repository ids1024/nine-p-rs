// http://man.cat-v.org/plan_9/5/intro

// TODO: no-copy parsing from virtio ring buffer?
// - even if we copy soon after, want to avoid copying twice. at least.
// TODO: avoid multiple small writes? buffer without copy?
// - maybe the trick is to buffer everything other than large payloads. or use readv/writev.
// Many messages are fixed size. Things like Walk should be relatively small. Read/Write may be
// huge.

use std::{io, str};

mod error;
pub use error::Error;
mod header;
use header::Header;
mod sync_client;
#[cfg(feature = "tokio")]
mod tokio_server;
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
    TWStat = 126,
    RWStat = 127,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Qid([u8; 13]);

unsafe impl bytemuck::Pod for Qid {}
unsafe impl bytemuck::Zeroable for Qid {}

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
        writer.write_all(&self.msize.to_le_bytes())?;
        writer.write_all(&(self.version.len() as u16).to_le_bytes())?;
        writer.write_all(self.version.as_bytes())?;
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
        writer.write_all(&self.msize.to_le_bytes())?;
        writer.write_all(&(self.version.len() as u16).to_le_bytes())?;
        writer.write_all(self.version.as_bytes())?;
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
        writer.write_all(&self.afid.0.to_le_bytes())?;
        writer.write_all(&(self.uname.len() as u16).to_le_bytes())?;
        writer.write_all(self.uname.as_bytes())?;
        writer.write_all(&(self.aname.len() as u16).to_le_bytes())?;
        writer.write_all(self.aname.as_bytes())?;
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

impl<'a> Message<'a> for TAttach<'a> {
    const TYPE: MessageType = MessageType::TAttach;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 4 + 2 + self.uname.len() + 2 + self.aname.len()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        writer.write_all(&self.afid.0.to_le_bytes())?;
        writer.write_all(&(self.uname.len() as u16).to_le_bytes())?;
        writer.write_all(self.uname.as_bytes())?;
        writer.write_all(&(self.aname.len() as u16).to_le_bytes())?;
        writer.write_all(self.aname.as_bytes())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RAttach {
    pub qid: Qid,
}

impl<'a> Message<'a> for RAttach {
    const TYPE: MessageType = MessageType::RAttach;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() != 13 {
            return Err(Error::MessageLength);
        }
        Ok(RAttach {
            qid: Qid([
                body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7], body[8],
                body[9], body[10], body[11], body[12],
            ]),
        })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
}

impl<'a> TMessage<'a> for TAttach<'a> {
    type RMessage<'b> = RAttach;
}

#[derive(Clone, Debug, Default)]
pub struct TWalk<'a> {
    pub fid: Fid,
    pub newfid: Fid,
    // XXX use &[&str] in argument, but `Vec` for return?
    pub wnames: Vec<&'a str>,
}

impl<'a> Message<'a> for TWalk<'a> {
    const TYPE: MessageType = MessageType::TWalk;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 4 + 2 + 2 * self.wnames.len() + self.wnames.iter().map(|x| x.len()).sum::<usize>()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        writer.write_all(&self.newfid.0.to_le_bytes())?;
        writer.write_all(&(self.wnames.len() as u16).to_le_bytes())?;
        for wname in &self.wnames {
            writer.write_all(&(wname.len() as u16).to_le_bytes())?;
            writer.write_all(wname.as_bytes())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RWalk<'a> {
    qids: &'a [Qid],
}

impl<'a> Message<'a> for RWalk<'a> {
    const TYPE: MessageType = MessageType::RWalk;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() < 2 {
            return Err(Error::MessageLength);
        }
        let len = u16::from_le_bytes([body[0], body[1]]);
        if body.len() < 2 + 13 * len as usize {
            return Err(Error::MessageLength);
        }
        Ok(RWalk {
            qids: bytemuck::cast_slice(&body[2..]),
        })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
}

impl<'a> TMessage<'a> for TWalk<'a> {
    type RMessage<'b> = RWalk<'b>;
}

#[derive(Clone, Debug, Default)]
pub struct TOpen {
    pub fid: Fid,
    pub mode: u8,
}

impl<'a> Message<'a> for TOpen {
    const TYPE: MessageType = MessageType::TOpen;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 1
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        writer.write_all(&[self.mode])?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ROpen {
    pub qid: Qid,
    pub iounit: u32,
}

impl<'a> Message<'a> for ROpen {
    const TYPE: MessageType = MessageType::ROpen;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() != 13 + 4 {
            return Err(Error::MessageLength);
        }
        let qid = Qid([
            body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7], body[8],
            body[9], body[10], body[11], body[12],
        ]);
        let iounit = u32::from_le_bytes([body[13], body[14], body[15], body[16]]);
        Ok(ROpen { qid, iounit })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
}

impl<'a> TMessage<'a> for TOpen {
    type RMessage<'b> = ROpen;
}

#[derive(Clone, Debug, Default)]
pub struct TCreate<'a> {
    pub fid: Fid,
    pub name: &'a str,
    pub perm: u32,
    pub mode: u8,
}

impl<'a> Message<'a> for TCreate<'a> {
    const TYPE: MessageType = MessageType::TCreate;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 2 + self.name.len() + 4 + 1
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        writer.write_all(&(self.name.len() as u16).to_le_bytes())?;
        writer.write_all(self.name.as_bytes())?;
        writer.write_all(&self.perm.to_le_bytes())?;
        writer.write_all(&[self.mode])?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RCreate {
    pub qid: Qid,
    pub iounit: u32,
}

impl<'a> Message<'a> for RCreate {
    const TYPE: MessageType = MessageType::RCreate;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() != 13 + 4 {
            return Err(Error::MessageLength);
        }
        let qid = Qid([
            body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7], body[8],
            body[9], body[10], body[11], body[12],
        ]);
        let iounit = u32::from_le_bytes([body[13], body[14], body[15], body[16]]);
        Ok(RCreate { qid, iounit })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
}

impl<'a> TMessage<'a> for TCreate<'a> {
    type RMessage<'b> = RCreate;
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

impl<'a> Message<'a> for TClunk {
    const TYPE: MessageType = MessageType::TClunk;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        Ok(())
    }
}

impl<'a> TMessage<'a> for TClunk {
    type RMessage<'b> = RClunk;
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

impl<'a> Message<'a> for TStat {
    const TYPE: MessageType = MessageType::TStat;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RStat<'a> {
    pub stat: &'a [u8],
}

impl<'a> Message<'a> for RStat<'a> {
    const TYPE: MessageType = MessageType::RStat;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        if body.len() < 2 {
            return Err(Error::MessageLength);
        }
        let len = u16::from_le_bytes([body[0], body[1]]) as usize;
        if body.len() < 2 + len {
            return Err(Error::MessageLength);
        }
        Ok(RStat { stat: &body[2..] })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
}

impl<'a> TMessage<'a> for TStat {
    type RMessage<'b> = RStat<'b>;
}

#[derive(Clone, Debug, Default)]
pub struct TWStat<'a> {
    pub fid: Fid,
    pub stat: &'a [u8],
}

impl<'a> Message<'a> for TWStat<'a> {
    const TYPE: MessageType = MessageType::TWStat;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 2 + self.stat.len()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        writer.write_all(&(self.stat.len() as u16).to_le_bytes())?;
        writer.write_all(&self.stat)?;
        Ok(())
    }
}

impl<'a> TMessage<'a> for TWStat<'a> {
    type RMessage<'b> = RWStat;
}

#[derive(Clone, Debug, Default)]
pub struct RWStat;

impl_empty_message!(RWStat, MessageType::RWStat);
