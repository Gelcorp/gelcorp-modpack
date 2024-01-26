pub mod reader;
pub mod types;

#[cfg(test)]
mod tests {
  use std::fs::File;

  use log::{ debug, info };
  use path_clean::clean;
  use simple_logger::SimpleLogger;

  use crate::reader::{ folder::ModpackFolderReader, zip::ModpackArchiveReader };

  use self::types::StdError;

  use super::*;

  #[test]
  fn test_modpack() -> Result<(), StdError> {
    let input_folder = clean("./input");
    let modpack_file = clean("./modpack.zip");

    SimpleLogger::new().init()?;

    if modpack_file.is_file() {
      info!("Modpack already exists at {}", modpack_file.display());
    } else {
      info!("Bundling modpack...");
      let mut folder_reader = ModpackFolderReader::open(input_folder)?;
      let mut file = File::create(&modpack_file)?;
      folder_reader.bundle(&mut file)?;
      debug!("Bundle created at {}!", modpack_file.display());
    }
    println!();

    info!("Reading modpack...");
    let mut modpack = ModpackArchiveReader::open(&modpack_file)?;
    info!("Installing...");
    modpack.install("./.minecraft", vec!["visuals".to_string()])?;
    info!("Done!");
    Ok(())
  }
}
