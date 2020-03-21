use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs::{File as Disk, OpenOptions};
use std::io::SeekFrom;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use fuse::{Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry, Request};
use libc::ENOENT;

use crate::file_meta::dump_file_attr::FileTypeDump;
use crate::file_meta::File;
use crate::fs::meta::{DumbFsMeta, MAGIC};
use crate::util::align;

pub mod meta;

const TTL: Duration = Duration::from_secs(1);

pub struct DumbFS {
    disk: Rc<RefCell<Disk>>,
    meta: DumbFsMeta,
}

impl DumbFS {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        DumbFS {
            disk: Rc::new(RefCell::new(OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .unwrap())),
            meta: DumbFsMeta::new(),
        }
    }
}

impl Filesystem for DumbFS {
    fn init(&mut self, _req: &Request<'_>) -> Result<(), i32> {
        info!("init");
        let meta =
            DumbFsMeta::deserialize_from(self.disk.deref().borrow_mut().deref_mut())
                .ok()
                .and_then(|it| {
                    if it.magic != MAGIC {
                        None
                    } else {
                        Some(it)
                    }
                });
        match meta {
            Some(meta) => {
                info!("recover meta from disk");
                self.meta = meta
            }
            None => {
                info!("init filesystem");
                let mut file = File::new(self.meta.next_free_address(), self.disk.clone());
                let mut header = file.header();
                header.fixed_sized_part.file_attr.ino = self.meta.acquire_next_ino(self.disk.deref().borrow_mut().deref_mut());
                file.set_header(header);
                self.meta.sync(self.disk.deref().borrow_mut().deref_mut());
            }
        }
        Ok(())
    }
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        info!("lookup content in inode: {}", parent);
        if parent == 1 {
            let root = File::new(align(DumbFsMeta::serialize_size()), self.disk.clone());
            let result = root.children().find(|it|
                name.to_str().unwrap() == it.header().filename
            );
            if let Some(it) = result {
                reply.entry(&TTL, &it.header().fixed_sized_part.file_attr.into(), 0);
            } else {
                reply.error(ENOENT);
            }
        } else {
            reply.error(ENOENT);
        }
    }
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        info!("fetch attr for inode: {}", ino);
        let root = File::new(align(DumbFsMeta::serialize_size()), self.disk.clone());
        if root.header().fixed_sized_part.file_attr.ino == ino {
            reply.attr(&TTL, &root.header().fixed_sized_part.file_attr.into())
        } else if let Some(it) = root.children()
            .find(|it| it.header().fixed_sized_part.file_attr.ino == ino) {
            reply.attr(&TTL, &it.header().fixed_sized_part.file_attr.into())
        }
    }

    fn read(&mut self, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, size: u32, reply: ReplyData) {
        info!("read inode: {} [{}..{}]", ino, offset, offset + size as i64);
        let root = File::new(align(DumbFsMeta::serialize_size()), self.disk.clone());
        let file = root.children()
            .find(|it| it.header().fixed_sized_part.file_attr.ino == ino).unwrap();
        let mut buffer = vec![0; size as _];
        file.read_at(offset as _, &mut buffer[..]);
        reply.data(&buffer[..]);
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        info!("read dir: {}", ino);
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }
        let root = File::new(align(DumbFsMeta::serialize_size()), self.disk.clone());
        for (i, file) in root.children().enumerate().skip(offset as _) {
            let header = file.header();
            let attr = header.fixed_sized_part.file_attr;
            let filename = header.filename;
            reply.add(attr.ino, (i + 1) as i64, attr.kind.into(), filename);
        }
        reply.ok()
    }

    fn create(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, _mode: u32, _flags: u32, reply: ReplyCreate) {
        info!("create: {:?} in {}", name, parent);
        assert_eq!(parent, 1);
        let mut root = File::new(align(DumbFsMeta::serialize_size()), self.disk.clone());
        let last_file = root.children().last();
        let mut start_address: u64 = 0;
        match last_file {
            Some(mut last_file) => {
                start_address = last_file.next_chunk_start();
                last_file.set_next_sibling(start_address);
            }
            None => {
                start_address = root.next_chunk_start();
                root.set_first_child(start_address);
            }
        }
        let mut newfile = File::new(start_address, self.disk.clone());
        let mut header = newfile.header();
        let ino = self.meta.acquire_next_ino(self.disk.deref().borrow_mut().deref_mut());
        header.filename = name.to_str().unwrap().to_string();
        header.fixed_sized_part.file_attr.ino = ino;
        header.fixed_sized_part.file_attr.crtime = SystemTime::now();
        header.fixed_sized_part.file_attr.atime = SystemTime::now();
        header.fixed_sized_part.file_attr.ctime = SystemTime::now();
        header.fixed_sized_part.file_attr.mtime = SystemTime::now();
        header.fixed_sized_part.file_attr.perm = 0o777;
        header.fixed_sized_part.file_attr.kind = FileTypeDump::RegularFile;
        header.fixed_sized_part.file_attr.size = 0;
        header.fixed_sized_part.file_attr.blocks = 1;
        let attr = header.fixed_sized_part.file_attr.clone();
        newfile.set_header(header);
        reply.created(&TTL, &attr.into(), 1, ino, 0);
    }
}
