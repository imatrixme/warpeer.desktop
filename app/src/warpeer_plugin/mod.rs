mod bridge_dispatch;
mod container_host;
mod container_transition_visual;
mod diagnostics;
mod host;
#[cfg(feature = "integration_tests")]
mod host_integration;
mod legacy_container_adapter;
mod lifecycle;
mod pane_surfaces;
mod preview_bridge;
mod process;
mod surface_drag;
mod surface_element;
mod surface_host;
mod surface_resize;
mod surface_store;

pub(crate) use container_host::PresentedContainer;
pub(crate) use diagnostics::PluginDiagnosticRecord;
pub(crate) use host::{PluginHostClient, WarpeerPluginHost};
pub(crate) use lifecycle::PluginRestartMode;
pub(crate) use pane_surfaces::PanePluginSurfaceState;
pub(crate) use surface_element::{
    PLUGIN_WINDOW_TITLEBAR_HEIGHT, PluginSurfacePresentation, SurfacePresentationOptions,
    container_bounds, plugin_window_container_metrics,
};
pub(crate) use surface_store::SurfaceSnapshot;
