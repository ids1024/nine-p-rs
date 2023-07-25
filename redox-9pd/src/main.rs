use std::net::TcpStream;

struct Scheme {
    client: nine_p::SyncClient<TcpStream>,
}

impl syscall::scheme::SchemeMut for Scheme {
    fn open(&mut self, _path: &str, flags: usize, uid: u32, _gid: u32) -> syscall::Result<usize> {
        todo!()
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> syscall::Result<usize> {
        todo!()
    }

    fn write(&mut self, id: usize, buffer: &[u8]) -> syscall::Result<usize> {
        todo!()
    }
}

fn main() {}
