use std::ffi::OsStr;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bincode::{deserialize_from, serialize_into};
use fuse::{
    FileAttr, Filesystem, FileType, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::c_int;
use libc::ENOENT;

use crate::disk::Disk;
use crate::dump_file_attr::FileAttrDump;

const TTL: Duration = Duration::from_secs(1);
const MAGIC: u64 = 0xAA559669;
const NULL_ATTR: FileAttr = FileAttr {
    ino: 0,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o000,
    nlink: 0,
    uid: 0,
    gid: 0,
    rdev: 0,
    flags: 0,
};
const ROOT_DIR_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o755,
    nlink: 2,
    uid: 0,
    gid: 0,
    rdev: 0,
    flags: 0,
};

pub struct DumbFS(Disk);

impl DumbFS {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        DumbFS(Disk::new(path))
    }
}

impl Filesystem for DumbFS {
    fn init(&mut self, _req: &Request<'_>) -> Result<(), c_int> {
        let mut magic_bytes = [0u8; 8];
        self.0.read_exact_at(&mut magic_bytes, 0).unwrap();
        if u64::from_le_bytes(magic_bytes) != MAGIC {
            self.0.disk.seek(SeekFrom::Start(0)).unwrap();
            self.0.disk.write(&(MAGIC.to_le_bytes())).unwrap();
        }
        Ok(())
    }
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &ROOT_DIR_ATTR),
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
        if offset == 0 {
            reply.add(1, 1i64, FileType::Directory, ".");
        }
        if offset <= 1 {
            reply.add(1, 2i64, FileType::Directory, "..");
        }
        let mut current_offset = 3i64;
        let offset = if offset > 0 { offset } else { 1 };
        self
            .0
            .disk
            .seek(SeekFrom::Start(
                8u64
                    + (offset - 1) as u64
                    * bincode::serialized_size::<FileAttrDump>(&NULL_ATTR.into()).unwrap() as u64,
            ))
            .unwrap();
        loop {
            let next_attr: Result<FileAttrDump, _> = deserialize_from(&self.0.disk);
            match next_attr {
                Ok(dump_info) => {
                    if dump_info.ino != 0 {
                        reply.add(dump_info.ino, current_offset, dump_info.kind.into(), "file");
                        current_offset += 1;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        reply.ok();
    }
}
