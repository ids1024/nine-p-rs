use nine_p::Fid;
use std::{collections::HashMap, sync::Arc};
use syscall::{
    error::{Error, EBADFD, EISDIR, ENOTDIR},
    flag::{O_DIRECTORY, O_RDWR, O_WRONLY},
    SchemeMut,
};

use crate::transport::{Transport, MSIZE};

// XXX attach seperately per user? What does linux driver do?
const ROOT: Fid = Fid(0);

struct File {
    qid: nine_p::Qid,
    dir_contents: Option<Vec<u8>>,
    offset: u64,
}

pub struct Scheme<'a> {
    transport: Transport<'a>,
    next_id: Fid,
    files: HashMap<Fid, File>,
}

impl<'a> Scheme<'a> {
    pub fn new(queue: Arc<virtio_core::transport::Queue<'a>>) -> Self {
        let mut transport = Transport::new(queue);

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

impl<'a> SchemeMut for Scheme<'a> {
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
