// virtio transport for 9p

use nine_p::{Header, Message, RError};
use std::sync::Arc;
use virtio_core::spec::{Buffer, ChainBuilder, DescriptorFlags};

// XXX Configurable? Default?
// https://wiki.qemu.org/Documentation/9psetup#msize
pub const MSIZE: usize = 128 * 1024;

pub struct Transport<'a> {
    queue: Arc<virtio_core::transport::Queue<'a>>,
    dma: common::dma::Dma<[u8; MSIZE]>,
    reply_dma: common::dma::Dma<[u8; MSIZE]>,
}

impl<'a> Transport<'a> {
    pub fn new(queue: Arc<virtio_core::transport::Queue<'a>>) -> Self {
        Transport {
            queue,
            dma: common::dma::Dma::new([0; MSIZE]).unwrap(),
            reply_dma: common::dma::Dma::new([0; MSIZE]).unwrap(),
        }
    }

    pub fn send<'b, T: nine_p::TMessage<'b>>(
        &mut self,
        tag: u16,
        msg: T,
    ) -> Result<T::RMessage<'_>, nine_p::Error> {
        let header = nine_p::Header::for_message(&msg, tag);
        header.write(&mut self.dma[..]).unwrap();
        msg.write(&mut self.dma[7..]).unwrap();

        let command = ChainBuilder::new()
            .chain(Buffer::new(&self.dma))
            .chain(Buffer::new(&self.reply_dma).flags(DescriptorFlags::WRITE_ONLY))
            .build();
        // XXX return value?
        futures_executor::block_on(self.queue.send(command));

        let reply_header = Header::from_array(<[u8; 7]>::try_from(&self.reply_dma[..7]).unwrap());
        parse_reply(
            &reply_header,
            &self.reply_dma[7..reply_header.size as usize],
        )
    }
}

fn parse_reply<'a, Reply: Message<'a>>(
    header: &Header,
    body: &'a [u8],
) -> Result<Reply, nine_p::Error> {
    if header.type_ == Reply::TYPE as u8 {
        Reply::parse(body)
    } else if header.type_ == RError::TYPE as u8 {
        Err(nine_p::Error::Protocol(
            nine_p::RError::parse(body)?.ename.to_string(),
        ))
    } else {
        Err(nine_p::Error::UnexpectedType(header.type_))
    }
}
