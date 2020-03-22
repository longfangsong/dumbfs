use crate::disk::dump::DumpToFixedLocation;
use crate::disk::Disk;
use bincode::Error;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::io;

pub const MAGIC: u32 = 0xAA55_9669;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DumbFsMeta {
    pub magic: u32,
    next_ino: u64,
    pub next_free_address: u64,
}

impl Default for DumbFsMeta {
    fn default() -> Self {
        DumbFsMeta {
            magic: 0xAA55_9669,
            next_ino: 1,
            next_free_address: 512,
        }
    }
}

impl DumbFsMeta {
    pub fn acquire_next_ino(&mut self) -> u64 {
        let result = self.next_ino;
        self.next_ino += 1;
        result
    }
    pub fn valid(&self) -> bool {
        self.magic == MAGIC
    }
}

impl DumpToFixedLocation<DumbFsMeta> for DumbFsMeta {
    fn dump_part(&self) -> DumbFsMeta {
        self.clone()
    }

    fn location(&self) -> u64 {
        0
    }

    fn load(disk: &Disk, address: u64) -> Result<Self, Error> {
        assert_eq!(address, 0);
        disk.load_at(address)
    }
}

#[test]
fn test_meta() -> io::Result<()> {
    use tempfile::tempdir;
    let tempdir = tempdir()?;
    let file_path = tempdir.path().join("temp.img");
    let disk = Disk::new(file_path);
    let new_meta = DumbFsMeta::default();
    new_meta.sync(&disk);
    let mut meta = DumbFsMeta::load(&disk, 0).unwrap();
    assert_eq!(meta.next_free_address, 512);
    meta.next_free_address = 1024;
    assert_eq!(meta.acquire_next_ino(), 1);
    assert_eq!(meta.acquire_next_ino(), 2);
    meta.sync(&disk);
    let mut meta = DumbFsMeta::load(&disk, 0).unwrap();
    assert!(meta.valid());
    assert_eq!(meta.acquire_next_ino(), 3);
    assert_eq!(meta.next_free_address, 1024);
    Ok(())
}
