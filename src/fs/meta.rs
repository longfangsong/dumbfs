use std::io::{Seek, SeekFrom, Write};

use serde::{Deserialize, Serialize};

use crate::util::align;

pub const MAGIC: u32 = 0xAA55_9669;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DumbFsMeta {
    pub magic: u32,
    next_ino: u64,
    next_free_address: u64,
}

impl DumbFsMeta {
    pub fn serialize_size() -> u64 {
        bincode::serialized_size(&DumbFsMeta {
            magic: MAGIC,
            next_ino: 1,
            next_free_address: 0,
        })
            .unwrap()
    }
    pub fn new() -> Self {
        DumbFsMeta {
            magic: 0xAA55_9669,
            next_ino: 1,
            next_free_address: align(DumbFsMeta::serialize_size()),
        }
    }
    pub fn deserialize_from<R>(reader: R) -> bincode::Result<Self>
        where
            R: std::io::Read,
    {
        bincode::deserialize_from(reader)
    }
    pub fn serialize_into<W>(&self, writer: W) -> bincode::Result<()>
        where
            W: std::io::Write,
    {
        bincode::serialize_into(writer, self)
    }
    fn save<W>(&mut self, f: &mut W) where W: Write + Seek {
        f.seek(SeekFrom::Start(0)).unwrap();
        self.serialize_into(f).unwrap();
    }
    pub fn sync<W>(&mut self, f: &mut W) where W: Write + Seek {
        self.next_free_address = align(f.seek(SeekFrom::End(0)).unwrap());
        self.save(f);
    }
    pub fn acquire_next_ino<W>(&mut self, f: &mut W) -> u64
        where W: Write + Seek {
        let result = self.next_ino;
        self.next_ino += 1;
        self.save(f);
        result
    }
    pub fn next_free_address(&self) -> u64 { self.next_free_address }
}
