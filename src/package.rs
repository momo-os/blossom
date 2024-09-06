use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use spdx::Expression;

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    pub info: Info,
    pub dependencies: Option<Dependencies>,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub directories: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dependencies {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub optional: Vec<String>,
    #[serde(default)]
    pub build: Vec<String>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde_as(as = "DisplayFromStr")]
    pub license: Expression,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub checksum: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    #[serde(flatten)]
    pub variant: StepVariant,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StepVariant {
    Command { runner: String, command: String },
    Move { path: PathBuf },
}
