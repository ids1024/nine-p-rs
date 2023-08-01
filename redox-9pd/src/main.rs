use std::{
    fs,
    io::{Read, Write},
};
use syscall::SchemeMut;

mod scheme;
use scheme::Scheme;
mod transport;

const VIRTIO_9P_MOUNT_TAG: u32 = 0;

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
