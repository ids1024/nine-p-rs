use nine_p::{Fid, Header, Message, RError};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    sync::Arc,
};
use syscall::{
    error::{Error, EBADFD, EISDIR, ENOTDIR},
    flag::{O_DIRECTORY, O_RDWR, O_WRONLY},
    SchemeMut,
};
use virtio_core::spec::{Buffer, ChainBuilder, DescriptorFlags};

const VIRTIO_9P_MOUNT_TAG: u32 = 0;

// XXX attach seperately per user? What does linux driver do?
const ROOT: Fid = Fid(0);

// XXX Configurable? Default?
// https://wiki.qemu.org/Documentation/9psetup#msize
const MSIZE: usize = 128 * 1024;

struct File {
    qid: nine_p::Qid,
    dir_contents: Option<Vec<u8>>,
    offset: u64,
}

struct Transport<'a> {
    queue: Arc<virtio_core::transport::Queue<'a>>,
    dma: common::dma::Dma<[u8; MSIZE]>,
    reply_dma: common::dma::Dma<[u8; MSIZE]>,
}

impl<'a> Transport<'a> {
    fn send<'b, T: nine_p::TMessage<'b>>(
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
            dma: common::dma::Dma::new([0; MSIZE]).unwrap(),
            reply_dma: common::dma::Dma::new([0; MSIZE]).unwrap(),
        };

        std::thread::yield_now(); // Why is this needed XXX?

        // TODO does msize include header? consider reply.
        transport
            .send(
                65535,
                nine_p::TVersion {
                    msize: MSIZE as u32,
                    version: "9P2000.u",
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
        // XXX better path processing?
        let wnames = path.split('/').filter(|x| !x.is_empty()).collect();
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
                offset: 0,
            },
        );
        let id = self.next_id.0 as usize;
        self.next_id.0 += 1;

        let is_dir = res.qid.is_dir();
        if flags & O_DIRECTORY == O_DIRECTORY && !is_dir {
            // XXX close
            Err(Error::new(ENOTDIR))
        } else if flags & O_DIRECTORY != O_DIRECTORY
            && is_dir
            && (flags & O_WRONLY == O_WRONLY && flags & O_RDWR == O_RDWR)
        {
            Err(Error::new(EISDIR))
        } else {
            Ok(id)
        }
    }

    fn close(&mut self, id: usize) -> syscall::Result<usize> {
        let fid = Fid(id as u32);
        if let Some(file) = self.files.remove(&fid) {
            self.transport.send(0, nine_p::TClunk { fid }).unwrap();
            Ok(0)
        } else {
            Err(Error::new(EBADFD))
        }
    }

    fn fsync(&mut self, id: usize) -> syscall::Result<usize> {
        let fid = Fid(id as u32);
        if let Some(file) = self.files.get_mut(&fid) {
            // Nothing to do?
            Ok(0)
        } else {
            Err(Error::new(EBADFD))
        }
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> syscall::Result<usize> {
        // If directory, convert format
        let fid = Fid(id as u32);
        if let Some(file) = self.files.get_mut(&fid) {
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
                                    fid,
                                    offset,
                                    count: MSIZE as u32 - 7 - 4,
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

                let slice = &file.dir_contents.as_deref().unwrap()[file.offset as usize..];
                let len = slice.len().min(buf.len());
                buf[..len].copy_from_slice(&slice[..len]);
                file.offset += len as u64;
                Ok(len)
            } else {
                let res = self
                    .transport
                    .send(
                        0,
                        nine_p::TRead {
                            fid,
                            offset: file.offset,
                            count: (buf.len() as u32).min(MSIZE as u32 - 7 - 4),
                        },
                    )
                    .unwrap(); // XXX
                buf[..res.data.len()].copy_from_slice(res.data);
                file.offset += res.data.len() as u64;
                Ok(res.data.len())
            }
        } else {
            Err(Error::new(EBADFD))
        }
    }

    fn write(&mut self, id: usize, buffer: &[u8]) -> syscall::Result<usize> {
        todo!()
    }

    fn seek(&mut self, id: usize, pos: isize, whence: usize) -> syscall::Result<isize> {
        let fid = Fid(id as u32);
        if let Some(file) = self.files.get_mut(&fid) {
            Ok(0) // XXX
        } else {
            Err(Error::new(EBADFD))
        }
    }

    fn fstatvfs(&mut self, id: usize, stat: &mut syscall::StatVfs) -> syscall::Result<usize> {
        let fid = Fid(id as u32);
        if let Some(_file) = self.files.get_mut(&fid) {
            stat.f_bsize = 4096;
            stat.f_blocks = 0;
            stat.f_bfree = 0;
            stat.f_bavail = 0;

            Ok(0)
        } else {
            Err(Error::new(EBADFD))
        }
    }

    // TODO
    fn fstat(&mut self, id: usize, stat: &mut syscall::Stat) -> syscall::Result<usize> {
        let fid = Fid(id as u32);
        if let Some(file) = self.files.get_mut(&fid) {
            let type_ = if file.qid.is_dir() {
                syscall::flag::MODE_DIR
            } else {
                syscall::flag::MODE_FILE
            };
            let res = self.transport.send(0, nine_p::TStat { fid }).unwrap();
            *stat = syscall::data::Stat {
                st_mode: type_,
                st_size: res.stat.length,
                ..Default::default()
            };
            Ok(0)
        } else {
            Err(Error::new(EBADFD))
        }
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
