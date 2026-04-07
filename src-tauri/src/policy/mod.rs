use std::collections::HashMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteDecision {
    Auto,
    RequireHuman,
    Deny,
}

impl WriteDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::RequireHuman => "require_human",
            Self::Deny => "deny",
        }
    }
}

impl Default for WriteDecision {
    fn default() -> Self {
        Self::Auto
    }
}

impl std::fmt::Display for WriteDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WriteDecision {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "require_human" => Ok(Self::RequireHuman),
            "deny" => Ok(Self::Deny),
            other => Err(format!("invalid write decision: {other}")),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RawPolicyConfig {
    pub default: Option<WriteDecision>,
    pub actions: Option<HashMap<String, WriteDecision>>,
}

#[derive(Clone, Debug)]
pub struct PolicyConfig {
    pub default: WriteDecision,
    pub actions: HashMap<String, WriteDecision>,
}

impl PolicyConfig {
    pub fn from_raw(raw: RawPolicyConfig) -> Self {
        Self {
            default: raw.default.unwrap_or_default(),
            actions: raw.actions.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PolicyEngine {
    config: PolicyConfig,
}

#[derive(Clone, Debug)]
pub struct PolicyViolation {
    pub action: String,
    pub decision: WriteDecision,
}

impl PolicyEngine {
    pub fn new(config: PolicyConfig) -> Self {
        Self { config }
    }

    pub fn decision_for(&self, action: &str) -> WriteDecision {
        self.config
            .actions
            .get(action)
            .copied()
            .unwrap_or(self.config.default)
    }

    pub fn enforce(&self, action: &str) -> Result<(), PolicyViolation> {
        match self.decision_for(action) {
            WriteDecision::Auto => Ok(()),
            decision => Err(PolicyViolation {
                action: action.to_owned(),
                decision,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PolicyConfig, PolicyEngine, RawPolicyConfig, WriteDecision};

    #[test]
    fn uses_action_override_when_present() {
        let config = PolicyConfig::from_raw(RawPolicyConfig {
            default: Some(WriteDecision::Auto),
            actions: Some(
                [("task.create".to_string(), WriteDecision::Deny)]
                    .into_iter()
                    .collect(),
            ),
        });
        let engine = PolicyEngine::new(config);

        assert_eq!(engine.decision_for("task.create"), WriteDecision::Deny);
        assert_eq!(engine.decision_for("task.update"), WriteDecision::Auto);
    }
}
