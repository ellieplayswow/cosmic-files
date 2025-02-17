use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use trash::TrashItem;
use walkdir::WalkDir;
use crate::operation::OperationError;

pub(crate) trait Shreddable {
    fn shred(&self) -> Result<(), OperationError>;
}

impl Shreddable for PathBuf {
    fn shred(&self) -> Result<(), OperationError> {
        // if it's a directory, we can't shred it but will he handled elsewhere
        if !self.is_file() {
            return Ok(());
        }

        if !self.exists() {
            return Err(OperationError::from_str("File does not exist"));
        }

        shred_by_path(self)
    }
}

impl Shreddable for TrashItem {
    fn shred(&self) -> Result<(), OperationError> {
        let info_file = self.id.clone();
        // crawl up 2 directories, eg:
        // /home/cosmic/.local/share/Trash/info/foo.trashinfo -> /home/cosmic/.local/share/Trash/
        let trash_folder = Path::new(&info_file)
            .parent()
            .map_or(Err("Parent does not exist"), |p| Ok(p))
            .map_err(OperationError::from_str)?
            .parent()
            .map_or(Err("Parent does not exist"), |p| Ok(p))
            .map_err(OperationError::from_str)?;

        let name_in_trash = Path::new(&info_file)
            .file_stem()
            .map_or(Err("File has no name"), |p| Ok(p))
            .map_err(OperationError::from_str)?;

        let full_file = trash_folder
            .join("files")
            .join(name_in_trash);

        // full_file can be a dir here, but there's no guarantee the trash crate will provide
        // files within, so we should handle dirs
        if full_file.is_dir() {
            let new_paths_it = WalkDir::new(full_file.clone())
                .into_iter();

            for entry in new_paths_it.skip(1) {
                let entry = entry.map_err(OperationError::from_str)?;
                entry.into_path()
                    .shred()
                    .map_err(OperationError::from_str)?;
            }

            // recursively remove any dirs involved
            std::fs::remove_dir_all(full_file)
                .map_err(OperationError::from_str)?;

            std::fs::remove_file(info_file)
                .map_err(OperationError::from_str)?;

            Ok(())
        }
        else {
            let shred_res = full_file.shred();
            std::fs::remove_file(info_file)
                .map_err(OperationError::from_str)?;
            shred_res
        }
    }
}

/// Shred a single file by its `path`
///
/// # Examples
///
/// ```rs
/// if shred_by_path(PathBuf::from("/path/to/my/file.txt")).is_ok() {
///     println!("File successfully shredded")
/// }
/// ```
fn shred_by_path(path: &PathBuf) -> Result<(), OperationError> {
    /*
     In shred mode, we want to:
     - open the file for writing;
     - rewrite the entire file in 4096 byte chunks of `\0`;
     - flush & sync the new contents;
     - THEN remove the file normally
    */
    let buffer_size = 4096;

    let mut file = OpenOptions::new()
        .write(true)
        .create(false)
        .truncate(false)
        .open(path.clone())
        .map_err(OperationError::from_str)?;

    let file_size = file
        .metadata()
        .map_err(OperationError::from_str)?
        .len();

    let zero_buffer = vec![0u8; buffer_size];
    let mut bytes_written = 0;
    while bytes_written < file_size {
        file.write_all(&zero_buffer)
            .map_err(OperationError::from_str)?;
        bytes_written += buffer_size as u64;
    }

    file.flush()
        .map_err(OperationError::from_str)?;

    file.sync_all()
        .map_err(OperationError::from_str)?;

    std::fs::remove_file(path)
        .map_err(OperationError::from_str)?;

    Ok(())
}