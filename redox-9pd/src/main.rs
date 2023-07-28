use nine_p::Fid;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
};
use syscall::{
    error::{Error, ENOTDIR},
    flag::{O_DIRECTORY, O_WRONLY},
    SchemeMut,
};

struct Scheme<'a> {
    client: nine_p::SyncClient<TcpStream>,
    queue: virtio_core::transport::Queue<'a>,
}

impl<'a> syscall::scheme::SchemeMut for Scheme<'a> {
    fn open(&mut self, _path: &str, flags: usize, uid: u32, _gid: u32) -> syscall::Result<usize> {
        virtio_core::spec::ChainBuilder::new();
        let dma = common::dma::Dma::new(0i32).unwrap();
        virtio_core::spec::Buffer::new(&dma);

        let res = self
            .client
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

    let socket_file = fs::File::create(":9p").unwrap();

    let scheme: Scheme = todo!(); // XXX

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
