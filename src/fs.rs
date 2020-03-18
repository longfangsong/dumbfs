use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use fuse::{Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, Request};
use libc::c_int;
use libc::ENOENT;

use crate::dump_file_attr::{FileAttrDump, FileTypeDump};
use crate::file_node::FileNode;

const TTL: Duration = Duration::from_secs(1);
const MAGIC: u32 = 0xAA559669;
const EMPTY_ROOT_FILE_NODE: FileNode = FileNode {
    first_child: 0,
    next_sibling: 0,
    file_attr: FileAttrDump {
        ino: 1,
        size: 0,
        blocks: 0,
        atime: UNIX_EPOCH, // 1970-01-01 00:00:00
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileTypeDump::Directory,
        perm: 0o777,
        nlink: 2,
        uid: 0,
        gid: 0,
        rdev: 0,
        flags: 0,
    },
};

pub struct DumbFS {
    disk: Arc<RefCell<File>>
}

pub struct DumbFSIterator {
    file: Arc<RefCell<File>>,
    pub current_address: u64,
}

impl Iterator for DumbFSIterator {
    type Item = (u64, FileNode);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_address == 0 {
            None
        } else {
            let mut file = &*self.file.clone();
            let address = self.current_address;
            file.borrow_mut().seek(SeekFrom::Start(self.current_address)).unwrap();
            let result: Option<FileNode> = bincode::deserialize_from(&*file.borrow_mut()).ok();
            self.current_address = result.clone().map(|it: FileNode| it.next_sibling).unwrap();
            result.map(|it| (address, it))
        }
    }
}

impl DumbFS {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        DumbFS {
            disk: Arc::new(RefCell::new(OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .unwrap())),
        }
    }

    fn children_of(&mut self, node_address: u64) -> DumbFSIterator {
        let disk = &*self.disk.clone();
        disk.borrow_mut().seek(SeekFrom::Start(node_address)).unwrap();
        let node: FileNode = bincode::deserialize_from(&*disk.borrow_mut()).unwrap();
        DumbFSIterator {
            file: self.disk.clone(),
            current_address: node.first_child,
        }
    }

    // todo: dfs
    fn find_node_address(&mut self, root_address: u64, condition: impl Fn(&FileNode) -> bool) -> Option<u64> {
        let disk = &*self.disk.clone();
        disk.borrow_mut().seek(SeekFrom::Start(root_address)).unwrap();
        let node: FileNode = bincode::deserialize_from(&*disk.borrow_mut()).unwrap();
        drop(disk);
        if condition(&node) {
            Some(root_address)
        } else {
            self.children_of(root_address)
                .find(|info| condition(&info.1))
                .map(|it| it.0)
        }
    }
}

#[test]
fn test_dumb_fs_iterator() {
    create_test_img();
    let mut fs = DumbFS::new("/tmp/test.img");
    let children = fs.children_of(4);
    let inos: Vec<u64> = children.map(|it| it.1.file_attr.ino).collect();
    assert_eq!(inos.len(), 2);
    assert!(inos.iter().any(|&it| it == 2));
    assert!(inos.iter().any(|&it| it == 3));
}

#[cfg(test)]
fn create_test_img() {
    let mut f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open("/tmp/test.img")
        .unwrap();
    f.write_all(&(MAGIC.to_le_bytes())).unwrap();
    let mut root = EMPTY_ROOT_FILE_NODE.clone();
    let next_start_position = 4 + bincode::serialized_size(&root).unwrap();
    root.first_child = next_start_position;
    bincode::serialize_into(&mut f, &root).unwrap();

    let mut next_node = EMPTY_ROOT_FILE_NODE.clone();
    next_node.file_attr.ino = 2;
    let next_node_filename = "dir1".to_string();
    let next_start_position = next_start_position
        + bincode::serialized_size(&next_node).unwrap()
        + bincode::serialized_size(&next_node_filename).unwrap();
    next_node.next_sibling = next_start_position;
    bincode::serialize_into(&mut f, &next_node).unwrap();
    bincode::serialize_into(&mut f, &next_node_filename).unwrap();

    let mut next_node = EMPTY_ROOT_FILE_NODE.clone();
    next_node.file_attr.ino = 3;
    let next_node_filename = "dir2".to_string();
    bincode::serialize_into(&mut f, &next_node).unwrap();
    bincode::serialize_into(&mut f, &next_node_filename).unwrap();
}

impl Filesystem for DumbFS {
    fn init(&mut self, _req: &Request<'_>) -> Result<(), c_int> {
        println!("init");
        let disk = &*self.disk.clone();
        let mut magic_bytes = [0u8; 4];
        disk.borrow_mut().read_exact(&mut magic_bytes).unwrap();
        if u32::from_le_bytes(magic_bytes) != MAGIC {
            disk.borrow_mut().seek(SeekFrom::Start(0)).unwrap();
            disk.borrow_mut().write_all(&(MAGIC.to_le_bytes())).unwrap();
            bincode::serialize_into(&mut *disk.borrow_mut(), &EMPTY_ROOT_FILE_NODE).unwrap();
        }
        Ok(())
    }
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == 1 {
            println!("lookup for: {:?}", name);
            let disk = &*self.disk.clone();
            let children = self.children_of(4);
            for (address, node) in children {
                let filename_address = address + bincode::serialized_size(&node).unwrap();
                disk.borrow_mut().seek(SeekFrom::Start(filename_address)).unwrap();
                let filename: String = bincode::deserialize_from(&*disk.borrow_mut()).unwrap();
                if name.to_str().unwrap() == filename {
                    reply.entry(&TTL, &node.file_attr.into(), 0);
                    break;
                }
            }
        } else {
            reply.error(ENOENT);
        }
    }
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr {}", ino);
        match ino {
            1 => reply.attr(&TTL, &EMPTY_ROOT_FILE_NODE.file_attr.into()),
            _ => reply.error(ENOENT),
        }
    }
    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }
        let disk = &*self.disk.clone();
        for (i, (address, node)) in self.children_of(4).enumerate().skip(offset as _) {
            disk.borrow_mut().seek(SeekFrom::Start(address + bincode::serialized_size(&node).unwrap())).unwrap();
            let filename: String = bincode::deserialize_from(&*disk.borrow_mut()).unwrap();
            reply.add(node.file_attr.ino, (i + 1) as i64, node.file_attr.kind.into(), filename);
        }
        reply.ok()
    }
}
