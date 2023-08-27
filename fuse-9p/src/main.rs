use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use std::{
    ffi::OsStr,
    net::TcpStream,
    os::unix::ffi::OsStrExt,
    time::{Duration, UNIX_EPOCH},
};

const TTL: Duration = Duration::from_secs(1);
const ROOT_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH,
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o755,
    nlink: 2,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};
const FOO_ATTR: FileAttr = FileAttr {
    ino: 2,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH,
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o755,
    nlink: 2,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

struct FS {
    client: nine_p::SyncClient<TcpStream>,
}

impl fuser::Filesystem for FS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        eprintln!("lookup");
        if parent == 1 && name == OsStr::from_bytes(b"foo") {
            reply.entry(&TTL, &FOO_ATTR, 0);
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        eprintln!("getattr");
        if ino == 1 {
            reply.attr(&TTL, &ROOT_ATTR);
            return;
        }
        reply.error(libc::ENOENT)
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
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        eprintln!("readdir");
        if ino != 1 {
            reply.error(libc::ENOENT);
            return;
        }
        if offset == 0 {
            reply.add(
                2,
                offset + 0 + 1,
                FileType::Directory,
                OsStr::from_bytes(b"foo"),
            );
        }
        reply.ok();
    }
}

fn main() {
    let stream = TcpStream::connect("localhost:564").unwrap();
    let client = nine_p::SyncClient::new(stream);
    let fs = FS { client };
    fuser::mount2(fs, "mnt", &[]).unwrap();
}
