// Uses Qid path as ino

use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, Request,
};
use nine_p::{Fid, Qid};
use std::{
    collections::HashMap,
    ffi::OsStr,
    net::TcpStream,
    os::unix::ffi::OsStrExt,
    str,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const TTL: Duration = Duration::from_secs(1);

// An Inode is added by `lookup`, and at start for root node
struct Inode {
    // `path` of qid should match inode number
    qid: Qid,
    fid: Fid,
    lookups: u64,
}

fn file_type(type_: u8) -> FileType {
    if type_ & 0x80 != 0 {
        FileType::Directory
    } else {
        FileType::RegularFile
    }
}

fn attr_from_stat(stat: &nine_p::Stat) -> FileAttr {
    FileAttr {
        ino: stat.qid.path,
        size: stat.length,
        blocks: (stat.length + 4096 - 1) / 4096,
        atime: UNIX_EPOCH + Duration::from_secs(stat.atime as u64),
        mtime: UNIX_EPOCH + Duration::from_secs(stat.mtime as u64),
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: file_type(stat.qid.type_),
        perm: 0o755,
        // TODO use .u extension, if available
        nlink: 1,
        uid: 0,
        gid: 0,
        rdev: 0,
        flags: 0,
        blksize: 4096,
    }
}

struct DirEntry {
    ino: u64,
    kind: FileType,
    name: String,
}

struct OpenFile {
    dir_entries: Vec<DirEntry>,
    fid: Fid,
}

struct FS {
    client: nine_p::SyncClient<TcpStream>,
    next_id: Fid,
    next_fh: u64,
    inodes: HashMap<u64, Inode>,
    open_files: HashMap<u64, OpenFile>,
    root_ino: u64,
}

impl FS {
    fn inode(&self, ino: u64) -> Option<&Inode> {
        // XXX could 9p use 1 as a qid path?
        if ino == 1 {
            self.inodes.get(&self.root_ino)
        } else {
            self.inodes.get(&ino)
        }
    }

    fn inode_mut(&mut self, ino: u64) -> Option<&mut Inode> {
        if ino == 1 {
            self.inodes.get_mut(&self.root_ino)
        } else {
            self.inodes.get_mut(&ino)
        }
    }
}

impl fuser::Filesystem for FS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let Ok(name) = str::from_utf8(name.as_bytes()) else {
            reply.error(libc::ENOENT);
            return;
        };

        eprintln!("lookup: {name}");

        let parent_fid = if let Some(parent_inode) = self.inode_mut(parent) {
            parent_inode.fid
        } else {
            reply.error(libc::ENOENT);
            return;
        };

        let res = self
            .client
            .send(
                0,
                nine_p::TWalk {
                    fid: parent_fid,
                    newfid: self.next_id,
                    wnames: vec![name],
                },
            )
            .unwrap();
        let mut fid = self.next_id;
        self.next_id.0 += 1;

        // XXX panic?
        let qid = res.qids[0];
        let ino = qid.path;
        if let Some(inode) = self.inode_mut(ino) {
            inode.lookups += 1;

            // We already have an fid for this qid/ino, so we don't need another
            let existing_fid = inode.fid;
            let res = self.client.send(0, nine_p::TClunk { fid }).unwrap();
            fid = existing_fid;
        } else {
            self.inodes.insert(
                ino,
                Inode {
                    qid,
                    fid,
                    lookups: 1,
                },
            );
        }

        let stat = self.client.send(0, nine_p::TStat { fid }).unwrap().stat;
        let mut attr = attr_from_stat(&stat);

        reply.entry(&TTL, &attr, 0); // XXX generation?
    }

    fn open(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        // TODO: TOpen
        println!("open");
        reply.opened(0, 0);
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
        println!("opendir");
        if let Some(inode) = self.inode(ino) {
            let fid = inode.fid;

            let newfid = self.next_id;
            let res = self
                .client
                .send(
                    0,
                    nine_p::TWalk {
                        fid,
                        newfid,
                        wnames: vec![],
                    },
                )
                .unwrap();
            self.next_id.0 += 1;

            let res = self
                .client
                .send(
                    0,
                    nine_p::TOpen {
                        fid: newfid,
                        mode: 0, // XXX
                    },
                )
                .unwrap(); // XXX

            let mut dir_contents = Vec::new(); // XXX
            let mut offset = 0;
            loop {
                let res = self
                    .client
                    .send(
                        0,
                        nine_p::TRead {
                            fid: newfid,
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

            let stats = nine_p::parse_dir(&dir_contents).unwrap(); // XXX
            let dir_entries = stats
                .iter()
                .map(|stat| DirEntry {
                    ino: stat.qid.path,
                    kind: file_type(stat.qid.type_),
                    name: stat.name.to_string(),
                })
                .collect();

            let fh = self.next_fh;
            self.next_fh += 1;

            self.open_files.insert(
                fh,
                OpenFile {
                    dir_entries,
                    fid: newfid,
                },
            );

            reply.opened(fh, 0);
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        if let Some(open_file) = self.open_files.remove(&fh) {
            let res = self
                .client
                .send(0, nine_p::TClunk { fid: open_file.fid })
                .unwrap();
        }
        reply.ok();
    }

    fn forget(&mut self, _req: &Request<'_>, ino: u64, nlookup: u64) {
        if let Some(inode) = self.inode_mut(ino) {
            inode.lookups -= nlookup;
            if inode.lookups == 0 {
                let fid = inode.fid;
                let res = self
                    .client
                    .send(0, nine_p::TClunk { fid })
                    .unwrap();
                self.inodes.remove(&ino);
            }
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        eprintln!("getattr: {ino}");
        if let Some(inode) = self.inode(ino) {
            let stat = self
                .client
                .send(0, nine_p::TStat { fid: inode.fid })
                .unwrap()
                .stat;
            dbg!(stat);
            let mut attr = attr_from_stat(&stat);

            reply.attr(&TTL, &attr);
        } else {
            println!("FOO!");
            reply.error(libc::ENOENT);
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        _size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        eprintln!("read");
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        eprintln!("readdir");
        if let Some(open_file) = self.open_files.get(&fh) {
            let entries = &open_file.dir_entries[offset as usize..];
            for (i, entry) in entries.iter().enumerate() {
                reply.add(entry.ino, offset + i as i64 + 1, entry.kind, &entry.name);
            }
            reply.ok();
        } else {
            reply.error(libc::ENOENT);
        }
    }
}

fn main() {
    let stream = TcpStream::connect("localhost:564").unwrap();
    let mut client = nine_p::SyncClient::new(stream);

    let res = client
        .send(
            65535,
            nine_p::TVersion {
                msize: 8192,
                version: "9P2000.u",
            },
        )
        .unwrap();
    println!("{:?}", res);

    let res = client
        .send(
            0,
            nine_p::TAttach {
                fid: Fid(0),
                afid: Fid(u32::MAX),
                uname: "",
                aname: "",
            },
        )
        .unwrap();

    let root_inode = Inode {
        qid: res.qid,
        fid: Fid(0),
        lookups: 1,
    };
    let mut inodes = HashMap::new();
    inodes.insert(res.qid.path, root_inode);

    println!("{:?}", res);

    let fs = FS {
        client,
        next_id: Fid(1),
        next_fh: 0,
        inodes,
        open_files: HashMap::new(),
        root_ino: res.qid.path,
    };
    fuser::mount2(fs, "mnt", &[]).unwrap();
}
