use std::collections::{BTreeMap, BTreeSet};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::{ExtensionRequirement, PluginId, RuntimeAbi, SurfaceSlot};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub description: String,
    pub version: Version,
    pub runtime: RuntimeRequirement,
    pub entrypoints: Entrypoints,
    pub hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub publisher: Option<Publisher>,
    #[serde(default)]
    pub signature: Option<SignatureMetadata>,
    #[serde(default)]
    pub surfaces: BTreeSet<SurfaceSlot>,
    #[serde(default)]
    pub triggers: Vec<TriggerDeclaration>,
    #[serde(default)]
    pub extensions: Vec<ExtensionRequirement>,
    #[serde(default)]
    pub capabilities: Vec<CapabilityDeclaration>,
    #[serde(default)]
    pub settings: Vec<SettingDeclaration>,
    #[serde(default)]
    pub budgets: ResourceBudgets,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRequirement {
    pub protocol: u16,
    pub abi: VersionReq,
    pub engine: PluginEngine,
    #[serde(default)]
    pub target: Option<String>,
}

impl RuntimeRequirement {
    pub fn accepts(&self, protocol: u16, abi: RuntimeAbi) -> bool {
        self.protocol == protocol && self.abi.matches(&abi.as_semver())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginEngine {
    PocketSurface,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TriggerDeclaration {
    TerminalHover {
        #[serde(default)]
        extensions: Vec<String>,
        #[serde(default)]
        schemes: Vec<String>,
    },
}

impl PluginManifest {
    pub fn accepts_terminal_hover(&self, target: &str) -> bool {
        let target_without_query = target
            .split(['?', '#'])
            .next()
            .unwrap_or(target)
            .to_ascii_lowercase();
        let extension = target_without_query
            .rsplit_once('.')
            .map(|(_, extension)| extension);
        let scheme = target_without_query
            .split_once("://")
            .map(|(scheme, _)| scheme);
        self.triggers.iter().any(|trigger| match trigger {
            TriggerDeclaration::TerminalHover {
                extensions,
                schemes,
            } => {
                let extension_matches = extensions.is_empty()
                    || extension.is_some_and(|candidate| {
                        extensions
                            .iter()
                            .any(|allowed| allowed.eq_ignore_ascii_case(candidate))
                    });
                let scheme_matches = schemes.is_empty()
                    || scheme.is_some_and(|candidate| {
                        schemes
                            .iter()
                            .any(|allowed| allowed.eq_ignore_ascii_case(candidate))
                    })
                    || scheme.is_none();
                extension_matches && scheme_matches
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entrypoints {
    pub bundle: String,
    pub assets: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Publisher {
    pub name: String,
    #[serde(default)]
    pub identifier: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureMetadata {
    pub algorithm: String,
    pub key_id: String,
    pub file: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDeclaration {
    pub capability: Capability,
    pub reason: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    TerminalContext,
    ResourceRead,
    NetworkFetch,
    PaneSurface,
    PaneMetadata,
    SettingsStore,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SettingDeclaration {
    pub key: String,
    pub title: String,
    pub description: String,
    #[serde(flatten)]
    pub value: SettingValueDeclaration,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SettingValueDeclaration {
    Boolean {
        default: bool,
    },
    Integer {
        default: i64,
        min: i64,
        max: i64,
        #[serde(default = "default_integer_step")]
        step: i64,
        #[serde(default)]
        unit: Option<String>,
    },
    String {
        default: String,
        #[serde(default)]
        choices: Vec<String>,
    },
}

fn default_integer_step() -> i64 {
    1
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResourceBudgets {
    pub memory_bytes: u64,
    pub execution_slice_ms: u32,
    pub queued_messages: u32,
    pub active_surfaces: u16,
    pub max_surface_pixels: u64,
    pub max_frame_rate: u16,
    pub cache_bytes: u64,
}

impl Default for ResourceBudgets {
    fn default() -> Self {
        Self {
            memory_bytes: 96 * 1024 * 1024,
            execution_slice_ms: 16,
            queued_messages: 128,
            active_surfaces: 4,
            max_surface_pixels: 4_194_304,
            max_frame_rate: 60,
            cache_bytes: 32 * 1024 * 1024,
        }
    }
}

impl ResourceBudgets {
    pub fn clamped_to(self, ceiling: Self) -> Self {
        Self {
            memory_bytes: self.memory_bytes.min(ceiling.memory_bytes),
            execution_slice_ms: self.execution_slice_ms.min(ceiling.execution_slice_ms),
            queued_messages: self.queued_messages.min(ceiling.queued_messages),
            active_surfaces: self.active_surfaces.min(ceiling.active_surfaces),
            max_surface_pixels: self.max_surface_pixels.min(ceiling.max_surface_pixels),
            max_frame_rate: self.max_frame_rate.min(ceiling.max_frame_rate),
            cache_bytes: self.cache_bytes.min(ceiling.cache_bytes),
        }
    }
}
