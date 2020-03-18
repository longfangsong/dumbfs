use serde::{Deserialize, Serialize};

use crate::dump_file_attr::FileAttrDump;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileNode {
    pub first_child: u64,
    pub next_sibling: u64,
    pub file_attr: FileAttrDump,
}