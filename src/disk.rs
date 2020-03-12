use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

const BLKGETSIZE64_CODE: u8 = 0x12;
const BLKGETSIZE64_SEQ: u8 = 114;
ioctl_read!(ioctl_blkgetsize64, BLKGETSIZE64_CODE, BLKGETSIZE64_SEQ, u64);

pub struct Disk {
    pub disk: File,
}

impl Disk {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Disk {
            disk: OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .unwrap(),
        }
    }
    pub fn size(&self) -> usize {
        let fd = self.disk.as_raw_fd();
        let mut cap = 0u64;
        unsafe {
            ioctl_blkgetsize64(fd, &mut cap).unwrap();
        }
        return cap as _;
    }
    pub(crate) fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        self.disk.read_exact_at(buf, offset)
    }
    pub(crate) fn write_all_at(&self, buf: &[u8], offset: u64) -> std::io::Result<()> {
        self.disk.write_all_at(buf, offset)
    }
}

#[test]
fn test_disk_size() {
    let disk = Disk::new("/dev/sdb");
    assert_eq!(disk.size(), 4 * 1024 * 1024 * 1024);
}

#[test]
fn test_read_write() {
    // 0xAA559669
    let disk = Disk::new("/dev/sdb");
    disk.write_all_at(&[0x69, 0x96, 0x55, 0xAA], 0).unwrap();
    let mut result = [0u8; 4];
    disk.read_exact_at(&mut result, 0).unwrap();
    assert_eq!(result, [0x69, 0x96, 0x55, 0xAA]);
}
