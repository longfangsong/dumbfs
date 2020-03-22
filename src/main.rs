#[macro_use]
extern crate log;

use crate::fs::DumbFS;
use std::env;
use std::ffi::OsStr;

mod disk;
mod file;
mod fs;
mod util;

fn main() {
    env_logger::init();
    let disk = env::args_os().nth(1).unwrap();
    let mountpoint = env::args_os().nth(2).unwrap();
    info!("mount: {:?} on {:?}", disk, mountpoint);
    let options = ["-o", "rw,default_permissions", "-o", "fsname=dumbfs"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    let dumbfs = DumbFS::new(disk);
    fuse::mount(dumbfs, mountpoint, &options).unwrap();
}
