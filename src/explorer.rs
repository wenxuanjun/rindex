use anyhow::anyhow;
use serde::Serialize;
use std::{cmp::Ordering, fs::DirEntry};

#[derive(Serialize, PartialEq, Eq)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum ExplorerEntry {
    Directory {
        mtime: String,
        name: String,
    },
    File {
        mtime: String,
        name: String,
        size: u64,
    },
}

impl Ord for ExplorerEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Directory { name: name_a, .. }, Self::Directory { name: name_b, .. })
            | (Self::File { name: name_a, .. }, Self::File { name: name_b, .. }) => {
                name_a.cmp(name_b)
            }
            (Self::Directory { .. }, _) => Ordering::Less,
            (_, Self::Directory { .. }) => Ordering::Greater,
        }
    }
}

impl PartialOrd for ExplorerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ExplorerEntry {
    #[inline]
    pub fn new(file: &DirEntry) -> anyhow::Result<Self> {
        let metadata = file.metadata()?;
        let modified = metadata.modified()?;
        let mtime = httpdate::fmt_http_date(modified);
        let name = file
            .file_name()
            .to_str()
            .ok_or(anyhow!("Invalid file name"))?
            .to_string();

        let explorer_entry = if metadata.is_dir() {
            Self::Directory { name, mtime }
        } else {
            let size = metadata.len();
            Self::File { name, size, mtime }
        };

        Ok(explorer_entry)
    }
}
