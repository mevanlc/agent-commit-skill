use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    Antigravity,
    Copilot,
    Codex,
    Claude,
}

impl Harness {
    pub const ALL: [Self; 4] = [Self::Antigravity, Self::Copilot, Self::Codex, Self::Claude];

    pub fn name(self) -> &'static str {
        match self {
            Self::Antigravity => "antigravity",
            Self::Copilot => "copilot",
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

impl fmt::Display for Harness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    All,
    Staged,
    Ask,
}

impl Mode {
    pub fn argument(self, yes: bool, push: bool) -> String {
        let mode = match self {
            Self::All => "all",
            Self::Staged => "staged",
            Self::Ask => "ask",
        };
        format!(
            "--{mode}{}{}",
            if yes && self != Self::Ask { "-yes" } else { "" },
            if push { "-push" } else { "" }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultra,
}

impl Effort {
    pub const ALL: [Self; 6] = [
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Xhigh,
        Self::Max,
        Self::Ultra,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
            Self::Ultra => "ultra",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EffortSelection {
    Shared(Effort),
    Raw(String),
    HarnessDefault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelection {
    Named(String),
    Literal(String),
    Size(ModelSize),
}

/// acsc's relative model strength, independent of the model's reasoning effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ModelSize {
    Normal,
    Large,
    Xlarge,
}

impl ModelSize {
    pub const ALL: [Self; 3] = [Self::Normal, Self::Large, Self::Xlarge];

    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Large => "large",
            Self::Xlarge => "xlarge",
        }
    }
}

impl fmt::Display for ModelSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Where a value was chosen. Cache metadata is deliberately not a default source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Cli,
    UserModel,
    UserHarness,
    UserGlobal,
    BundledModel,
    BundledHarness,
    BundledGlobal,
    Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Selection<T> {
    pub value: T,
    pub source: Source,
}

impl<T> Selection<T> {
    pub fn new(value: T, source: Source) -> Self {
        Self { value, source }
    }
}
