//! Cleanup removes only the same regular inode/file ID created by this operation.
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
pub struct OwnedPng {
    file: File,
    path: PathBuf,
}
impl OwnedPng {
    pub fn create(directory: &Path, bytes: &[u8]) -> Result<Self, String> {
        let mut temp = tempfile::Builder::new()
            .prefix("blurhash-owned-")
            .suffix(".png")
            .tempfile_in(directory)
            .map_err(|_| "Cannot create private clipboard PNG")?;
        temp.write_all(bytes)
            .map_err(|_| "Cannot write private clipboard PNG")?;
        let (file, path) = temp
            .keep()
            .map_err(|_| "Cannot retain private clipboard PNG")?;
        Ok(Self { file, path })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    fn same_file(&self) -> bool {
        let Ok(meta) = std::fs::symlink_metadata(&self.path) else {
            return false;
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.file
                .metadata()
                .is_ok_and(|original| original.dev() == meta.dev() && original.ino() == meta.ino())
        }
        #[cfg(windows)]
        {
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
                FILE_FLAG_OPEN_REPARSE_POINT,
            };
            let Ok(current) = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(&self.path)
            else {
                return false;
            };
            unsafe {
                let mut a: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
                let mut b: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
                GetFileInformationByHandle(self.file.as_raw_handle() as _, &mut a) != 0
                    && GetFileInformationByHandle(current.as_raw_handle() as _, &mut b) != 0
                    && a.dwVolumeSerialNumber == b.dwVolumeSerialNumber
                    && a.nFileIndexHigh == b.nFileIndexHigh
                    && a.nFileIndexLow == b.nFileIndexLow
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            false
        }
    }
}
impl Drop for OwnedPng {
    fn drop(&mut self) {
        if self.same_file() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
