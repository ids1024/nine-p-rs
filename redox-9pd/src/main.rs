use nine_p::{Fid, Header, Message, RError};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::TcpStream,
    sync::Arc,
};
use syscall::{
    error::{Error, EBADFD, EISDIR, ENOTDIR},
    flag::{O_DIRECTORY, O_WRONLY},
    SchemeMut,
};
use virtio_core::spec::{Buffer, ChainBuilder, DescriptorFlags};

const VIRTIO_9P_MOUNT_TAG: u32 = 0;

// XXX attach seperately per user? What does linux driver do?
const ROOT: Fid = Fid(0);

struct File {
    qid: nine_p::Qid,
    dir_contents: Option<Vec<u8>>,
}

struct Transport<'a> {
    queue: Arc<virtio_core::transport::Queue<'a>>,
    dma: common::dma::Dma<[u8; 4096]>,
    reply_dma: common::dma::Dma<[u8; 4096]>,
}

impl<'a> Transport<'a> {
    fn send<'b, T: nine_p::TMessage<'b>>(
        &mut self,
        tag: u16,
        msg: T,
    ) -> Result<T::RMessage<'_>, nine_p::Error> {
        println!("A0"); // XXX load bearing?
        let header = nine_p::Header::for_message(&msg, tag);
        header.write(&mut self.dma[..]).unwrap();
        msg.write(&mut self.dma[7..]).unwrap();

        let command = ChainBuilder::new()
            .chain(Buffer::new(&self.dma))
            .chain(Buffer::new(&self.reply_dma).flags(DescriptorFlags::WRITE_ONLY))
            .build();
        futures_executor::block_on(self.queue.send(command));

        let reply_header = Header::from_array(<[u8; 7]>::try_from(&self.reply_dma[..7]).unwrap());
        parse_reply(
            &reply_header,
            &self.reply_dma[7..reply_header.size as usize],
        )
    }
}

struct Scheme<'a> {
    transport: Transport<'a>,
    next_id: Fid,
    files: HashMap<Fid, File>,
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
        let mut transport = Transport {
            queue,
            dma: common::dma::Dma::new([0; 4096]).unwrap(),
            reply_dma: common::dma::Dma::new([0; 4096]).unwrap(),
        };

        // TODO does msize include header? consider reply.
        transport
            .send(
                65535,
                nine_p::TVersion {
                    msize: 4096,
                    version: "9P2000",
                },
            )
            .unwrap();
        transport
            .send(
                0,
                nine_p::TAttach {
                    fid: ROOT,
                    afid: Fid(u32::MAX),
                    uname: "",
                    aname: "",
                },
            )
            .unwrap();

        Self {
            transport,
            next_id: Fid(1),
            files: HashMap::new(),
        }
    }
}

impl<'a> syscall::scheme::SchemeMut for Scheme<'a> {
    fn open(&mut self, path: &str, flags: usize, uid: u32, _gid: u32) -> syscall::Result<usize> {
        let mut wnames: Vec<_> = path.split('/').collect();
        if wnames.is_empty() {
            wnames.push(".");
        }
        let res = self
            .transport
            .send(
                0,
                nine_p::TWalk {
                    fid: ROOT,
                    newfid: self.next_id,
                    wnames,
                },
            )
            .unwrap();
        let res = self
            .transport
            .send(
                0,
                nine_p::TOpen {
                    fid: self.next_id,
                    mode: 0, // XXX
                },
            )
            .unwrap(); // XXX error?
        self.files.insert(
            self.next_id,
            File {
                qid: res.qid,
                dir_contents: None,
            },
        );
        let id = self.next_id.0 as usize;
        self.next_id.0 += 1;
        return Ok(id);
        let is_dir = res.qid.is_dir();
        if flags & O_DIRECTORY == O_DIRECTORY && !is_dir {
            // XXX close
            return Err(Error::new(ENOTDIR));
        } else if flags & O_DIRECTORY != O_DIRECTORY && is_dir {
            return Err(Error::new(EISDIR));
        } // else if flags & O_WRONLY == O_WRONLY {
        todo!()
    }

    fn close(&mut self, id: usize) -> syscall::Result<usize> {
        // XXX
        Ok(0)
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> syscall::Result<usize> {
        // If directory, convert format
        if let Some(file) = self.files.get_mut(&Fid(id as u32)) {
            if file.qid.is_dir() {
                if file.dir_contents.is_none() {
                    let mut dir_contents = Vec::new(); // XXX
                    let mut offset = 0;
                    loop {
                        let res = self
                            .transport
                            .send(
                                0,
                                nine_p::TRead {
                                    fid: Fid(1),
                                    offset,
                                    count: 4096,
                                },
                            )
                            .unwrap(); // XXX
                        if res.data.len() == 0 {
                            break;
                        }
                        dir_contents.extend_from_slice(res.data);
                        offset += res.data.len() as u64;
                    }

                    // Convert
                    let stats = nine_p::parse_dir(&dir_contents).unwrap(); // XXX
                    let mut dir_contents = Vec::new();
                    for i in stats {
                        dir_contents.extend_from_slice(i.name.as_bytes());
                        dir_contents.push(b'\n');
                    }
                    file.dir_contents = Some(dir_contents);
                }

                todo!()
            } else {
                todo!()
            }
        } else {
            Err(Error::new(EBADFD))
        }
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

    if device.transport.check_device_feature(VIRTIO_9P_MOUNT_TAG) {
        // XXX
        unsafe {
            let tag_len = u16::from_ne_bytes([
                device_space.read_volatile(),
                device_space.add(1).read_volatile(),
            ]);
            let mut tag = Vec::new();
            for i in 2..2 + tag_len as usize {
                tag.push(device_space.add(i).read_volatile());
            }
            let tag = String::from_utf8_lossy(&tag);
            eprintln!("9p tag: {}", tag);
        }

        device.transport.ack_driver_feature(VIRTIO_9P_MOUNT_TAG);
    }

    // TODO?
    device.transport.finalize_features();

    let queue = device
        .transport
        .setup_queue(virtio_core::MSIX_PRIMARY_VECTOR, &device.irq_handle)
        .unwrap();

    device.transport.run_device();

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
