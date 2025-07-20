use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetConfig {
    pub name: String,
    pub out_dir: String,
    pub kind: String,
    pub sources: Vec<String>,
    pub includes: Option<Vec<String>>,
    pub defines: Option<Vec<String>>,
    pub linker_flags: Option<Vec<String>>,
    pub compiler_flags: Option<Vec<String>>,
    pub frameworks: Option<Vec<String>>, // MacOS frameworks
    pub os_target: String,
    pub compiler: String,
    pub pre_build_scripts: Option<Vec<String>>, // скрипты до сборки
    pub post_build_scripts: Option<Vec<String>>, // скрипты после сборки
    pub env: Option<Vec<(String, String)>>,
    pub working_dir: Option<String>,
    pub custom_output: Option<String>,
    pub extra_steps: Option<Vec<String>>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildConfig
{
    pub project: ProjectConfig,
    pub dependencies: Option<Vec<Dependency>>,
    pub targets: Vec<TargetConfig>,
    pub description: Option<String>,
    pub env: Option<Vec<(String, String)>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Dependency
{
    pub name: String,
    pub source: String,
    pub location: String,
}

pub fn load_config(path: &str) -> anyhow::Result<BuildConfig>
{
    let content = std::fs::read_to_string(path)?;

    if path.ends_with(".toml") || path.ends_with(".constructor") {
        Ok(toml::from_str(&content)?)
    } else if path.ends_with(".yaml") || path.ends_with(".yml") {
        Ok(serde_yaml::from_str(&content)?)
    } else {
        Err(anyhow::anyhow!("Unsupported config file format!"))
    }
}