use crate::disk::Disk;
use bincode::{serialized_size, Error};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub trait DumpToFixedLocation<DumpPart: Serialize + DeserializeOwned>: Sized {
    fn dump_part(&self) -> DumpPart;
    fn location(&self) -> u64;
    fn load(disk: &Disk, address: u64) -> Result<Self, Error>;
    fn dump_size(&self) -> u64 {
        let dump_part = self.dump_part();
        serialized_size(&dump_part).unwrap()
    }
    fn address_after_dump(&self) -> u64 {
        self.location() + self.dump_size()
    }
    fn sync(&self, disk: &Disk) {
        disk.dump_fixed_location(self)
    }
}
