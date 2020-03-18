#[macro_use]
extern crate nix;

use std::env;
use std::ffi::OsStr;

use fs::DumbFS;

mod dump_file_attr;
mod file_node;
mod fs;

fn main() {
    env_logger::init();
    let disk = env::args_os().nth(1).unwrap();
    let mountpoint = env::args_os().nth(2).unwrap();
    println!("mount: {:?} on {:?}", disk, mountpoint);
    let options = ["-o", "rw", "-o", "fsname=dumbfs"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    let dumbfs = DumbFS::new(disk);
    fuse::mount(dumbfs, mountpoint, &options).unwrap();
}