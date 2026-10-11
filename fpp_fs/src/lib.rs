use fpp_core::{Error, FileReader};
use std::fs;

/// A file reader for the file system
#[derive(Clone, Copy, Debug, Default)]
pub struct FsReader {}

impl FileReader for FsReader {
    fn read(&self, path: &str) -> Result<String, Error> {
        fs::read_to_string(path).map_err(|err| format!("cannot open file {path}: {err}").into())
    }
}
