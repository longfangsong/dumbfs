use crate::disk::dump::DumpToFixedLocation;
use bincode::{deserialize_from, serialize_into, Error};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;

pub mod dump;

#[derive(Clone)]
pub struct Disk(Rc<RefCell<File>>);

impl Disk {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Disk(Rc::new(RefCell::new(
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(cfg!(test))
                .open(path)
                .unwrap(),
        )))
    }
    pub fn dump_at<D: Serialize + DeserializeOwned>(&self, location: u64, value: &D) {
        self.0.borrow_mut().seek(SeekFrom::Start(location)).unwrap();
        serialize_into(self.0.deref().borrow().deref(), value).unwrap();
    }
    pub fn load_at<D: Serialize + DeserializeOwned>(&self, location: u64) -> Result<D, Error> {
        self.0.borrow_mut().seek(SeekFrom::Start(location)).unwrap();
        deserialize_from(self.0.deref().borrow().deref())
    }
    pub fn dump_fixed_location<D: Serialize + DeserializeOwned, T: DumpToFixedLocation<D>>(
        &self,
        object: &T,
    ) {
        let location = object.location();
        self.dump_at(location, &object.dump_part());
    }
}

impl Seek for Disk {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.0.borrow_mut().seek(pos)
    }
}

impl Read for Disk {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.borrow_mut().read(buf)
    }
}

impl Write for Disk {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

#[test]
fn test_disk() -> io::Result<()> {
    use tempfile::tempdir;
    let tempdir = tempdir()?;
    let file_path = tempdir.path().join("temp.img");
    let mut disk = Disk::new(&file_path);
    disk.write_all(b"hello world").unwrap();
    disk.seek(SeekFrom::Start(6)).unwrap();
    let mut result = [0u8; 5];
    disk.read_exact(&mut result).unwrap();
    assert_eq!(&result, b"world");
    Ok(())
}
