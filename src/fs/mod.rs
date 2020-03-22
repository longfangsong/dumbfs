use crate::disk::dump::DumpToFixedLocation;
use crate::disk::Disk;
use crate::file::dump_file_attr::FileAttrDump;
use crate::file::{dump_file_attr::FileTypeDump, File, FileBuilder};
use crate::fs::meta::DumbFsMeta;
use fuse::{
    Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyOpen, ReplyWrite, Request,
};
use libc::{EINVAL, EIO, ENOENT, ENOSYS, EPERM};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::FileType;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

mod meta;

const TTL: Duration = Duration::from_secs(1);

pub struct DumbFS {
    disk: Disk,
    meta: DumbFsMeta,
    next_file_handler: u64,
    opened_files: HashMap<u64, File>,
}

impl DumbFS {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        DumbFS {
            disk: Disk::new(path),
            meta: DumbFsMeta::default(),
            next_file_handler: 1,
            opened_files: HashMap::new(),
        }
    }
    fn init_filesystem(&mut self) {
        info!("init filesystem");
        self.meta = DumbFsMeta::default();
        let ino = self.meta.acquire_next_ino();
        assert_eq!(ino, 1);
        let root_dir = FileBuilder::new(&self.disk, self.meta.next_free_address)
            .ino(ino)
            .build();
        self.meta.next_free_address += 512;
        root_dir.sync(&self.disk);
        self.meta.sync(&self.disk);
    }
    fn find_file_with_root(&self, ino: u64, root: File) -> Option<File> {
        let kind = root.meta.file_attr.kind.clone();
        match kind {
            FileTypeDump::Directory => {
                if root.meta.file_attr.ino == ino {
                    Some(root)
                } else {
                    root.children()
                        .find_map(|it| self.find_file_with_root(ino, it))
                }
            }
            FileTypeDump::RegularFile => {
                if root.meta.file_attr.ino == ino {
                    Some(root)
                } else {
                    None
                }
            }
        }
    }
    fn find_file(&self, ino: u64) -> Option<File> {
        let root = File::load(&self.disk, 512).unwrap();
        assert_eq!(root.meta.file_attr.ino, 1);
        self.find_file_with_root(ino, root)
    }
}

impl Filesystem for DumbFS {
    fn init(&mut self, _req: &Request<'_>) -> Result<(), i32> {
        let meta = DumbFsMeta::load(&self.disk, 0);
        match meta {
            Ok(meta) => {
                if meta.valid() {
                    self.meta = meta
                } else {
                    self.init_filesystem();
                }
            }
            Err(_) => self.init_filesystem(),
        }
        Ok(())
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        debug!("lookup {:?} in ino={}", name, parent);
        let parent = self.find_file(parent);
        if let Some(parent) = parent {
            let found = parent
                .children()
                .find(|it| &it.meta.filename == name.to_str().unwrap());
            if let Some(found) = found {
                reply.entry(&TTL, &found.meta.file_attr.into(), 1)
            } else {
                reply.error(ENOENT);
            }
        } else {
            reply.error(ENOENT)
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        info!("getattr for ino={}", ino);
        let file = self.find_file(ino);
        if let Some(file) = file {
            info!("ino={}'s size = {}", ino, file.meta.file_attr.size);
            reply.attr(&TTL, &file.meta.file_attr.into())
        } else {
            error!("getattr failed for ino={}", ino);
            reply.error(ENOENT)
        }
    }

    fn open(&mut self, _req: &Request, ino: u64, flags: u32, reply: ReplyOpen) {
        let file = self.find_file(ino);
        if let Some(file) = file {
            let fh = self.next_file_handler;
            self.next_file_handler += 1;
            self.opened_files.insert(fh, file);
            info!("open ino={}, return fh={}", ino, fh);
            reply.opened(fh, 0);
        } else {
            reply.error(ENOENT)
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        reply: ReplyData,
    ) {
        info!("read with fh={}", fh);
        let file = self.opened_files.get_mut(&fh);
        if let Some(file) = file {
            let mut buffer = vec![0u8; size as usize];
            file.seek(SeekFrom::Start(offset as _)).unwrap();
            file.read_exact(&mut buffer).unwrap();
            reply.data(&buffer)
        } else {
            reply.error(EIO)
        }
    }

    fn write(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _flags: u32,
        reply: ReplyWrite,
    ) {
        info!("write into fh={}", fh);
        let file = self.opened_files.get_mut(&fh);
        if let Some(file) = file {
            file.seek(SeekFrom::Start(offset as _)).unwrap();
            file.write_all(data).unwrap();
            reply.written(data.len() as _)
        } else {
            reply.error(EIO)
        }
    }

    fn release(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        if !self.opened_files.contains_key(&fh) {
            return reply.error(EIO);
        }
        self.opened_files.remove(&fh);
        reply.ok()
    }

    fn fsync(&mut self, _req: &Request, _ino: u64, fh: u64, _datasync: bool, reply: ReplyEmpty) {
        let file = self.opened_files.get_mut(&fh);
        if let Some(file) = file {
            file.flush().unwrap();
            reply.ok()
        } else {
            reply.error(EIO)
        }
    }

    fn create(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        flags: u32,
        reply: ReplyCreate,
    ) {
        let parent = self.find_file(parent);
        if let Some(mut parent) = parent {
            if parent.meta.file_attr.kind != FileTypeDump::Directory {
                reply.error(EIO);
            } else {
                let at_address = self.meta.next_free_address;
                let new_created = FileBuilder::new(&self.disk, at_address)
                    .ino(self.meta.acquire_next_ino())
                    .kind(FileTypeDump::RegularFile.into())
                    .filename(name.to_str().unwrap())
                    .build();
                new_created.sync(&self.disk);
                if let Some(mut last_child) = parent.children().last() {
                    last_child.meta.next_sibling = at_address;
                    last_child.sync(&self.disk)
                } else {
                    parent.meta.first_child = at_address;
                    parent.sync(&self.disk);
                }
                self.meta.next_free_address = new_created.address_after_dump();
                self.meta.sync(&self.disk);
                let fh = self.next_file_handler;
                self.next_file_handler += 1;
                reply.created(
                    &TTL,
                    &new_created.meta.file_attr.clone().into(),
                    1,
                    fh,
                    flags,
                );
                self.opened_files.insert(fh, new_created);
            }
        } else {
            reply.error(ENOENT);
        }
    }

    fn opendir(&mut self, _req: &Request, ino: u64, flags: u32, reply: ReplyOpen) {
        info!("opendir: {}", ino);
        let file = self.find_file(ino);
        if let Some(file) = file {
            let fh = self.next_file_handler;
            self.next_file_handler += 1;
            self.opened_files.insert(fh, file);
            reply.opened(fh, flags);
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        _ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let dir = self.opened_files.get(&fh);
        if let Some(dir) = dir {
            for (i, entry) in dir.children().enumerate().skip(offset as _) {
                if reply.add(
                    entry.meta.file_attr.ino,
                    (i + 1) as i64,
                    entry.meta.file_attr.kind.into(),
                    &entry.meta.filename,
                ) {
                    break;
                }
            }
            reply.ok()
        } else {
            reply.error(EIO)
        }
    }

    fn releasedir(&mut self, _req: &Request, _ino: u64, fh: u64, _flags: u32, reply: ReplyEmpty) {
        if !self.opened_files.contains_key(&fh) {
            return reply.error(EIO);
        }

        self.opened_files.remove(&fh);
        reply.ok();
    }

    fn mkdir(&mut self, _req: &Request, parent: u64, name: &OsStr, _mode: u32, reply: ReplyEntry) {
        let parent = self.find_file(parent);
        if let Some(mut parent) = parent {
            if parent.meta.file_attr.kind != FileTypeDump::Directory {
                reply.error(EIO);
            } else {
                let at_address = self.meta.next_free_address;
                let new_created = FileBuilder::new(&self.disk, at_address)
                    .ino(self.meta.acquire_next_ino())
                    .kind(FileTypeDump::Directory.into())
                    .filename(name.to_str().unwrap())
                    .build();
                new_created.sync(&self.disk);
                if let Some(mut last_child) = parent.children().last() {
                    last_child.meta.next_sibling = at_address;
                    last_child.sync(&self.disk)
                } else {
                    parent.meta.first_child = at_address;
                    parent.sync(&self.disk);
                }
                self.meta.next_free_address = new_created.address_after_dump();
                self.meta.sync(&self.disk);
                let fh = self.next_file_handler;
                self.next_file_handler += 1;
                reply.entry(&TTL, &new_created.meta.file_attr.clone().into(), 1);
                self.opened_files.insert(fh, new_created);
            }
        } else {
            reply.error(ENOENT);
        }
    }
}
