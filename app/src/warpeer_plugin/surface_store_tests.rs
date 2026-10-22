use warpeer_plugin_protocol::{FrameTransport, SurfacePoint};

use super::*;

fn descriptor(context_id: ContextId, revision: Revision) -> SurfaceDescriptor {
    SurfaceDescriptor {
        id: SurfaceId::new(),
        slot: SurfaceSlot::TerminalHover,
        target: SurfaceTarget::TerminalContext {
            context_id,
            pane_id: "pane-1".into(),
        },
        revision,
        bounds: SurfaceBounds {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
            scale: 1.0,
        },
        accessibility_label: "Preview".into(),
        accepts_pointer: false,
        accepts_keyboard: false,
    }
}

fn frame(id: SurfaceId, surface: u64, environment: u64, frame: u64) -> SurfaceFrame {
    SurfaceFrame {
        surface_id: id,
        surface_revision: Revision(surface),
        environment_revision: Revision(environment),
        frame_revision: Revision(frame),
        width_px: 20,
        height_px: 20,
        dirty_regions: vec![[0, 0, 20, 20]],
        transport: FrameTransport::SharedBuffer {
            bytes: vec![0; 20 * 20 * 4],
            stride_bytes: 80,
        },
    }
}

#[test]
fn rejects_frames_until_environment_and_after_resize() {
    let store = SurfaceStore::default();
    let owner = PluginId::parse("dev.warpeer.preview").unwrap();
    let context_id = ContextId::new();
    let descriptor = descriptor(context_id, Revision(2));
    let id = descriptor.id;
    store.open(owner.clone(), descriptor);
    assert!(!store.apply_frame(&owner, frame(id, 2, 1, 1)));

    let now = Instant::now();
    let delivery = store
        .present(
            id,
            SurfaceBounds {
                x: 0.,
                y: 0.,
                width: 20.,
                height: 20.,
                scale: 2.,
            },
            SurfaceContainerMetrics::default(),
            AppearanceKind::Dark,
            SurfacePalette::dark(),
            false,
            true,
            true,
            false,
            now,
        )
        .unwrap();
    assert_eq!(delivery.environment.revision, Revision(1));
    assert!(store.apply_frame(&owner, frame(id, 2, 1, 1)));

    let resized = store
        .present(
            id,
            SurfaceBounds {
                width: 30.,
                ..delivery.environment.bounds
            },
            SurfaceContainerMetrics::default(),
            AppearanceKind::Dark,
            SurfacePalette::dark(),
            false,
            true,
            true,
            false,
            now,
        )
        .unwrap();
    assert_eq!(resized.environment.revision, Revision(2));
    assert!(!store.apply_frame(&owner, frame(id, 2, 1, 2)));
    assert!(
        store
            .for_target(
                SurfaceSlot::TerminalHover,
                &SurfaceTarget::TerminalContext {
                    context_id,
                    pane_id: "pane-1".into(),
                },
                Revision(2),
            )
            .unwrap()
            .frame
            .is_none()
    );
}

#[test]
fn occlusion_advances_environment_only_after_lease_expires() {
    let store = SurfaceStore::default();
    let owner = PluginId::parse("dev.warpeer.preview").unwrap();
    let context_id = ContextId::new();
    let descriptor = descriptor(context_id, Revision(1));
    let id = descriptor.id;
    let bounds = descriptor.bounds;
    store.open(owner.clone(), descriptor);
    let now = Instant::now();
    store.present(
        id,
        bounds,
        SurfaceContainerMetrics::default(),
        AppearanceKind::Dark,
        SurfacePalette::dark(),
        false,
        false,
        true,
        false,
        now,
    );
    assert!(store.apply_frame(&owner, frame(id, 1, 1, 1)));
    assert!(
        store
            .occlude_unpresented(now + Duration::from_millis(99), Duration::from_millis(100))
            .is_empty()
    );
    let hidden =
        store.occlude_unpresented(now + Duration::from_millis(100), Duration::from_millis(100));
    assert_eq!(hidden.len(), 1);
    assert!(!hidden[0].environment.visible);
    assert!(hidden[0].environment.occluded);
    assert_eq!(hidden[0].environment.revision, Revision(2));
    assert!(
        store
            .for_target(
                SurfaceSlot::TerminalHover,
                &SurfaceTarget::TerminalContext {
                    context_id,
                    pane_id: "pane-1".into(),
                },
                Revision(1),
            )
            .unwrap()
            .frame
            .is_some()
    );
}

#[test]
fn target_and_bounds_are_exact() {
    let store = SurfaceStore::default();
    let owner = PluginId::parse("dev.warpeer.preview").unwrap();
    let context_id = ContextId::new();
    let descriptor = descriptor(context_id, Revision(1));
    assert!(descriptor.bounds.contains(SurfacePoint { x: 2., y: 3. }));
    store.open(owner.clone(), descriptor);
    assert_eq!(store.remove_context(context_id), vec![owner]);
    assert!(store.for_pane("pane-1").is_empty());
}

#[test]
fn palette_changes_advance_environment_without_discarding_same_size_frame() {
    let store = SurfaceStore::default();
    let owner = PluginId::parse("dev.warpeer.preview").unwrap();
    let descriptor = descriptor(ContextId::new(), Revision(1));
    let id = descriptor.id;
    let bounds = descriptor.bounds;
    store.open(owner.clone(), descriptor);
    let now = Instant::now();
    store.present(
        id,
        bounds,
        SurfaceContainerMetrics::default(),
        AppearanceKind::Dark,
        SurfacePalette::dark(),
        false,
        false,
        true,
        false,
        now,
    );
    assert!(store.apply_frame(&owner, frame(id, 1, 1, 1)));

    let mut palette = SurfacePalette::dark();
    palette.background = [240, 241, 243, 255];
    let delivery = store
        .present(
            id,
            bounds,
            SurfaceContainerMetrics::default(),
            AppearanceKind::Light,
            palette,
            false,
            false,
            true,
            false,
            now,
        )
        .unwrap();

    assert_eq!(delivery.environment.revision, Revision(2));
    assert_eq!(delivery.environment.palette, palette);
    assert!(store.owner(id).unwrap().2.unwrap().visible);
    assert!(store.records.read().get(&id).unwrap().frame.is_some());
}
