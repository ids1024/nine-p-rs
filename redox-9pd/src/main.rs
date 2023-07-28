use nine_p::{Fid, Header, Message, RError};
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    sync::Arc,
};
use syscall::{
    error::{Error, ENOTDIR},
    flag::{O_DIRECTORY, O_WRONLY},
    SchemeMut,
};
use virtio_core::spec::{Buffer, ChainBuilder, DescriptorFlags};

struct Scheme<'a> {
    queue: Arc<virtio_core::transport::Queue<'a>>,
    dma: common::dma::Dma<[u8; 4096]>,
    reply_dma: common::dma::Dma<[u8; 4096]>,
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

impl<'a> Scheme<'a> {
    fn new(queue: Arc<virtio_core::transport::Queue<'a>>) -> Self {
        let mut scheme = Self {
            queue,
            dma: common::dma::Dma::new([0; 4096]).unwrap(),
            reply_dma: common::dma::Dma::new([0; 4096]).unwrap(),
        };
        // TODO does msize include header? consider reply.
        scheme
            .send(
                65535,
                nine_p::TVersion {
                    msize: 4096,
                    version: "9P2000",
                },
            )
            .unwrap();
        scheme
    }

    fn send<'b, T: nine_p::TMessage<'b>>(
        &mut self,
        tag: u16,
        msg: T,
    ) -> Result<T::RMessage<'_>, nine_p::Error> {
        let header = nine_p::Header::for_message(&msg, tag);
        header.write(&mut self.dma[..]).unwrap();
        msg.write(&mut self.dma[..]).unwrap();

        let command = ChainBuilder::new()
            .chain(Buffer::new(&self.dma))
            .chain(Buffer::new(&self.reply_dma).flags(DescriptorFlags::WRITE_ONLY))
            .build();
        futures_executor::block_on(self.queue.send(command));

        let header = Header::from_array(<[u8; 7]>::try_from(&self.reply_dma[..7]).unwrap());
        parse_reply(&header, &self.reply_dma[7..])
    }
}

impl<'a> syscall::scheme::SchemeMut for Scheme<'a> {
    fn open(&mut self, _path: &str, flags: usize, uid: u32, _gid: u32) -> syscall::Result<usize> {
        let res = self
            .send(
                0,
                nine_p::TOpen {
                    fid: Fid(1), // XXX lowest unused FD?
                    mode: 0,     // XXX
                },
            )
            .unwrap();
        let is_dir = res.qid.is_dir();
        if flags & O_DIRECTORY == O_DIRECTORY && !is_dir {
            return Err(Error::new(ENOTDIR));
        } // else if flags & O_WRONLY == O_WRONLY {
        todo!()
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> syscall::Result<usize> {
        todo!()
    }

    fn write(&mut self, id: usize, buffer: &[u8]) -> syscall::Result<usize> {
        todo!()
    }
}

fn daemon(daemon: redox_daemon::Daemon) -> ! {
    let mut pcid_handle = pcid_interface::PcidServerHandle::connect_default().unwrap();
    let pci_config = pcid_handle.fetch_config().unwrap();
    assert_eq!(pci_config.func.devid, 0x1009);

    let device = virtio_core::probe_device(&mut pcid_handle).unwrap();
    let device_space = device.device_space;

    // TODO?
    device.transport.finalize_features();

    let queue = device
        .transport
        .setup_queue(virtio_core::MSIX_PRIMARY_VECTOR, &device.irq_handle)
        .unwrap();

    let mut socket_file = fs::File::create(":9p").unwrap();

    let mut scheme: Scheme = Scheme::new(queue);

    daemon.ready().unwrap();

    let mut packet = syscall::Packet::default();
    loop {
        socket_file.read(&mut packet).unwrap();
        scheme.handle(&mut packet);
        socket_file.write(&mut packet).unwrap();
    }
}

fn main() {
    redox_daemon::Daemon::new(daemon).expect("9p: failed to daemonize");
}
