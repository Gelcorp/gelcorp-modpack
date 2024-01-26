use std::{ fs::{ read_dir, File }, io::{ self, Read, Seek, Write }, path::{ Path, PathBuf } };

use path_clean::clean;
use walkdir::WalkDir;
use zip::{ write::FileOptions, CompressionMethod, ZipWriter };

use crate::types::{ ModpackManifest, StdError };

use super::ModpackReader;

pub struct ModpackFolderReader(PathBuf);

impl ModpackFolderReader {
  pub fn open(folder_path: impl AsRef<Path>) -> Result<Self, StdError> {
    if !folder_path.as_ref().is_dir() {
      return Err("Provided path is not a directory".into());
    }
    let mut new = Self(clean(folder_path.as_ref()));
    new.validate()?;
    Ok(new)
  }

  pub fn path(&self) -> &PathBuf {
    &self.0
  }
}

impl ModpackReader for ModpackFolderReader {
  fn get_manifest(&mut self) -> Result<ModpackManifest, StdError> {
    let manifest = self.0.join("manifest.json");
    if !manifest.is_file() {
      return Err("Error loading manifest: File manifest.json doesn't exist".into());
    }
    let manifest: ModpackManifest = serde_json::from_reader(File::open(manifest)?)?;
    Ok(manifest)
  }

  fn open_file(&mut self, path: &str) -> Result<Box<dyn Read>, StdError> {
    let path = self.path().join(clean(path));
    if !path.is_file() {
      return Err("Provided path is not a file".into());
    }
    Ok(Box::new(File::open(path)?))
  }

  fn read_dir(&mut self, path: &str) -> Result<Vec<String>, StdError> {
    let path = self.path().join(clean(path));
    if !path.is_dir() {
      return Err("Provided path is not a directory".into());
    }
    let files = read_dir(path)?
      .filter_map(|e| e.ok())
      .map(|e| e.file_name().to_string_lossy().to_string())
      .collect();
    Ok(files)
  }

  fn exists(&mut self, path: &str) -> bool {
    self.path().join(clean(path)).exists()
  }

  fn is_file(&mut self, path: &str) -> bool {
    self.path().join(clean(path)).is_file()
  }

  fn is_dir(&mut self, path: &str) -> bool {
    self.path().join(clean(path)).is_dir()
  }
}

impl ModpackFolderReader {
  pub fn bundle<W: Write + Seek>(&mut self, writer: &mut W) -> Result<(), StdError> {
    self.validate()?;

    let mut archive = ZipWriter::new(writer);
    let options = FileOptions::default().compression_method(CompressionMethod::Bzip2).unix_permissions(0o755);

    let walkdir = WalkDir::new(&self.0).min_depth(1);

    for entry in walkdir {
      let entry = entry?;
      let path = entry.path().to_path_buf();
      let relative_path = clean(path.strip_prefix(&self.0)?);
      let relative_path = relative_path.to_string_lossy().replace("\\", "/"); // Windows

      if path.is_dir() {
        archive.add_directory(relative_path, options)?;
        continue;
      }
      archive.start_file(relative_path, options)?;
      let mut file = File::open(path)?;
      io::copy(&mut file, &mut archive)?;
    }

    Ok(())
  }
}
