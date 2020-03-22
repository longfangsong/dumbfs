use crate::disk::dump::DumpToFixedLocation;
use crate::disk::Disk;
use crate::file::dump_file_attr::FileAttrDump;
use crate::util::align;
use bincode::{serialized_size, Error};
use fuse::FileType;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::cmp::max;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::SystemTime;

pub mod dump_file_attr;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FileMeta {
    pub first_child: u64,
    pub next_sibling: u64,
    pub file_attr: FileAttrDump,
    pub filename: String,
}

pub struct File {
    address: u64,
    cursor: u64,
    pub meta: FileMeta,
    disk: Disk,
}

pub struct FileIterator {
    address: Option<u64>,
    disk: Disk,
}

impl DumpToFixedLocation<FileMeta> for File {
    fn dump_part(&self) -> FileMeta {
        self.meta.clone()
    }

    fn location(&self) -> u64 {
        self.address
    }

    fn load(disk: &Disk, address: u64) -> Result<Self, Error> {
        disk.load_at(address).map(|meta| File {
            meta,
            address,
            cursor: 0,
            disk: disk.clone(),
        })
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(address) => self.cursor = address,
            SeekFrom::Current(offset) => self.cursor = (self.address as i64 + offset) as _,
            SeekFrom::End(offset) => self.cursor = (self.meta.file_attr.size as i64 + offset) as _,
        };
        Ok(self.cursor)
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.disk
            .seek(SeekFrom::Start(
                self.address + serialized_size(&self.meta).unwrap() + self.cursor,
            ))
            .unwrap();
        self.disk.read(buf)
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.disk
            .seek(SeekFrom::Start(
                self.address + serialized_size(&self.meta).unwrap() + self.cursor,
            ))
            .unwrap();
        self.cursor += buf.len() as u64;
        self.meta.file_attr.size = max(self.cursor, self.meta.file_attr.size);
        self.sync(&self.disk);
        self.disk.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.disk.dump_fixed_location(self);
        self.disk.flush()
    }
}

impl<'a> Iterator for FileIterator {
    type Item = File;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(address) = self.address {
            let this_file = File::load(&self.disk, address).unwrap();
            self.address = if this_file.meta.next_sibling == 0 {
                None
            } else {
                Some(this_file.meta.next_sibling)
            };
            Some(this_file)
        } else {
            None
        }
    }
}

pub struct FileBuilder {
    address: u64,
    disk: Disk,
    pub meta: FileMeta,
}

impl FileBuilder {
    pub fn new(disk: &Disk, address: u64) -> Self {
        FileBuilder {
            disk: disk.clone(),
            address,
            meta: FileMeta::default(),
        }
    }
    pub fn filename(mut self, filename: &str) -> Self {
        self.meta.filename = filename.to_string();
        self
    }
    pub fn first_child(mut self, address: u64) -> Self {
        self.meta.first_child = address;
        self
    }
    pub fn next_sibling(mut self, address: u64) -> Self {
        self.meta.next_sibling = address;
        self
    }
    pub fn ino(mut self, ino: u64) -> Self {
        self.meta.file_attr.ino = ino;
        self
    }
    pub fn size(mut self, size: u64) -> Self {
        self.meta.file_attr.size = size;
        self
    }
    pub fn kind(mut self, kind: FileType) -> Self {
        self.meta.file_attr.kind = kind.into();
        self
    }
    pub fn build(&self) -> File {
        let mut file = File {
            address: self.borrow().address,
            cursor: 0,
            meta: self.meta.clone(),
            disk: self.disk.clone(),
        };
        let size = align(file.dump_size() + file.meta.file_attr.size, 512);
        file.meta.file_attr.blocks = size / 512;
        file.meta.file_attr.crtime = SystemTime::now();
        file.meta.file_attr.ctime = SystemTime::now();
        file.meta.file_attr.mtime = SystemTime::now();
        file.meta.file_attr.atime = SystemTime::now();
        file
    }
}

impl File {
    pub fn children(&self) -> FileIterator {
        FileIterator {
            address: if self.meta.first_child == 0 {
                None
            } else {
                Some(self.meta.first_child)
            },
            disk: self.disk.clone(),
        }
    }
    pub fn siblings(&self) -> FileIterator {
        FileIterator {
            address: if self.meta.next_sibling == 0 {
                None
            } else {
                Some(self.meta.first_child)
            },
            disk: self.disk.clone(),
        }
    }
}

#[cfg(test)]
fn prepare_test_data() -> io::Result<Disk> {
    use tempfile::tempdir;
    let tempdir = tempdir()?;
    let file_path = tempdir.path().join("temp.img");
    let disk = Disk::new(&file_path);

    let mut root = FileBuilder::new(&disk, 512)
        .ino(1)
        .first_child(1024)
        .build();
    let mut dir1 = FileBuilder::new(&disk, 1024)
        .ino(2)
        .filename("dir1")
        .first_child(2560)
        .next_sibling(1536)
        .build();
    let mut dir2 = FileBuilder::new(&disk, 1536)
        .ino(3)
        .filename("dir2")
        .next_sibling(2048)
        .build();
    let mut file1 = FileBuilder::new(&disk, 2048)
        .ino(4)
        .filename("file1.txt")
        .build();
    let mut file2 = FileBuilder::new(&disk, 2560)
        .ino(5)
        .filename("file2.txt")
        .build();
    root.flush().unwrap();
    dir1.flush().unwrap();
    dir2.flush().unwrap();
    file1.flush().unwrap();
    file2.flush().unwrap();
    Ok(disk)
}

#[test]
fn test_file() {
    let disk = prepare_test_data().unwrap();
    let root = File::load(&disk, 512).unwrap();
    let children: Vec<_> = root.children().collect();
    assert_eq!(children.len(), 3);
    assert_eq!(children[2].meta.filename, "file1.txt");
    let mut children: Vec<_> = children[0].children().collect();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].meta.filename, "file2.txt");
    children[0].write_all(b"hello world").unwrap();
    let mut buffer = [0u8; 5];
    children[0].seek(SeekFrom::Start(6)).unwrap();
    children[0].read_exact(&mut buffer).unwrap();
    assert_eq!(buffer[0], b'w');
}
