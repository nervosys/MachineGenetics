#![allow(dead_code)]

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Redox project configuration — parsed from `Forge.toml`.
#[derive(Debug, Deserialize)]
pub struct ForgeConfig {
    pub module: ModuleConfig,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, DependencySpec>,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub safety: SafetyConfig,
    #[serde(default)]
    pub agent: AgentConfig,
}

#[derive(Debug, Deserialize)]
pub struct ModuleConfig {
    pub name: String,
    pub version: String,
    #[serde(default = "default_edition")]
    pub edition: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub authors: Vec<String>,
}

fn default_edition() -> String {
    "2025".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DependencySpec {
    Simple(String),
    Detailed {
        version: String,
        #[serde(default)]
        features: Vec<String>,
    },
}

#[derive(Debug, Default, Deserialize)]
pub struct BuildConfig {
    #[serde(default)]
    pub target: Vec<String>,
    #[serde(default)]
    pub mlir_cache: bool,
    #[serde(default)]
    pub parallel: bool,
}

#[derive(Debug, Deserialize)]
pub struct SafetyConfig {
    #[serde(default = "default_safety_mode")]
    pub mode: String,
    #[serde(default = "default_safety_profile")]
    pub profile: String,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            mode: default_safety_mode(),
            profile: default_safety_profile(),
        }
    }
}

fn default_safety_mode() -> String {
    "skb-only".to_string()
}

fn default_safety_profile() -> String {
    "agent-dev".to_string()
}

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    #[serde(default = "default_swarm_size")]
    pub swarm_size: u32,
    #[serde(default = "default_consensus")]
    pub consensus: String,
    #[serde(default = "default_lease_timeout")]
    pub lease_timeout: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            swarm_size: default_swarm_size(),
            consensus: default_consensus(),
            lease_timeout: default_lease_timeout(),
        }
    }
}

fn default_swarm_size() -> u32 {
    4
}
fn default_consensus() -> String {
    "majority".to_string()
}
fn default_lease_timeout() -> String {
    "5m".to_string()
}

/// Find `Forge.toml` by searching current directory and ancestors.
pub fn find_config() -> Result<PathBuf, String> {
    let mut dir = std::env::current_dir().map_err(|e| format!("cannot get cwd: {e}"))?;
    loop {
        let candidate = dir.join("Forge.toml");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            return Err("could not find Forge.toml in current directory or any parent".to_string());
        }
    }
}

/// Load and parse the project config.
pub fn load_config() -> Result<ForgeConfig, String> {
    let path = find_config()?;
    load_config_from(&path)
}

pub fn load_config_from(path: &Path) -> Result<ForgeConfig, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    toml::from_str(&content).map_err(|e| format!("invalid Forge.toml: {e}"))
}
