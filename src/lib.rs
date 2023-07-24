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

trait Field<'a>: Sized {
    fn parse(bytes: &'a [u8]) -> Result<(&'a [u8], Self), Error>;

    // fn len(&self) -> usize;

    // fn write<T: io::Write>(&self, iter: T) -> io::Result<()>;
}

macro_rules! impl_field_le_bytes {
    ($type:ty, $size:expr) => {
        impl<'a> Field<'a> for $type {
            fn parse(bytes: &[u8]) -> Result<(&[u8], Self), Error> {
                if let Some(value) = bytes.get(..$size) {
                    let value = <$type>::from_le_bytes(<[u8; $size]>::try_from(value).unwrap());
                    Ok((&bytes[$size..], value))
                } else {
                    Err(Error::MessageLength)
                }
            }
        }
    };
}
impl_field_le_bytes!(u8, 1);
impl_field_le_bytes!(u16, 2);
impl_field_le_bytes!(u32, 4);
impl_field_le_bytes!(u64, 8);

impl<'a> Field<'a> for &'a [u8] {
    fn parse(bytes: &'a [u8]) -> Result<(&'a [u8], Self), Error> {
        let (bytes, len) = u16::parse(bytes)?;
        let len = len as usize;
        if bytes.len() < len {
            return Err(Error::MessageLength);
        }
        Ok((&bytes[len..], &bytes[..len]))
    }
}

impl<'a> Field<'a> for &'a str {
    fn parse(bytes: &'a [u8]) -> Result<(&'a [u8], Self), Error> {
        let (bytes, value) = <&[u8]>::parse(bytes)?;
        Ok((bytes, str::from_utf8(value)?))
    }
}

fn end_of_message<T>(bytes: &[u8], value: T) -> Result<T, Error> {
    if bytes.is_empty() {
        Ok(value)
    } else {
        Err(Error::MessageLength)
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
    TWStat = 126,
    RWStat = 127,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Qid([u8; 13]);

unsafe impl bytemuck::Pod for Qid {}
unsafe impl bytemuck::Zeroable for Qid {}

impl<'a> Field<'a> for Qid {
    fn parse(bytes: &[u8]) -> Result<(&[u8], Self), Error> {
        if bytes.len() < 13 {
            return Err(Error::MessageLength);
        }
        Ok((
            &bytes[13..],
            Self([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12],
            ]),
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Fid(pub u32);

impl<'a> Field<'a> for Fid {
    fn parse(bytes: &'a [u8]) -> Result<(&'a [u8], Self), Error> {
        let (bytes, value) = u32::parse(bytes)?;
        Ok((bytes, Fid(value)))
    }
}

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
                end_of_message(body, $type)
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

#[derive(Clone, Debug, Default)]
pub struct RVersion<'a> {
    pub msize: u32,
    pub version: &'a str,
}

impl<'a> Message<'a> for RVersion<'a> {
    const TYPE: MessageType = MessageType::RVersion;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        let (body, msize) = u32::parse(body)?;
        let (body, version) = <&str>::parse(body)?;
        end_of_message(body, Self { msize, version })
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

#[derive(Clone, Debug, Default)]
pub struct RAuth {
    pub aqid: Qid,
}

impl<'a> Message<'a> for RAuth {
    const TYPE: MessageType = MessageType::RAuth;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        let (body, aqid) = Qid::parse(body)?;
        end_of_message(body, RAuth { aqid })
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
        let (body, ename) = <&str>::parse(body)?;
        end_of_message(body, RError { ename })
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
        let (body, qid) = Qid::parse(body)?;
        end_of_message(body, RAttach { qid })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
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
        let (body, len) = u16::parse(body)?;
        if body.len() != 13 * len as usize {
            return Err(Error::MessageLength);
        }
        Ok(RWalk {
            qids: bytemuck::cast_slice(body),
        })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
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
        let (body, qid) = Qid::parse(body)?;
        let (body, iounit) = u32::parse(body)?;
        end_of_message(body, ROpen { qid, iounit })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
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
        let (body, qid) = Qid::parse(body)?;
        let (body, iounit) = u32::parse(body)?;
        end_of_message(body, RCreate { qid, iounit })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug, Default)]
pub struct TRead {
    pub fid: Fid,
    pub offset: u64,
    pub count: u32,
}

impl<'a> Message<'a> for TRead {
    const TYPE: MessageType = MessageType::TRead;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        let (body, fid) = Fid::parse(body)?;
        let (body, offset) = u64::parse(body)?;
        let (body, count) = u32::parse(body)?;
        end_of_message(body, TRead { fid, offset, count })
    }

    fn size(&self) -> usize {
        4 + 8 + 4
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&self.count.to_le_bytes())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RRead<'a> {
    pub data: &'a [u8],
}

impl<'a> Message<'a> for RRead<'a> {
    const TYPE: MessageType = MessageType::RRead;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        let (body, data) = <&[u8]>::parse(body)?;
        end_of_message(body, RRead { data })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug, Default)]
pub struct TWrite<'a> {
    pub fid: Fid,
    pub offset: u64,
    pub data: &'a [u8],
}

impl<'a> Message<'a> for TWrite<'a> {
    const TYPE: MessageType = MessageType::TWrite;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 8 + 2 + self.data.len()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.fid.0.to_le_bytes())?;
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RWrite {
    pub count: u32,
}

impl<'a> Message<'a> for RWrite {
    const TYPE: MessageType = MessageType::RWrite;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        let (body, count) = u32::parse(body)?;
        end_of_message(body, RWrite { count })
    }

    fn size(&self) -> usize {
        4
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.count.to_le_bytes())?;
        Ok(())
    }
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

#[derive(Clone, Debug, Default)]
pub struct RClunk;

impl_empty_message!(RClunk, MessageType::RClunk);

#[derive(Clone, Debug, Default)]
pub struct TRemove {
    pub fid: Fid,
}

impl<'a> Message<'a> for TRemove {
    const TYPE: MessageType = MessageType::TRemove;

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
        let (body, stat) = <&[u8]>::parse(body)?;
        end_of_message(body, RStat { stat })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        todo!()
    }
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

#[derive(Clone, Debug, Default)]
pub struct RWStat;

impl_empty_message!(RWStat, MessageType::RWStat);

pub trait TMessage<'a>: Message<'a> {
    type RMessage<'b>: Message<'b>;
}

macro_rules! impl_tmessage_rmessage {
    ($tmsg:ty, $rmsg:ty) => {
        impl<'a> TMessage<'a> for $tmsg {
            type RMessage<'b> = $rmsg;
        }
    };
}

impl_tmessage_rmessage!(TVersion<'a>, RVersion<'b>);
impl_tmessage_rmessage!(TAuth<'a>, RAuth);
impl_tmessage_rmessage!(TAttach<'a>, RAttach);
impl_tmessage_rmessage!(TWalk<'a>, RWalk<'b>);
impl_tmessage_rmessage!(TOpen, ROpen);
impl_tmessage_rmessage!(TCreate<'a>, RCreate);
impl_tmessage_rmessage!(TRead, RRead<'b>);
impl_tmessage_rmessage!(TWrite<'a>, RWrite);
impl_tmessage_rmessage!(TClunk, RClunk);
impl_tmessage_rmessage!(TRemove, RRemove);
impl_tmessage_rmessage!(TStat, RStat<'b>);
impl_tmessage_rmessage!(TWStat<'a>, RWStat);
