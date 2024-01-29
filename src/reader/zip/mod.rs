#[cfg(feature = "install")]
mod install;

use std::{ collections::HashSet, fs::File, io::{ Read, Seek }, ops::{ Deref, DerefMut }, path::Path };

use path_clean::clean;
use zip::{ result::ZipError, ZipArchive };

use crate::types::{ ModpackManifest, StdError };

use super::ModpackReader;

pub struct ModpackArchiveReader<T: Read + Seek>(ZipArchive<T>);

impl ModpackArchiveReader<File> {
  pub fn open(file_path: impl AsRef<Path>) -> Result<Self, StdError> {
    if !file_path.as_ref().is_file() {
      return Err("Provided path is not a file".into());
    }
    let file = File::open(file_path).map_err(|err| format!("Error opening file: {}", err))?;
    let archive = ZipArchive::new(file).map_err(|err| format!("Error opening archive: {}", err))?;
    Self::try_from(archive)
  }
}

impl<T: Read + Seek> ModpackArchiveReader<T> {
  pub fn into_inner(self) -> ZipArchive<T> {
    self.0
  }
}

impl<T: Read + Seek> ModpackReader for ModpackArchiveReader<T> {
  fn get_manifest(&mut self) -> Result<ModpackManifest, StdError> {
    let manifest = self.0.by_name("manifest.json")?;
    Ok(serde_json::from_reader(manifest)?)
  }

  fn open_file<'a>(&'a mut self, path: &str) -> Result<Box<dyn Read + 'a>, StdError> {
    // TODO: weird, but whatever

    let mut file_name: Option<String> = None;
    let path = clean(path);
    for i in 0..self.len() {
      let file = self.by_index(i).unwrap();
      if clean(file.name()) == path {
        file_name = Some(file.name().to_string());
      }
    }
    if let Some(name) = file_name {
      let file = self.by_name(&name)?;
      return Ok(Box::new(file));
    }
    return Err(ZipError::FileNotFound.into());
  }

  fn read_dir(&mut self, path: &str) -> Result<Vec<String>, StdError> {
    if !self.is_dir(path) {
      return Err("Provided path is not a directory".into());
    }
    let path = clean(path);
    let mut names = HashSet::new();

    for i in 0..self.len() {
      let entry = self.0.by_index(i)?;
      let clean_zip_name = clean(entry.name());

      let name = if path.to_string_lossy() == "." {
        &clean_zip_name
      } else if let Ok(n) = clean_zip_name.strip_prefix(&path) {
        n
      } else {
        continue;
      };

      if name.components().count() == 1 {
        let name = name.to_string_lossy().to_string();
        if !name.is_empty() {
          names.insert(name);
        }
      }
    }

    Ok(names.into_iter().collect())
  }

  fn exists(&mut self, path: &str) -> bool {
    let path = clean(path);
    for i in 0..self.len() {
      let file = self.by_index(i).unwrap();
      if clean(file.name()) == path {
        return true;
      }
    }
    return false;
  }

  fn is_file(&mut self, path: &str) -> bool {
    let path = clean(path);
    for i in 0..self.len() {
      let file = self.by_index(i).unwrap();
      if clean(file.name()) == path {
        return file.is_file();
      }
    }
    return false;
  }

  fn is_dir(&mut self, path: &str) -> bool {
    let path = clean(path);
    for i in 0..self.len() {
      let file = self.by_index(i).unwrap();
      if clean(file.name()) == path {
        return file.is_dir();
      }
    }
    return false;
  }
}

impl<T: Read + Seek> TryFrom<ZipArchive<T>> for ModpackArchiveReader<T> {
  type Error = StdError;

  fn try_from(value: ZipArchive<T>) -> Result<Self, Self::Error> {
    let mut new = Self(value);
    new.validate()?;
    Ok(new)
  }
}

impl<T: Read + Seek> Deref for ModpackArchiveReader<T> {
  type Target = ZipArchive<T>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: Read + Seek> DerefMut for ModpackArchiveReader<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
