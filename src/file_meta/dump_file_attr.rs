use std::time::{SystemTime, UNIX_EPOCH};

use fuse::{FileAttr, FileType};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum FileTypeDump {
    Directory,
    RegularFile,
    Symlink,
}

impl From<FileType> for FileTypeDump {
    fn from(origin: FileType) -> Self {
        match origin {
            FileType::Directory => FileTypeDump::Directory,
            FileType::RegularFile => FileTypeDump::RegularFile,
            FileType::Symlink => FileTypeDump::Symlink,
            _ => unimplemented!("Not supported now"),
        }
    }
}

impl Into<FileType> for FileTypeDump {
    fn into(self) -> FileType {
        match self {
            FileTypeDump::Directory => FileType::Directory,
            FileTypeDump::RegularFile => FileType::RegularFile,
            FileTypeDump::Symlink => FileType::Symlink,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileAttrDump {
    pub ino: u64,
    pub size: u64,
    pub blocks: u64,
    pub atime: SystemTime,
    pub mtime: SystemTime,
    pub ctime: SystemTime,
    pub crtime: SystemTime,
    pub kind: FileTypeDump,
    pub perm: u16,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u32,
    pub flags: u32,
}

impl From<FileAttr> for FileAttrDump {
    fn from(origin: FileAttr) -> Self {
        FileAttrDump {
            ino: origin.ino,
            size: origin.size,
            blocks: origin.blocks,
            atime: origin.atime,
            mtime: origin.mtime,
            ctime: origin.ctime,
            crtime: origin.crtime,
            kind: origin.kind.into(),
            perm: origin.perm,
            nlink: origin.nlink,
            uid: origin.uid,
            gid: origin.gid,
            rdev: origin.rdev,
            flags: origin.flags,
        }
    }
}

impl Into<FileAttr> for FileAttrDump {
    fn into(self) -> FileAttr {
        FileAttr {
            ino: self.ino,
            size: self.size,
            blocks: self.blocks,
            atime: self.atime,
            mtime: self.mtime,
            ctime: self.ctime,
            crtime: self.crtime,
            kind: self.kind.into(),
            perm: self.perm,
            nlink: self.nlink,
            uid: self.uid,
            gid: self.gid,
            rdev: self.rdev,
            flags: self.flags,
        }
    }
}

#[test]
fn test_encode_decode() {
    let file_attr = FileAttr {
        ino: 1,
        size: 1024,
        blocks: 2,
        atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::RegularFile,
        perm: 0o777,
        nlink: 1,
        uid: 501,
        gid: 20,
        rdev: 0,
        flags: 0,
    };
    let encoded: Vec<u8> = bincode::serialize::<FileAttrDump>(&file_attr.into()).unwrap();
    assert_eq!(encoded.len(), 98);
    let decoded: FileAttr = bincode::deserialize::<FileAttrDump>(&encoded[..])
        .unwrap()
        .into();
    assert_eq!(decoded.size, 1024);
}
