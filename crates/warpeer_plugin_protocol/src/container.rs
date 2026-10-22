use serde::{Deserialize, Serialize};

use crate::{
    ContainerId, ContextId, PageId, PluginId, RequestId, Revision, SurfaceContentSize, SurfaceId,
    SurfacePoint, SurfaceResizeEasing, SurfaceSlot, WorkerId,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerExtension {
    Chrome,
    Navigation,
    ResizeTransition,
    DragRegions,
}

impl ContainerExtension {
    pub const fn host_version(self) -> u16 {
        1
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionRequirement {
    pub extension: ContainerExtension,
    #[serde(default = "default_extension_version")]
    pub min_version: u16,
    #[serde(default = "default_extension_version")]
    pub max_version: u16,
    #[serde(default)]
    pub optional: bool,
}

impl ExtensionRequirement {
    pub fn accepts(&self, version: u16) -> bool {
        self.min_version > 0
            && self.min_version <= self.max_version
            && (self.min_version..=self.max_version).contains(&version)
    }

    pub fn is_valid(&self) -> bool {
        self.min_version > 0 && self.min_version <= self.max_version
    }
}

const fn default_extension_version() -> u16 {
    1
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionSupport {
    pub extension: ContainerExtension,
    pub version: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContainerLimits {
    pub max_stack_depth: u16,
    pub max_retained_pages: u16,
    pub max_retained_surfaces: u16,
    pub max_retained_pixels: u64,
    pub max_chrome_groups_per_side: u8,
    pub max_actions_per_group: u8,
    pub max_menu_depth: u8,
    pub max_drag_regions: u8,
    pub max_title_bytes: u16,
    pub max_transition_ms: u16,
    pub max_snapshot_bytes: u64,
    pub max_transition_snapshot_bytes: u64,
    pub max_transactions_per_second: u16,
}

impl Default for ContainerLimits {
    fn default() -> Self {
        Self {
            max_stack_depth: 1,
            max_retained_pages: 1,
            max_retained_surfaces: 1,
            max_retained_pixels: 16 * 1024 * 1024,
            max_chrome_groups_per_side: 4,
            max_actions_per_group: 8,
            max_menu_depth: 3,
            max_drag_regions: 8,
            max_title_bytes: 256,
            max_transition_ms: 600,
            max_snapshot_bytes: 16 * 1024 * 1024,
            max_transition_snapshot_bytes: 8 * 1024 * 1024,
            max_transactions_per_second: 120,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChromeVisibility {
    #[default]
    Visible,
    Hidden,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChromeActionRole {
    Action,
    Toggle,
    Menu,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChromeMenuItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub children: Vec<Self>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChromeAction {
    pub id: String,
    pub role: ChromeActionRole,
    pub icon: String,
    pub label: String,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub menu: Vec<ChromeMenuItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChromeGroup {
    pub id: String,
    pub actions: Vec<ChromeAction>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerDragRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ContainerDragRegion {
    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContainerChrome {
    pub visibility: ChromeVisibility,
    pub title: Option<String>,
    pub leading: Vec<ChromeGroup>,
    pub trailing: Vec<ChromeGroup>,
    pub drag_regions: Vec<ContainerDragRegion>,
}

impl ContainerChrome {
    pub fn is_valid(&self, limits: ContainerLimits) -> bool {
        self.title.as_ref().is_none_or(|title| {
            !title.contains(['\r', '\n']) && title.len() <= usize::from(limits.max_title_bytes)
        }) && self.leading.len() <= usize::from(limits.max_chrome_groups_per_side)
            && self.trailing.len() <= usize::from(limits.max_chrome_groups_per_side)
            && self.drag_regions.len() <= usize::from(limits.max_drag_regions)
            && self.drag_regions.iter().all(|region| region.is_valid())
            && self
                .leading
                .iter()
                .chain(&self.trailing)
                .all(|group| valid_group(group, limits))
    }
}

fn valid_group(group: &ChromeGroup, limits: ContainerLimits) -> bool {
    valid_id(&group.id)
        && group.actions.len() <= usize::from(limits.max_actions_per_group)
        && group.actions.iter().all(|action| {
            valid_id(&action.id)
                && valid_label(&action.label)
                && valid_icon(&action.icon)
                && action
                    .tooltip
                    .as_ref()
                    .is_none_or(|tooltip| valid_label(tooltip))
                && action.menu.iter().all(|item| valid_menu(item, 1, limits))
        })
}

fn valid_menu(item: &ChromeMenuItem, depth: u8, limits: ContainerLimits) -> bool {
    depth <= limits.max_menu_depth
        && valid_id(&item.id)
        && valid_label(&item.label)
        && item.icon.as_ref().is_none_or(|icon| valid_icon(icon))
        && item
            .children
            .iter()
            .all(|child| valid_menu(child, depth.saturating_add(1), limits))
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn valid_label(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.contains(['\r', '\n'])
}

fn valid_icon(value: &str) -> bool {
    valid_id(value)
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerPage {
    pub id: PageId,
    pub revision: Revision,
    pub surface_id: SurfaceId,
    pub chrome: ContainerChrome,
    pub preferred_content_size: SurfaceContentSize,
}

impl ContainerPage {
    pub fn is_valid(&self, limits: ContainerLimits) -> bool {
        self.preferred_content_size.is_valid()
            && self.chrome.is_valid(limits)
            && self.chrome.drag_regions.iter().all(|region| {
                region.x + region.width <= self.preferred_content_size.width
                    && region.y + region.height <= self.preferred_content_size.height
            })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerTransitionStyle {
    None,
    Fade,
    Slide,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerTransition {
    pub style: ContainerTransitionStyle,
    pub duration_ms: u16,
    pub easing: SurfaceResizeEasing,
}

impl ContainerTransition {
    pub fn is_valid(self, limits: ContainerLimits) -> bool {
        self.duration_ms <= limits.max_transition_ms
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
pub enum ContainerOperation {
    Update { page: ContainerPage },
    Push { page: ContainerPage },
    Pop,
    Replace { page: ContainerPage },
    Reset { root: ContainerPage },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OpenContainerRequest {
    pub request_id: RequestId,
    pub container_id: ContainerId,
    pub context_id: ContextId,
    pub context_revision: Revision,
    pub slot: SurfaceSlot,
    pub root: ContainerPage,
    #[serde(default)]
    pub transition: Option<ContainerTransition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerTransaction {
    pub request_id: RequestId,
    pub container_id: ContainerId,
    pub expected_revision: Revision,
    pub operation: ContainerOperation,
    #[serde(default)]
    pub transition: Option<ContainerTransition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CloseContainerRequest {
    pub request_id: RequestId,
    pub container_id: ContainerId,
    pub expected_revision: Revision,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerSnapshot {
    pub plugin_id: PluginId,
    pub worker_id: WorkerId,
    pub container_id: ContainerId,
    pub revision: Revision,
    pub context_id: ContextId,
    pub context_revision: Revision,
    pub slot: SurfaceSlot,
    pub origin: SurfacePoint,
    pub viewport: [f32; 4],
    pub pages: Vec<ContainerPage>,
    pub active_page_id: PageId,
    #[serde(default)]
    pub transition: Option<ContainerTransition>,
}

impl ContainerSnapshot {
    pub fn active_page(&self) -> Option<&ContainerPage> {
        self.pages
            .iter()
            .find(|page| page.id == self.active_page_id)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerRejection {
    Compatibility(String),
    StaleRevision,
    InvalidRequest(String),
    ResourceLimit(String),
    UnknownContainer,
    WrongWorker,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerApplied {
    pub revision: Revision,
    pub content_size: SurfaceContentSize,
    pub clamped: bool,
    pub transition: Option<ContainerTransition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerAcknowledgement {
    pub request_id: RequestId,
    pub container_id: ContainerId,
    pub result: Result<ContainerApplied, ContainerRejection>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChromeActionSource {
    Pointer,
    Keyboard,
    Menu,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageLifecyclePhase {
    WillAppear,
    DidAppear,
    WillDisappear,
    DidDisappear,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ContainerEvent {
    ChromeAction {
        container_id: ContainerId,
        page_id: PageId,
        revision: Revision,
        action_id: String,
        #[serde(default)]
        menu_item_id: Option<String>,
        source: ChromeActionSource,
    },
    BackRequested {
        container_id: ContainerId,
        page_id: PageId,
        revision: Revision,
    },
    PageLifecycle {
        container_id: ContainerId,
        page_id: PageId,
        revision: Revision,
        phase: PageLifecyclePhase,
    },
    GeometryChanged {
        container_id: ContainerId,
        revision: Revision,
        origin: SurfacePoint,
        content_size: SurfaceContentSize,
    },
    Dismissed {
        container_id: ContainerId,
        revision: Revision,
    },
    Cancelled {
        container_id: ContainerId,
        revision: Revision,
        reason: String,
    },
}
