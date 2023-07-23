use std::io;

use crate::Message;

pub struct Header {
    pub size: u32,
    pub type_: u8,
    pub tag: u16,
}

impl Header {
    #[inline]
    pub fn for_message<'a, T: Message<'a>>(message: &T, tag: u16) -> Self {
        Self {
            size: 7 + message.size() as u32,
            type_: T::TYPE as u8,
            tag,
        }
    }

    #[inline]
    pub fn from_array(header: [u8; 7]) -> Self {
        Self {
            size: u32::from_le_bytes([header[0], header[1], header[2], header[3]]),
            type_: header[4],
            tag: u16::from_le_bytes([header[5], header[6]]),
        }
    }

    #[inline]
    pub fn write<T: io::Write>(&self, mut writer: T) -> io::Result<()> {
        writer.write_all(&self.size.to_le_bytes())?;
        writer.write_all(&[self.type_])?;
        writer.write_all(&self.tag.to_le_bytes())?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    pub async fn async_write<T: tokio::io::AsyncWrite + Unpin>(
        &self,
        mut writer: T,
    ) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;
        writer.write_all(&self.size.to_le_bytes()).await?;
        writer.write_all(&[self.type_]).await?;
        writer.write_all(&self.tag.to_le_bytes()).await?;
        Ok(())
    }
}
