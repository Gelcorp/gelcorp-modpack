use serde::{ Serialize, Deserialize };

pub(crate) type StdError = Box<dyn std::error::Error>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModpackManifest {
  pub format_version: u8,
  pub minecraft_version: String,
  pub forge_version: String,
  pub java_version: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub optionals: Vec<ModOptional>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub extraction_rules: Vec<ExtractionRule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModOptional {
  pub name: String,
  pub description: String,
  pub id: String,
  pub icon: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub incompatible_with: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ExtractionRule {
  Extract {
    from: String,
    to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replace: Option<bool>,
  },
  Remove {
    path: String,
  },
}
