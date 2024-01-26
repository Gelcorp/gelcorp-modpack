// TODO: fix unused imports of "install" feature
use std::{
  collections::HashSet,
  fs::{ self, create_dir_all, File },
  io::{ copy, BufRead, BufReader, Read, Seek, Write },
  ops::{ Deref, DerefMut },
  path::{ Path, PathBuf },
};

use log::{ debug, error, info, warn };
use path_clean::clean;
use zip::{ result::ZipError, ZipArchive };

use crate::types::{ ExtractionRule, ModpackManifest, StdError };

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

// Installer

/*
Modpack structure:
mods/                     // Essential mods, mandatory
  - libs/                 // Essential libs (for essential mods, mandatory) 
  - {optional_mods}/      // Optional mods (performance, visuals, etc)
    - libs/
    - {mod}.jar
  - {mod}.jar             
.minecraft/               // Config Files, files to extract in general (replace = false by default, add extraction rules to override)
manifest.json
  - format_version: 1    // Format version of deserializer
*/

#[cfg(feature = "install")]
impl<T: Read + Seek> ModpackArchiveReader<T> {
  pub fn install(&mut self, mc_path: impl AsRef<Path>, optionals: Vec<String>) -> Result<(), StdError> {
    let mc_path = clean(mc_path.as_ref());
    let manifest = self.get_manifest()?;

    // Check if passed optionals are valid
    for optional in &optionals {
      if !manifest.optionals.iter().any(|o| o.id == *optional) {
        return Err(format!("Optional mod '{optional}' not found in manifest").into());
      }
    }

    let mods = mc_path.join("mods");
    let extracted_mods_path = mods.join("extracted_mods.txt");

    // Prepare mods folder
    prepare_mods_folder(&mods, &extracted_mods_path)?;

    info!("Extracting mods...");
    let mut extracted_mods = vec![];
    extracted_mods.extend(self.extract_mods("mods/", &mods)?);

    for optional in &manifest.optionals {
      if optionals.contains(&optional.id) {
        info!(" - Extracting optional mod '{}'", &optional.name);
        extracted_mods.extend(self.extract_mods(&format!("mods/{}/", optional.id), &mods)?);
      }
    }

    info!("Writing extracted mods list to {}", extracted_mods_path.display());
    let mut file = File::create(&extracted_mods_path)?;
    for name in &extracted_mods {
      writeln!(file, "{name}")?;
    }
    drop(file);
    info!("Mods extracted! Extracting config files...");

    debug!("Extracting .minecraft...");
    {
      let from = clean_zip_path(".minecraft/")?;
      if !self.is_dir(&from) {
        return Err(".minecraft not defined in manifest. Ignoring it".into());
      }
      for i in 0..self.len() {
        let mut file = self.by_index(i)?;
        if clean_zip_path(file.name())?.starts_with(&from) {
          let relative_path = clean_zip_path(&file.name()[from.len()..])?;
          let target_path = mc_path.join(&relative_path);
          if file.is_file() && !target_path.is_file() {
            copy(&mut file, &mut File::create(&target_path)?)?;
          } else if file.is_dir() {
            create_dir_all(&target_path)?;
          }
        }
      }
    }

    info!("Config files extracted! Extracting with custom rules...");
    for rule in &manifest.extraction_rules {
      match rule {
        ExtractionRule::Extract { from, to, replace } => {
          let from = clean_zip_path(format!("{from}/"))?;
          let to = clean_zip_path(to.as_ref().unwrap_or(&from))?;
          let replace = replace.unwrap_or(false);
          let target_path = mc_path.join(&to);

          if self.is_file(&from) {
            debug!("Extracting {} to {}", from, target_path.display());
            if target_path.is_file() && !replace {
              warn!("File {} already exists, skipping", target_path.display());
              continue;
            }
            let mut file = self.open_file(&from).map_err(|e| format!("Failed to open zip file {from}: {e}"))?;
            if let Some(parent) = target_path.parent() {
              create_dir_all(parent)?;
            }
            let mut target_file = File::create(&target_path)?;
            copy(&mut file, &mut target_file)?;
          } else if self.is_dir(&from) {
            debug!("Extracting {} to {}", from, target_path.display());
            for i in 0..self.len() {
              let mut file = self.by_index(i)?;
              if clean(file.name()).starts_with(&from) {
                let relative_path = clean_zip_path(&file.name()[from.len()..])?;
                let target_path = target_path.join(&relative_path);
                if file.is_dir() {
                  create_dir_all(&target_path)?;
                } else {
                  if target_path.is_file() && !replace {
                    warn!("File {} already exists, skipping", target_path.display());
                  }
                  if let Some(parent) = target_path.parent() {
                    create_dir_all(parent)?;
                  }
                  let mut target_file = File::create(&target_path)?;
                  copy(&mut file, &mut target_file)?;
                }
              }
            }
          } else {
            return Err(format!("File or directory {from} not found on zip").into());
          }
        }
        ExtractionRule::Remove { path } => {
          let path = clean_zip_path(path)?;
          if self.is_file(&path) {
            fs::remove_file(&mc_path.join(&path)).map_err(|e| format!("Failed to remove file {path}: {e}"))?;
          } else if self.is_dir(&path) {
            fs::remove_dir_all(&mc_path.join(&path)).map_err(|e| format!("Failed to remove dir {path}: {e}"))?;
          }
        }
      }
    }
    Ok(())
  }

  fn extract_mods(&mut self, mods_folder: &str, target_folder: &PathBuf) -> Result<Vec<String>, StdError> {
    let mods_folder = &clean_zip_path(mods_folder)?;
    let mut extracted_files = vec![];
    if self.is_dir(mods_folder) {
      create_dir_all(&target_folder)?;

      // Extract normal mods
      for file_name in self.read_dir(mods_folder)? {
        let path = clean_zip_path(format!("{mods_folder}/{file_name}"))?;
        if self.is_file(&path) && file_name.ends_with(".jar") {
          let mut file = self.open_file(&path).map_err(|e| format!("Failed to open file {path}: {e}"))?;
          let target_path = target_folder.join(&file_name);
          if target_path.exists() {
            warn!("File {} already exists, replacing it", target_path.display());
          }

          let mut target_file = File::create(target_path)?;
          copy(&mut file, &mut target_file)?;
          extracted_files.push(file_name);
        }
      }

      // Extract libs
      let libs_folder = clean_zip_path(format!("{mods_folder}/libs"))?;
      if self.is_dir(&libs_folder) {
        for file_name in self.read_dir(&libs_folder)? {
          let zip_path = format!("{libs_folder}/{file_name}");
          if self.is_file(&zip_path) && file_name.ends_with(".jar") {
            let mut file = self.open_file(&zip_path)?;
            let target_path = target_folder.join(&file_name);
            if target_path.exists() {
              warn!("File {} already exists, replacing", target_path.display());
            }
            let mut target_file = File::create(target_path)?;
            copy(&mut file, &mut target_file)?;
            extracted_files.push(file_name);
          }
        }
      }
    }
    Ok(extracted_files)
  }
}

#[cfg(feature = "install")]
fn prepare_mods_folder(mods_folder: &PathBuf, extracted_mods_file: &PathBuf) -> Result<(), StdError> {
  create_dir_all(mods_folder)?;
  info!("Extracting base mods...");

  if extracted_mods_file.is_file() {
    let file = File::open(&extracted_mods_file)?;
    let reader = BufReader::new(file);
    let lines: Vec<_> = reader.lines().collect();
    let len = lines.len();
    for (i, line) in lines.into_iter().enumerate() {
      let clean_line = clean_zip_path(line?)?; // Should be only the mod name, but just in case
      if clean_line.is_empty() {
        continue;
      }

      info!("Removing mod {clean_line}... ({}/{})", i + 1, len);
      if let Err(err) = fs::remove_file(mods_folder.join(&clean_line)) {
        error!("Failed to remove mod {clean_line}: '{err}'.");
      }
    }

    let _ = fs::remove_file(&extracted_mods_file)?;
  }
  Ok(())
}

#[cfg(feature = "install")]
fn clean_zip_path<T: AsRef<str>>(path: T) -> Result<String, String> {
  let path = path.as_ref().replace("\\", "/");
  let parts = path.split("/");

  let mut out = vec![];
  for part in parts {
    if part.is_empty() || part == "." {
      continue;
    }
    if part == ".." {
      if out.pop().is_none() {
        return Err("Path out of bounds!".into());
      }
    } else {
      out.push(part);
    }
  }
  let mut out_path = out.join("/");
  if path.ends_with("/") {
    out_path.push('/');
  }
  Ok(out_path)
}
