// http://man.cat-v.org/plan_9/5/intro
// http://ericvh.github.io/9p-rfc/rfc9p2000.u.html

// TODO: no-copy parsing from virtio ring buffer?
// - even if we copy soon after, want to avoid copying twice. at least.
// TODO: avoid multiple small writes? buffer without copy?
// - maybe the trick is to buffer everything other than large payloads. or use readv/writev.
// Many messages are fixed size. Things like Walk should be relatively small. Read/Write may be
// huge.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::str;

mod error;
pub use error::Error;
mod header;
pub use header::Header;
#[cfg(feature = "std")]
mod sync_client;
#[cfg(feature = "std")]
pub use sync_client::SyncClient;
#[cfg(feature = "tokio")]
mod tokio_server;

// XXX
pub fn parse_dir(mut bytes: &[u8]) -> Result<Vec<Stat<'_>>, Error> {
    let mut entries = Vec::new();
    while !bytes.is_empty() {
        let entry;
        (bytes, entry) = Stat::parse(bytes)?;
        entries.push(entry);
    }
    Ok(entries)
}

/// Equivalent of `io::Write`, but only with `write_all` behavior, and with
/// custom error type. Usable without std.
pub trait Writer {
    type Err;
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Err>;
}

#[cfg(feature = "std")]
impl<T: std::io::Write> Writer for T {
    type Err = std::io::Error;
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Err> {
        self.write_all(bytes)
    }
}

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
        let (bytes, len) = u32::parse(bytes)?;
        let len = len as usize;
        if bytes.len() < len {
            return Err(Error::MessageLength);
        }
        Ok((&bytes[len..], &bytes[..len]))
    }
}

impl<'a> Field<'a> for &'a str {
    fn parse(bytes: &'a [u8]) -> Result<(&'a [u8], Self), Error> {
        let (bytes, len) = u16::parse(bytes)?;
        let len = len as usize;
        if bytes.len() < len {
            return Err(Error::MessageLength);
        }
        Ok((&bytes[len..], str::from_utf8(&bytes[..len])?))
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
pub struct Qid {
    pub type_: u8,
    pub vers: u32,
    pub path: u64,
}

impl Qid {
    pub fn is_dir(&self) -> bool {
        self.type_ & 0x80 != 0
    }
}

impl<'a> Field<'a> for Qid {
    fn parse(bytes: &[u8]) -> Result<(&[u8], Self), Error> {
        let (bytes, type_) = u8::parse(bytes)?;
        let (bytes, vers) = u32::parse(bytes)?;
        let (bytes, path) = u64::parse(bytes)?;
        Ok((bytes, Qid { type_, vers, path }))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stat<'a> {
    pub type_: u16,
    pub dev: u32,
    pub qid: Qid,
    pub mode: u32,
    pub atime: u32,
    pub mtime: u32,
    pub length: u64,
    pub name: &'a str,
    pub uid: &'a str,
    pub gid: &'a str,
    pub muid: &'a str,
    /* 9p2000.u
    extension: &'a str,
    n_uid: u32,
    n_gid: u32,
    n_muid: u32,
    */
}

impl<'a> Field<'a> for Stat<'a> {
    fn parse(bytes: &'a [u8]) -> Result<(&[u8], Self), Error> {
        let (bytes, size) = u16::parse(bytes)?; // TODO
        let rest = &bytes[size as usize..];
        let (bytes, type_) = u16::parse(bytes)?;
        let (bytes, dev) = u32::parse(bytes)?;
        let (bytes, qid) = Qid::parse(bytes)?;
        let (bytes, mode) = u32::parse(bytes)?;
        let (bytes, atime) = u32::parse(bytes)?;
        let (bytes, mtime) = u32::parse(bytes)?;
        let (bytes, length) = u64::parse(bytes)?;
        let (bytes, name) = <&str>::parse(bytes)?;
        let (bytes, uid) = <&str>::parse(bytes)?;
        let (bytes, gid) = <&str>::parse(bytes)?;
        let (bytes, muid) = <&str>::parse(bytes)?;
        Ok((
            rest,
            Stat {
                type_,
                dev,
                qid,
                mode,
                atime,
                mtime,
                length,
                name,
                uid,
                gid,
                muid,
            },
        ))
    }
}

#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
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
    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err>;
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

            fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.msize.to_le_bytes())?;
        writer.write(&(self.version.len() as u16).to_le_bytes())?;
        writer.write(self.version.as_bytes())?;
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
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
    /* TODO 9p2000.u
    u_uname: u32,
    */
}

impl<'a> Message<'a> for TAuth<'a> {
    const TYPE: MessageType = MessageType::TAuth;

    fn parse(_body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 2 + self.uname.len() + 2 + self.aname.len()
    }

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.afid.0.to_le_bytes())?;
        writer.write(&(self.uname.len() as u16).to_le_bytes())?;
        writer.write(self.uname.as_bytes())?;
        writer.write(&(self.aname.len() as u16).to_le_bytes())?;
        writer.write(self.aname.as_bytes())?;
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

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
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
        let (mut body, ename) = <&str>::parse(body)?;
        // XXX 9p2000.u
        if body.len() == 4 {
            let _errno;
            (body, _errno) = u32::parse(body)?;
        }
        end_of_message(body, RError { ename })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
        todo!()
    }
}

#[derive(Clone, Debug, Default)]
pub struct TAttach<'a> {
    pub fid: Fid,
    pub afid: Fid,
    pub uname: &'a str,
    pub aname: &'a str,
    /* TODO 9p2000.u
    u_uname: u32,
    */
}

impl<'a> Message<'a> for TAttach<'a> {
    const TYPE: MessageType = MessageType::TAttach;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 4 + 2 + self.uname.len() + 2 + self.aname.len()
    }

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        writer.write(&self.afid.0.to_le_bytes())?;
        writer.write(&(self.uname.len() as u16).to_le_bytes())?;
        writer.write(self.uname.as_bytes())?;
        writer.write(&(self.aname.len() as u16).to_le_bytes())?;
        writer.write(self.aname.as_bytes())?;
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

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        writer.write(&self.newfid.0.to_le_bytes())?;
        writer.write(&(self.wnames.len() as u16).to_le_bytes())?;
        for wname in &self.wnames {
            writer.write(&(wname.len() as u16).to_le_bytes())?;
            writer.write(wname.as_bytes())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RWalk {
    pub qids: Vec<Qid>,
}

impl<'a> Message<'a> for RWalk {
    const TYPE: MessageType = MessageType::RWalk;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        let (mut body, len) = u16::parse(body)?;
        if body.len() != 13 * len as usize {
            return Err(Error::MessageLength);
        }
        let mut qids = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let qid;
            (body, qid) = Qid::parse(body)?;
            qids.push(qid);
        }
        Ok(RWalk { qids })
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        writer.write(&[self.mode])?;
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

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
        todo!()
    }
}

#[derive(Clone, Debug, Default)]
pub struct TCreate<'a> {
    pub fid: Fid,
    pub name: &'a str,
    pub perm: u32,
    pub mode: u8,
    /* TODO 9p2000.u
    extension: &'a str,
    */
}

impl<'a> Message<'a> for TCreate<'a> {
    const TYPE: MessageType = MessageType::TCreate;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        todo!()
    }

    fn size(&self) -> usize {
        4 + 2 + self.name.len() + 4 + 1
    }

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        writer.write(&(self.name.len() as u16).to_le_bytes())?;
        writer.write(self.name.as_bytes())?;
        writer.write(&self.perm.to_le_bytes())?;
        writer.write(&[self.mode])?;
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

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        writer.write(&self.offset.to_le_bytes())?;
        writer.write(&self.count.to_le_bytes())?;
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

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        writer.write(&self.offset.to_le_bytes())?;
        writer.write(&self.data)?;
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.count.to_le_bytes())?;
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct RStat<'a> {
    pub stat: Stat<'a>,
}

impl<'a> Message<'a> for RStat<'a> {
    const TYPE: MessageType = MessageType::RStat;

    fn parse(body: &'a [u8]) -> Result<Self, Error> {
        let (body, _len) = u16::parse(body)?;
        let (body, stat) = Stat::parse(body)?;
        //end_of_message(body, RStat { stat })
        Ok(RStat { stat }) // XXX?
    }

    fn size(&self) -> usize {
        todo!()
    }

    fn write<T: Writer>(&self, _writer: &mut T) -> Result<(), T::Err> {
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

    fn write<T: Writer>(&self, writer: &mut T) -> Result<(), T::Err> {
        writer.write(&self.fid.0.to_le_bytes())?;
        writer.write(&(self.stat.len() as u16).to_le_bytes())?;
        writer.write(&self.stat)?;
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
impl_tmessage_rmessage!(TWalk<'a>, RWalk);
impl_tmessage_rmessage!(TOpen, ROpen);
impl_tmessage_rmessage!(TCreate<'a>, RCreate);
impl_tmessage_rmessage!(TRead, RRead<'b>);
impl_tmessage_rmessage!(TWrite<'a>, RWrite);
impl_tmessage_rmessage!(TClunk, RClunk);
impl_tmessage_rmessage!(TRemove, RRemove);
impl_tmessage_rmessage!(TStat, RStat<'b>);
impl_tmessage_rmessage!(TWStat<'a>, RWStat);
