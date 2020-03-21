use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs::{File as Disk, OpenOptions};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use fuse::{Filesystem, ReplyAttr, ReplyDirectory, ReplyEntry, Request};
use libc::ENOENT;

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
            result.map(|it| {
                reply.entry(&TTL, &it.header().fixed_sized_part.file_attr.into(), 0);
            });
        } else {
            reply.error(ENOENT);
        }
    }
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        info!("fetch attr for inode: {}", ino);
        let root = File::new(align(DumbFsMeta::serialize_size()), self.disk.clone());
        if root.header().fixed_sized_part.file_attr.ino == ino {
            reply.attr(&TTL, &root.header().fixed_sized_part.file_attr.into())
        } else {
            root.children()
                .find(|it| it.header().fixed_sized_part.file_attr.ino == ino)
                .map(|it| reply.attr(&TTL, &it.header().fixed_sized_part.file_attr.into()));
        }
    }
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
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
}
