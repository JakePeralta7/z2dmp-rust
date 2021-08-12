use std::fs;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;

use crate::result::{Result, Error};

#[derive(Debug)]
pub struct File {
    file: std::fs::File,
    path: String,
}

type IoResult<T> = std::result::Result<T, std::io::Error>;

impl File {
    pub fn create<'a>(path: &'a Path) -> Result<File> {
        let file = std::fs::File::create(path)
            .map_err(|e| Error::IoError(format!(
            "Failed to create `{}`: {}", path.display(), e)))?;

        Ok(File { file, path: path.display().to_string() })
    }

    pub fn open<'a>(path: &'a Path) -> Result<File> {
        let file = std::fs::File::open(path)
            .map_err(|e| Error::IoError(format!(
            "Failed to open `{}`: {}", path.display(), e)))?;

        Ok(File { file, path: path.display().to_string() })
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        self.file.seek(pos).map_err(|e| {
            let s = match pos {
                SeekFrom::Start(x) => format!(
                    "Failed to seek `{}` {} from start: {}", self.path, x, e),

                SeekFrom::End(x) => format!(
                    "Failed to seek `{}` {} from end: {}", self.path, x, e),

                SeekFrom::Current(x) => format!(
                    "Failed to seek `{}` {} from current: {}", self.path, x, e),
            };

            std::io::Error::new(std::io::ErrorKind::Other, s)
        })
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.file.read(buf).map_err(|e|
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read `{}`: {}", self.path, e)))
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.file.write(buf).map_err(|e|
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write `{}`: {}", self.path, e)))
    }

    fn flush(&mut self) -> IoResult<()> {
        self.file.flush().map_err(|e|
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to flush `{}`: {}", self.path, e)))
    }
}

pub fn create_dir_all<'a>(dir: &'a Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|e|
        Error::IoError(format!(
            "Failed to create directory: `{}`: {}", dir.display(), e)))
}