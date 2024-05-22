use anyhow::Result;
use serde::Serialize;
use std::{cmp::Ordering, fs, fs::DirEntry};
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum ExplorerError {
    #[error("Symlink to a non-existent target: {0}")]
    MissingSymlinkTarget(String),
    #[error("Invalid file name: {0}")]
    InvalidFileName(String),
}

impl ExplorerEntry {
    #[inline]
    pub fn new(file: &DirEntry) -> Result<Self> {
        let path = file.path();

        let metadata = fs::metadata(&path).map_err(|_| {
            ExplorerError::MissingSymlinkTarget(path.to_string_lossy().into_owned())
        })?;

        let name = file
            .file_name()
            .to_str()
            .ok_or_else(|| ExplorerError::InvalidFileName(path.to_string_lossy().into_owned()))?
            .to_string();

        let modified = metadata.modified()?;
        let mtime = httpdate::fmt_http_date(modified);

        let explorer_entry = if metadata.is_dir() {
            Self::Directory { name, mtime }
        } else {
            Self::File {
                name,
                size: metadata.len(),
                mtime,
            }
        };

        Ok(explorer_entry)
    }
}
