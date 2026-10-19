use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::domain::{Effort, EffortSelection, Harness, Mode};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Defaults {
    pub harness: Option<Harness>,
    pub mode: Option<Mode>,
    pub skill: Option<String>,
    pub effort: Option<String>,
    pub raw_effort: Option<String>,
    pub yes: Option<bool>,
    pub push: Option<bool>,
    pub fast: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelConfig {
    pub aliases: Option<Vec<String>>,
    pub display_name: Option<String>,
    pub effort: Option<String>,
    pub raw_effort: Option<String>,
    pub effort_map: BTreeMap<Effort, String>,
    pub native_efforts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessConfig {
    pub mode: Option<Mode>,
    pub skill: Option<String>,
    pub effort: Option<String>,
    pub raw_effort: Option<String>,
    pub yes: Option<bool>,
    pub push: Option<bool>,
    pub fast: Option<bool>,
    pub model: Option<String>,
    pub command: Option<Vec<String>>,
    pub base_args: Option<Vec<String>>,
    pub effort_map: BTreeMap<Effort, String>,
    pub native_settings: BTreeMap<String, serde_json::Value>,
    pub models: BTreeMap<String, ModelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CodexCache {
    pub enabled: bool,
    pub path: Option<PathBuf>,
}

impl Default for CodexCache {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Discovery {
    pub codex_cache: CodexCache,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub defaults: Defaults,
    pub harnesses: BTreeMap<Harness, HarnessConfig>,
    pub discovery: Discovery,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConfigChoice {
    #[default]
    Default,
    File(PathBuf),
    Disabled,
}

pub fn parse_effort(effort: Option<&str>, raw: Option<&str>) -> Result<Option<EffortSelection>> {
    match (effort, raw) {
        (Some(_), Some(_)) => bail!("effort and raw_effort are mutually exclusive at each scope"),
        (Some("harness-default"), None) => Ok(Some(EffortSelection::HarnessDefault)),
        (Some(value), None) => {
            let level = Effort::ALL
                .into_iter()
                .find(|level| level.name() == value)
                .with_context(|| format!("invalid effort {value:?}; use low, medium, high, xhigh, max, ultra, or harness-default"))?;
            Ok(Some(EffortSelection::Shared(level)))
        }
        (None, Some(value)) => {
            nonempty(value, "raw_effort")?;
            Ok(Some(EffortSelection::Raw(value.to_owned())))
        }
        (None, None) => Ok(None),
    }
}

pub fn nonempty(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(())
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text).context("invalid acsc TOML configuration")?;
        config.validate()?;
        Ok(config)
    }

    pub fn bundled() -> Self {
        let mut config = Self::parse(include_str!("defaults.toml"))
            .expect("bundled configuration must be valid");
        if cfg!(windows) {
            for harness in config.harnesses.values_mut() {
                if let Some(command) = &mut harness.command {
                    command[0].push_str(".ps1");
                }
            }
        }
        config
    }

    pub fn load(choice: &ConfigChoice) -> Result<Self> {
        let (path, explicit) = match choice {
            ConfigChoice::Disabled => return Ok(Self::default()),
            ConfigChoice::File(path) => (path.clone(), true),
            ConfigChoice::Default => match default_path() {
                Some(path) => (path, false),
                None => return Ok(Self::default()),
            },
        };
        let text = match std::fs::read_to_string(&path) {
            Err(error) if !explicit && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            result => result.with_context(|| format!("cannot read config {}", path.display()))?,
        };
        Self::parse(&text).with_context(|| format!("config {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
        parse_effort(
            self.defaults.effort.as_deref(),
            self.defaults.raw_effort.as_deref(),
        )
        .context("defaults")?;
        if let Some(skill) = &self.defaults.skill {
            nonempty(skill, "defaults.skill")?;
        }
        for (name, harness) in &self.harnesses {
            let scope = format!("harnesses.{name}");
            parse_effort(harness.effort.as_deref(), harness.raw_effort.as_deref())
                .with_context(|| scope.clone())?;
            for (field, value) in [("model", &harness.model), ("skill", &harness.skill)] {
                if let Some(value) = value {
                    nonempty(value, &format!("{scope}.{field}"))?;
                }
            }
            if let Some(command) = &harness.command {
                let program = command
                    .first()
                    .with_context(|| format!("{scope}.command must contain an executable"))?;
                nonempty(program, &format!("{scope}.command executable"))?;
            }
            validate_map(&harness.effort_map, &scope)?;
            for (id, model) in &harness.models {
                nonempty(id, &format!("{scope} model ID"))?;
                if id == "harness-default" {
                    bail!("{scope}: harness-default is reserved; use a different model ID");
                }
                parse_effort(model.effort.as_deref(), model.raw_effort.as_deref())
                    .with_context(|| format!("{scope}.models.{id}"))?;
                validate_map(&model.effort_map, &format!("{scope}.models.{id}"))?;
            }
        }
        Ok(())
    }
}

fn validate_map(map: &BTreeMap<Effort, String>, scope: &str) -> Result<()> {
    for value in map.values() {
        nonempty(value, &format!("{scope}.effort_map value"))?;
    }
    Ok(())
}

/// Effective aliases, with user arrays replacing the bundled arrays per model.
/// Reject ambiguity before launching or presenting choices.
pub fn model_aliases(
    bundled: &HarnessConfig,
    user: &HarnessConfig,
) -> Result<BTreeMap<String, String>> {
    let ids: BTreeSet<_> = bundled
        .models
        .keys()
        .chain(user.models.keys())
        .cloned()
        .collect();
    let mut aliases = BTreeMap::new();
    for id in &ids {
        let names = user
            .models
            .get(id)
            .and_then(|m| m.aliases.as_ref())
            .or_else(|| bundled.models.get(id).and_then(|m| m.aliases.as_ref()));
        for alias in names.into_iter().flatten() {
            nonempty(alias, "model alias")?;
            if alias == "harness-default" {
                bail!("model alias harness-default is reserved");
            }
            if ids.contains(alias) && alias != id {
                bail!("model alias {alias:?} for {id:?} collides with a model ID");
            }
            if let Some(other) = aliases.insert(alias.clone(), id.clone())
                && other != *id
            {
                bail!("model alias {alias:?} is ambiguous between {other:?} and {id:?}");
            }
        }
    }
    Ok(aliases)
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn default_path() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA")
            .filter(|v| !v.is_empty())
            .map(|v| Path::new(&v).join("acsc/config.toml"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|home| home.join(".config")))
            .map(|base| base.join("acsc/config.toml"))
    }
}
