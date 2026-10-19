use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use crate::config::{CodexCache, home_dir, model_aliases, parse_effort};
use crate::domain::{EffortSelection, Harness};
use crate::resolve::Resolver;

/// Suggestions only: none of these fields participate in launch resolution.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogModel {
    pub harness: Harness,
    pub id: String,
    pub aliases: Vec<String>,
    pub display_name: String,
    pub native_efforts: Vec<String>,
    pub advertised_default: Option<String>,
    pub configured_effort: Option<EffortSelection>,
    pub source: String,
    pub priority: i64,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub models: Vec<CatalogModel>,
}

#[derive(Default)]
struct CachedModel {
    display_name: Option<String>,
    native_efforts: Vec<String>,
    advertised_default: Option<String>,
    hidden: bool,
    priority: i64,
}

pub fn cache_path(config: &CodexCache) -> Option<PathBuf> {
    if !config.enabled {
        return None;
    }
    config.path.clone().or_else(|| {
        std::env::var_os("CODEX_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|home| home.join(".codex")))
            .map(|base| base.join("models_cache.json"))
    })
}

fn read_cache(config: &CodexCache) -> BTreeMap<String, CachedModel> {
    let Some(path) = cache_path(config) else {
        return BTreeMap::new();
    };
    let Some(value) = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    else {
        return BTreeMap::new();
    };
    let Some(models) = value.get("models").and_then(|v| v.as_array()) else {
        return BTreeMap::new();
    };
    models
        .iter()
        .filter_map(|model| {
            let id = model.get("slug")?.as_str()?.to_owned();
            if id.trim().is_empty() {
                return None;
            }
            let native_efforts = model
                .get("supported_reasoning_levels")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str().or_else(|| value.get("effort")?.as_str()))
                .map(str::to_owned)
                .collect();
            let cached = CachedModel {
                display_name: model
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                native_efforts,
                advertised_default: model
                    .get("default_reasoning_level")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                hidden: matches!(
                    model.get("visibility").and_then(|v| v.as_str()),
                    Some("hide" | "hidden")
                ),
                priority: model
                    .get("priority")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(i64::MAX),
            };
            Some((id, cached))
        })
        .collect()
}

impl Catalog {
    pub fn load(resolver: &Resolver) -> Result<Self> {
        let mut cache = read_cache(&resolver.user.discovery.codex_cache);
        let mut models = Vec::new();
        for harness in Harness::ALL {
            let (bundled, user) = resolver.harness_configs(harness);
            let aliases = model_aliases(&bundled, &user)?;
            let mut ids: BTreeSet<_> = bundled
                .models
                .keys()
                .chain(user.models.keys())
                .cloned()
                .collect();
            if harness == Harness::Codex {
                ids.extend(
                    cache
                        .iter()
                        .filter(|(_, model)| !model.hidden)
                        .map(|(id, _)| id.clone()),
                );
            }
            for id in ids {
                let cached = if harness == Harness::Codex {
                    cache.remove(&id).unwrap_or_default()
                } else {
                    CachedModel::default()
                };
                let user_model = user.models.get(&id);
                let bundled_model = bundled.models.get(&id);
                let configured = user_model.or(bundled_model);
                let configured_effort = user_model
                    .and_then(|model| {
                        parse_effort(model.effort.as_deref(), model.raw_effort.as_deref())
                            .ok()
                            .flatten()
                    })
                    .or_else(|| {
                        bundled_model.and_then(|model| {
                            parse_effort(model.effort.as_deref(), model.raw_effort.as_deref())
                                .ok()
                                .flatten()
                        })
                    });
                let display_name = user_model
                    .and_then(|m| m.display_name.clone())
                    .or_else(|| bundled_model.and_then(|m| m.display_name.clone()))
                    .or(cached.display_name)
                    .unwrap_or_else(|| id.clone());
                let native_efforts = user_model
                    .and_then(|m| m.native_efforts.clone())
                    .or_else(|| bundled_model.and_then(|m| m.native_efforts.clone()))
                    .unwrap_or(cached.native_efforts);
                let source = if user_model.is_some() {
                    "user"
                } else if configured.is_some() {
                    "bundled"
                } else {
                    "cache"
                };
                models.push(CatalogModel {
                    harness,
                    aliases: aliases
                        .iter()
                        .filter(|(_, target)| **target == id)
                        .map(|(alias, _)| alias.clone())
                        .collect(),
                    display_name,
                    native_efforts,
                    configured_effort,
                    id,
                    advertised_default: cached.advertised_default,
                    source: source.into(),
                    priority: cached.priority,
                });
            }
        }
        models.sort_by(|a, b| (a.harness, a.priority, &a.id).cmp(&(b.harness, b.priority, &b.id)));
        Ok(Self { models })
    }

    pub fn for_harness(&self, harness: Option<Harness>) -> impl Iterator<Item = &CatalogModel> {
        self.models
            .iter()
            .filter(move |model| harness.is_none_or(|h| h == model.harness))
    }

    pub fn find(&self, harness: Harness, name: &str) -> Option<&CatalogModel> {
        self.models
            .iter()
            .find(|m| m.harness == harness && m.id == name)
            .or_else(|| {
                self.models
                    .iter()
                    .find(|m| m.harness == harness && m.aliases.iter().any(|alias| alias == name))
            })
    }
}

/// Reused by informational renderers; cache defaults are labeled separately.
pub fn effort_label(effort: &EffortSelection) -> String {
    match effort {
        EffortSelection::Shared(level) => level.name().into(),
        EffortSelection::Raw(value) => format!("raw:{value}"),
        EffortSelection::HarnessDefault => "harness-default".into(),
    }
}
