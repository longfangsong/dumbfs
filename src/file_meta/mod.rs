use std::cell::RefCell;
use std::cmp::max;
use std::fs::File as Disk;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::DerefMut;
use std::rc::Rc;
use std::time::UNIX_EPOCH;

use bincode::serialized_size;
use fuse::FileType;
use serde::{Deserialize, Serialize};

use crate::file_meta::dump_file_attr::{FileAttrDump, FileTypeDump};
use crate::file_meta::dump_file_attr::FileTypeDump::{Directory, RegularFile};
use crate::fs::meta::DumbFsMeta;
use crate::util::align;

pub(crate) mod dump_file_attr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetaFixedSizedPart {
    pub first_child: u64,
    pub next_sibling: u64,
    pub file_attr: FileAttrDump,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileHead {
    pub fixed_sized_part: FileMetaFixedSizedPart,
    pub filename: String,
}

pub struct File {
    address: u64,
    disk: Rc<RefCell<Disk>>,
}

pub struct FileIterator {
    address: Option<u64>,
    disk: Rc<RefCell<Disk>>,
}

impl FileHead {
    pub fn new() -> Self {
        FileHead {
            fixed_sized_part: FileMetaFixedSizedPart {
                first_child: 0,
                next_sibling: 0,
                file_attr: FileAttrDump {
                    ino: 0,
                    size: 0,
                    blocks: 0,
                    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
                    mtime: UNIX_EPOCH,
                    ctime: UNIX_EPOCH,
                    crtime: UNIX_EPOCH,
                    kind: FileTypeDump::Directory,
                    perm: 0o777,
                    nlink: 0,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    flags: 0,
                },
            },
            filename: String::new(),
        }
    }
    pub fn deserialize_from<R>(reader: R) -> bincode::Result<Self>
        where
            R: std::io::Read,
    {
        bincode::deserialize_from(reader)
    }
    pub fn serialize_into<W>(&self, writer: W) -> bincode::Result<()>
        where
            W: std::io::Write,
    {
        bincode::serialize_into(writer, self)
    }
    pub fn serialize_size(&self) -> u64 {
        bincode::serialized_size(self).unwrap()
    }
}

impl File {
    // todo: try builder pattern!
    pub fn new(address: u64, disk: Rc<RefCell<Disk>>) -> Self {
        disk.borrow_mut().seek(SeekFrom::Start(address)).unwrap();
        Self {
            address,
            disk,
        }
    }
    pub fn address(&self) -> u64 { self.address }
    pub fn content_address(&self) -> u64 {
        let header = self.header();
        self.address + serialized_size(&header).unwrap()
    }
    pub fn next_chunk_start(&self) -> u64 {
        let header = self.header();
        align(self.address
            + serialized_size(&header).unwrap()
            + header.fixed_sized_part.file_attr.size)
    }
    pub fn header(&self) -> FileHead {
        self.disk.borrow_mut().seek(SeekFrom::Start(self.address)).unwrap();
        FileHead::deserialize_from(self.disk.borrow_mut().deref_mut()).unwrap_or_else(|_| FileHead::new())
    }
    pub fn set_header(&mut self, head: FileHead) {
        self.disk.borrow_mut().seek(SeekFrom::Start(self.address)).unwrap();
        head.serialize_into(self.disk.borrow_mut().deref_mut()).unwrap();
    }
    pub fn children(&self) -> FileIterator {
        let first_child_address = self.header()
            .fixed_sized_part.first_child;
        FileIterator {
            address: if first_child_address == 0 {
                None
            } else {
                Some(first_child_address)
            },
            disk: self.disk.clone(),
        }
    }
    pub fn set_ino(&mut self, ino: u64) {
        let mut header = self.header();
        header.fixed_sized_part.file_attr.ino = ino;
        self.set_header(header);
    }
    // todo: check over-lapping and move the file away
    pub fn set_content(&mut self, content: &[u8]) {
        let mut header = self.header();
        header.fixed_sized_part.file_attr.size = content.len() as _;
        header.fixed_sized_part.file_attr.blocks = align(header.serialize_size() + content.len() as u64) / 512;
        self.set_header(header);
        self.disk.borrow_mut().write_all(content).unwrap();
    }
    pub fn set_name(&mut self, name: &str) {
        let mut header = self.header();
        header.filename = name.to_string();
        self.set_header(header);
    }
    pub fn set_next_sibling(&mut self, address: u64) {
        let mut header = self.header();
        header.fixed_sized_part.next_sibling = address;
        self.set_header(header);
    }
    pub fn set_first_child(&mut self, address: u64) {
        let mut header = self.header();
        header.fixed_sized_part.first_child = address;
        self.set_header(header);
    }
    pub fn set_file_type(&mut self, file_type: FileType) {
        let mut header = self.header();
        header.fixed_sized_part.file_attr.kind = file_type.into();
        self.set_header(header);
    }
    pub fn read_at(&self, offset: usize, result: &mut [u8]) {
        let address = self.content_address();
        self.disk.borrow_mut().seek(SeekFrom::Start(address + offset as u64)).unwrap();
        self.disk.borrow_mut().read_exact(result).unwrap();
    }
    pub fn write(&mut self, offset: usize, result: &[u8]) {
        let origin_address = self.content_address();
        self.disk.borrow_mut().seek(SeekFrom::Start(origin_address + offset as u64)).unwrap();
        self.disk.borrow_mut().write_all(result).unwrap();
        let new_address = self.disk.borrow_mut().seek(SeekFrom::Current(0)).unwrap();
        let mut header = self.header();
        let old_size = header.fixed_sized_part.file_attr.size;
        header.fixed_sized_part.file_attr.size =
            max(old_size, new_address - origin_address);
        self.set_header(header);
    }
}

impl<'a> Iterator for FileIterator {
    type Item = File;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(address) = self.address {
            self.disk
                .borrow_mut()
                .seek(SeekFrom::Start(address))
                .unwrap();
            let current: Option<FileHead> =
                bincode::deserialize_from(self.disk.borrow_mut().deref_mut()).ok();
            self.address = current
                .as_ref()
                .map(|it| it.fixed_sized_part.next_sibling)
                .and_then(|it| if it == 0 { None } else { Some(it) });
            current.map(|_| File {
                address,
                disk: self.disk.clone(),
            })
        } else {
            None
        }
    }
}

#[test]
fn test_iterate() {
    use crate::test::create_test_img;
    create_test_img();
    let f = Rc::new(RefCell::new(Disk::open("/tmp/test.img").unwrap()));
    DumbFsMeta::deserialize_from((*f).borrow_mut().deref_mut()).unwrap();
    let next_address = align((*f).borrow_mut().deref_mut().seek(SeekFrom::Current(0)).unwrap());
    let root = File::new(next_address, f.clone());
    let children: Vec<_> = root.children().collect();
    assert_eq!(children.len(), 4);
    assert_eq!(children[0].header().filename, "dir1");
    assert_eq!(children[3].header().filename, "bye.txt");
    assert_eq!(children[1].header().fixed_sized_part.file_attr.kind, Directory);
    assert_eq!(children[2].header().fixed_sized_part.file_attr.kind, RegularFile);
}
