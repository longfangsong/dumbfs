#[macro_use]
extern crate log;

use std::env;
use std::ffi::OsStr;

use crate::fs::DumbFS;

mod util;
mod test;
mod fs;
mod file_meta;

fn main() {
    env_logger::init();
    let disk = env::args_os().nth(1).unwrap();
    let mountpoint = env::args_os().nth(2).unwrap();
    info!("mount: {:?} on {:?}", disk, mountpoint);
    let options = ["-o", "rw", "-o", "fsname=dumbfs"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    let dumbfs = DumbFS::new(disk);
    fuse::mount(dumbfs, mountpoint, &options).unwrap();
}
