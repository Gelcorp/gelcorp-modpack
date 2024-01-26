pub mod zip;
#[cfg(feature = "folder-reader")]
pub mod folder;

use std::io::Read;
use path_clean::clean;
use crate::types::{ ExtractionRule, ModpackManifest, StdError };

// TODO: ModpackReaderError

pub trait ModpackReader {
  fn get_manifest(&mut self) -> Result<ModpackManifest, StdError>;
  fn validate(&mut self) -> Result<ModpackManifest, StdError> {
    // Get manifest
    let manifest = self.get_manifest()?;

    // Check mods dir
    if !self.is_dir("mods/") {
      return Err("Folder mods/ doesn't exist".into());
    }

    for optional in &manifest.optionals {
      let optional_dir = format!("mods/{}/", &optional.id);
      if !self.is_dir(&optional_dir) {
        return Err(format!("Error loading optional '{}': Folder {} doesn't exist", optional.id, optional_dir).into());
      }
    }

    for rule in &manifest.extraction_rules {
      match rule {
        ExtractionRule::Extract { from, to, .. } => {
          let from = clean(from);
          println!("Checking extract rule '{}'...", from.display());
          if from.is_absolute() || from.to_str().unwrap().contains("..") {
            return Err(format!("Error loading extraction rule '{}': Invalid path", from.display()).into());
          }
          if let Some(to) = to {
            let to = clean(to);
            if to.is_absolute() || to.to_str().unwrap().contains("..") {
              return Err(format!("Error loading extraction rule '{}': Invalid path", to.display()).into());
            }
          }
          if !self.exists(from.to_str().unwrap()) {
            return Err(format!("Error loading extraction rule '{}': Doesn't exist", from.display()).into());
          }
        }
        ExtractionRule::Remove { path } => {
          let path = clean(path);
          if path.is_absolute() || path.to_str().unwrap().contains("..") {
            return Err(format!("Error loading removal rule '{}': Invalid path", path.display()).into());
          }
        }
      }
    }
    Ok(manifest)
  }

  fn exists(&mut self, path: &str) -> bool;
  fn is_file(&mut self, path: &str) -> bool;
  fn is_dir(&mut self, path: &str) -> bool;

  fn open_file<'a>(&'a mut self, path: &str) -> Result<Box<dyn Read + 'a>, StdError>;
  fn read_dir(&mut self, path: &str) -> Result<Vec<String>, StdError>;
}
