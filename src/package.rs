use std::{collections::HashMap, fmt::Display, path::PathBuf, process::Command, str::FromStr};

use anyhow::anyhow;
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

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StepVariant {
    Command {
        #[serde_as(as = "DisplayFromStr")]
        runner: Runner,
        command: String,
    },
    Move {
        path: PathBuf,
    },
}

#[derive(Debug)]
pub enum Runner {
    Shell,
}

impl Display for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shell => write!(f, "shell"),
        }
    }
}

impl FromStr for Runner {
    // FIXME: use an actual error
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "shell" => Ok(Self::Shell),
            _ => Err(anyhow!("Unknown runner")),
        }
    }
}

impl Runner {
    pub fn into_command(&self) -> Command {
        match self {
            Self::Shell => {
                let mut command = Command::new("/bin/sh");

                command.arg("-c");

                command
            }
        }
    }
}
