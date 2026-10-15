//! Files written next to the database (invoice PDFs)

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use super::StorageError;

/// Write `bytes` to `path` atomically: a reader sees the old file or the new one, never half
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let fail = |source| StorageError::WriteFile {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let tmp = path.with_extension("tmp");
    let file = File::create(&tmp).map_err(fail)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(bytes).map_err(fail)?;
    let file = writer.into_inner().map_err(|e| fail(e.into_error()))?;
    file.sync_all().map_err(fail)?;
    fs::rename(&tmp, path).map_err(fail)
}
