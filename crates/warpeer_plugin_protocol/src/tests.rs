use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use semver::{Version, VersionReq};
use serde_json::json;

use crate::*;

#[test]
fn plugin_id_requires_reverse_dns_shape() {
    assert!(PluginId::parse("dev.warpeer.image-preview").is_ok());
    assert!(PluginId::parse("image-preview").is_err());
    assert!(PluginId::parse("dev..preview").is_err());
    assert!(PluginId::parse("dev.warpeer._preview").is_err());
}

#[test]
fn container_and_page_ids_parse_and_round_trip() {
    let container = ContainerId::new();
    let page = PageId::new();
    assert_eq!(
        ContainerId::from_str(&container.to_string()).unwrap(),
        container
    );
    assert_eq!(PageId::from_str(&page.to_string()).unwrap(), page);
    assert_eq!(
        serde_json::from_value::<ContainerId>(serde_json::to_value(container).unwrap()).unwrap(),
        container
    );
}

#[test]
fn chrome_and_container_messages_validate_and_round_trip() {
    let chrome = ContainerChrome {
        visibility: ChromeVisibility::Hidden,
        title: Some("Image Preview".into()),
        leading: Vec::new(),
        trailing: vec![ChromeGroup {
            id: "tools".into(),
            actions: vec![ChromeAction {
                id: "details".into(),
                role: ChromeActionRole::Action,
                icon: "info".into(),
                label: "Details".into(),
                tooltip: None,
                enabled: true,
                selected: false,
                menu: Vec::new(),
            }],
        }],
        drag_regions: vec![ContainerDragRegion {
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 20.0,
        }],
    };
    assert!(chrome.is_valid(ContainerLimits::default()));
    let page = ContainerPage {
        id: PageId::new(),
        revision: Revision(1),
        surface_id: SurfaceId::new(),
        chrome,
        preferred_content_size: SurfaceContentSize {
            width: 320.0,
            height: 180.0,
        },
    };
    assert!(page.is_valid(ContainerLimits::default()));
    let mut out_of_bounds = page.clone();
    out_of_bounds.chrome.drag_regions[0].x = 250.0;
    assert!(!out_of_bounds.is_valid(ContainerLimits::default()));
    let message = PluginToHost::OpenContainer(OpenContainerRequest {
        request_id: RequestId::new(),
        container_id: ContainerId::new(),
        context_id: ContextId::new(),
        context_revision: Revision(1),
        slot: SurfaceSlot::TerminalHover,
        root: page,
        transition: Some(ContainerTransition {
            style: ContainerTransitionStyle::Fade,
            duration_ms: 220,
            easing: SurfaceResizeEasing::EaseOut,
        }),
    });
    let encoded = serde_json::to_value(&message).unwrap();
    assert_eq!(
        serde_json::from_value::<PluginToHost>(encoded).unwrap(),
        message
    );
}

#[test]
fn runtime_requirement_checks_protocol_and_abi() {
    let requirement = RuntimeRequirement {
        protocol: 1,
        abi: VersionReq::parse(">=1.0.0, <2.0.0").unwrap(),
        engine: PluginEngine::PocketSurface,
        target: Some("warpeer-macos".into()),
    };
    assert!(requirement.accepts(1, RuntimeAbi { major: 1, minor: 4 }));
    assert!(!requirement.accepts(2, RuntimeAbi { major: 1, minor: 4 }));
    assert!(!requirement.accepts(1, RuntimeAbi { major: 2, minor: 0 }));
}

#[test]
fn stale_frames_are_not_newer() {
    let id = SurfaceId::new();
    let frame = |surface_revision, environment_revision, frame_revision| SurfaceFrame {
        surface_id: id,
        surface_revision: Revision(surface_revision),
        environment_revision: Revision(environment_revision),
        frame_revision: Revision(frame_revision),
        width_px: 10,
        height_px: 10,
        dirty_regions: Vec::new(),
        transport: FrameTransport::SharedBuffer {
            bytes: vec![0; 400],
            stride_bytes: 40,
        },
    };
    assert!(frame(1, 1, 2).is_newer_than(&frame(1, 1, 1)));
    assert!(!frame(2, 1, 1).is_newer_than(&frame(1, 1, 9)));
    assert!(!frame(1, 2, 2).is_newer_than(&frame(1, 1, 1)));
}

#[test]
fn surface_descriptors_reject_invalid_geometry_and_missing_labels() {
    let mut descriptor = SurfaceDescriptor {
        id: SurfaceId::new(),
        slot: SurfaceSlot::TerminalHover,
        target: SurfaceTarget::TerminalContext {
            context_id: ContextId::new(),
            pane_id: "pane-1".into(),
        },
        revision: Revision(1),
        bounds: SurfaceBounds {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 10.0,
            scale: 1.0,
        },
        accessibility_label: "Preview".into(),
        accepts_pointer: false,
        accepts_keyboard: false,
    };
    assert!(descriptor.is_valid());
    descriptor.bounds.width = f32::NAN;
    assert!(!descriptor.is_valid());
    descriptor.bounds.width = 20.0;
    descriptor.accessibility_label.clear();
    assert!(!descriptor.is_valid());
    descriptor.accessibility_label = "Preview".into();
    descriptor.target = SurfaceTarget::PluginSettings {
        context_id: ContextId::new(),
    };
    assert!(!descriptor.is_valid());
}

#[test]
fn protocol_limits_reject_large_payloads() {
    let envelope = Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        abi: CURRENT_RUNTIME_ABI,
        message: PluginToHost::Diagnostic {
            level: DiagnosticLevel::Info,
            message: "x".repeat(512),
        },
    };
    let limits = ProtocolLimits {
        max_payload_bytes: 64,
        max_queued_messages: 1,
    };
    assert!(matches!(
        limits.validate(&envelope),
        Err(ProtocolError::PayloadTooLarge { .. })
    ));
}

#[test]
fn legacy_hover_context_defaults_to_transient_preview() {
    let context: TerminalHoverContext = serde_json::from_value(json!({
        "context_id": ContextId::new(),
        "revision": Revision(1),
        "terminal_id": "terminal",
        "pane_id": "pane",
        "workspace_id": null,
        "working_directory": "/tmp",
        "resource_text": "image.png",
        "resolved_resource": "/tmp/image.png",
        "explicit_hyperlink": false,
        "anchor": [0.0, 0.0, 1.0, 1.0],
        "viewport": [0.0, 0.0, 800.0, 600.0]
    }))
    .unwrap();

    assert!(!context.explicit_preview);
}

#[test]
fn legacy_worker_start_defaults_to_no_container_extensions() {
    let plugin_id = PluginId::parse("dev.warpeer.legacy").unwrap();
    let worker_id = WorkerId::new();
    let message: HostToPlugin = serde_json::from_value(json!({
        "type": "start",
        "payload": {
            "plugin_id": plugin_id,
            "worker_id": worker_id
        }
    }))
    .unwrap();

    assert_eq!(
        message,
        HostToPlugin::Start {
            plugin_id,
            worker_id,
            extensions: Vec::new(),
            container_limits: ContainerLimits::default(),
        }
    );
}

#[test]
fn host_surface_release_round_trips_without_exposing_native_resources() {
    let message = HostToPlugin::ReleaseSurface {
        surface_id: SurfaceId::new(),
        revision: Revision(4),
    };
    let encoded = serde_json::to_value(&message).unwrap();
    assert_eq!(encoded["type"], "release-surface");
    assert_eq!(
        serde_json::from_value::<HostToPlugin>(encoded).unwrap(),
        message
    );

    let acknowledgement = PluginToHost::SurfaceReleased {
        surface_id: SurfaceId::new(),
        revision: Revision(4),
    };
    let encoded = serde_json::to_value(&acknowledgement).unwrap();
    assert_eq!(encoded["type"], "surface-released");
    assert_eq!(
        serde_json::from_value::<PluginToHost>(encoded).unwrap(),
        acknowledgement
    );
}

#[test]
fn surface_environment_round_trips_semantic_palette() {
    let environment = SurfaceEnvironment {
        surface_id: SurfaceId::new(),
        revision: Revision(3),
        bounds: SurfaceBounds {
            x: 4.,
            y: 8.,
            width: 320.,
            height: 180.,
            scale: 2.,
        },
        container: SurfaceContainerMetrics::default(),
        appearance: AppearanceKind::Dark,
        palette: SurfacePalette::dark(),
        reduced_motion: false,
        focused: false,
        visible: true,
        occluded: false,
    };
    let encoded = serde_json::to_value(&environment).unwrap();
    assert_eq!(encoded["palette"]["background"], json!([15, 17, 21, 255]));
    assert_eq!(
        serde_json::from_value::<SurfaceEnvironment>(encoded).unwrap(),
        environment
    );
}

#[test]
fn surface_resize_round_trips_content_size_and_animation() {
    let request = SurfaceResizeRequest {
        surface_id: SurfaceId::new(),
        revision: Revision(4),
        content_size: SurfaceContentSize {
            width: 420.0,
            height: 280.0,
        },
        animation: Some(SurfaceResizeAnimation {
            duration_ms: 220,
            easing: SurfaceResizeEasing::EaseOut,
        }),
    };
    assert!(request.is_valid());
    let message = PluginToHost::ResizeSurface(request);
    assert_eq!(
        serde_json::from_value::<PluginToHost>(serde_json::to_value(&message).unwrap()).unwrap(),
        message
    );
}

#[test]
fn manifest_round_trips_without_losing_contract_data() {
    let manifest = PluginManifest {
        id: PluginId::parse("dev.warpeer.image-preview").unwrap(),
        name: "Image Preview".into(),
        description: "Preview terminal image resources.".into(),
        version: Version::new(1, 0, 0),
        runtime: RuntimeRequirement {
            protocol: 1,
            abi: VersionReq::parse("^1").unwrap(),
            engine: PluginEngine::PocketSurface,
            target: None,
        },
        entrypoints: Entrypoints {
            bundle: "dist/main.pocketjs".into(),
            assets: "dist/assets.pak".into(),
            icon: None,
        },
        hashes: BTreeMap::new(),
        publisher: None,
        signature: None,
        surfaces: BTreeSet::from([SurfaceSlot::TerminalHover]),
        triggers: Vec::new(),
        extensions: Vec::new(),
        capabilities: Vec::new(),
        settings: Vec::new(),
        budgets: ResourceBudgets::default(),
    };
    let encoded = serde_json::to_value(&manifest).unwrap();
    assert_eq!(
        serde_json::from_value::<PluginManifest>(encoded).unwrap(),
        manifest
    );
    assert_eq!(json!(manifest.runtime.protocol), json!(1));
}
