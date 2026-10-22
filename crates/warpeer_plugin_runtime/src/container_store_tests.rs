use warpeer_plugin_protocol::*;

use crate::*;

fn chrome(title: &str) -> ContainerChrome {
    ContainerChrome {
        title: Some(title.into()),
        trailing: vec![ChromeGroup {
            id: "tools".into(),
            actions: vec![ChromeAction {
                id: "details".into(),
                role: ChromeActionRole::Action,
                icon: "info".into(),
                label: "Details".into(),
                tooltip: Some("Show image details".into()),
                enabled: true,
                selected: false,
                menu: Vec::new(),
            }],
        }],
        ..ContainerChrome::default()
    }
}

fn page(title: &str) -> ContainerPage {
    ContainerPage {
        id: PageId::new(),
        revision: Revision(1),
        surface_id: SurfaceId::new(),
        chrome: chrome(title),
        preferred_content_size: SurfaceContentSize {
            width: 320.0,
            height: 240.0,
        },
    }
}

fn open_request(root: ContainerPage) -> OpenContainerRequest {
    OpenContainerRequest {
        request_id: RequestId::new(),
        container_id: ContainerId::new(),
        context_id: ContextId::new(),
        context_revision: Revision(7),
        slot: SurfaceSlot::TerminalHover,
        root,
        transition: None,
    }
}

#[test]
fn negotiation_accepts_supported_extensions_and_rejects_navigation() {
    assert!(negotiate_extensions(&[]).unwrap().is_empty());
    let accepted = negotiate_extensions(&[
        ExtensionRequirement {
            extension: ContainerExtension::Chrome,
            min_version: 1,
            max_version: 1,
            optional: false,
        },
        ExtensionRequirement {
            extension: ContainerExtension::Navigation,
            min_version: 1,
            max_version: 1,
            optional: true,
        },
    ])
    .unwrap();
    assert_eq!(
        accepted,
        vec![ExtensionSupport {
            extension: ContainerExtension::Chrome,
            version: 1,
        }]
    );
    assert!(
        negotiate_extensions(&[ExtensionRequirement {
            extension: ContainerExtension::Navigation,
            min_version: 2,
            max_version: 3,
            optional: false,
        }])
        .is_err()
    );
}

#[test]
fn store_updates_one_root_page_and_rejects_navigation() {
    let store = ContainerStore::default();
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Preview");
    let root_surface = root.surface_id;
    let open = open_request(root);
    let container_id = open.container_id;
    let mut authorized = vec![root_surface];
    let opened = store.open(
        plugin_id.clone(),
        worker_id,
        open,
        [0.0, 0.0, 800.0, 600.0],
        SurfacePoint { x: 20.0, y: 30.0 },
        |surface| authorized.contains(&surface),
    );
    assert!(opened.acknowledgement.result.is_ok());
    assert_eq!(store.snapshot(container_id).unwrap().pages.len(), 1);

    let mut updated_page = store
        .snapshot(container_id)
        .unwrap()
        .active_page()
        .unwrap()
        .clone();
    updated_page.revision = Revision(2);
    updated_page.chrome = chrome("Updated");
    let update_request = RequestId::new();
    let updated = store.transact(
        &plugin_id,
        worker_id,
        ContainerTransaction {
            request_id: update_request,
            container_id,
            expected_revision: Revision(1),
            operation: ContainerOperation::Update {
                page: updated_page.clone(),
            },
            transition: None,
        },
        |surface| authorized.contains(&surface),
    );
    assert!(updated.acknowledgement.result.is_ok());
    assert_eq!(updated.snapshot.as_ref().unwrap().pages.len(), 1);
    assert_eq!(
        updated
            .snapshot
            .as_ref()
            .unwrap()
            .active_page()
            .unwrap()
            .chrome
            .title
            .as_deref(),
        Some("Updated")
    );

    let replay = store.transact(
        &plugin_id,
        worker_id,
        ContainerTransaction {
            request_id: update_request,
            container_id,
            expected_revision: Revision(1),
            operation: ContainerOperation::Update { page: updated_page },
            transition: None,
        },
        |surface| authorized.contains(&surface),
    );
    assert!(replay.replayed);
    assert_eq!(replay.snapshot.unwrap().pages.len(), 1);

    let rejected = store.transact(
        &plugin_id,
        worker_id,
        ContainerTransaction {
            request_id: RequestId::new(),
            container_id,
            expected_revision: Revision(2),
            operation: ContainerOperation::Pop,
            transition: None,
        },
        |surface| authorized.contains(&surface),
    );
    assert!(matches!(
        rejected.acknowledgement.result,
        Err(ContainerRejection::Compatibility(ref reason))
            if reason == "page stack navigation is unavailable"
    ));
    assert_eq!(store.snapshot(container_id).unwrap().pages.len(), 1);
}

#[test]
fn stale_or_invalid_transactions_preserve_the_previous_snapshot() {
    let store = ContainerStore::default();
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Preview");
    let root_surface = root.surface_id;
    let open = open_request(root);
    let container_id = open.container_id;
    store.open(
        plugin_id.clone(),
        worker_id,
        open,
        [0.0, 0.0, 640.0, 480.0],
        SurfacePoint { x: 8.0, y: 8.0 },
        |surface| surface == root_surface,
    );
    let invalid = store.transact(
        &plugin_id,
        worker_id,
        ContainerTransaction {
            request_id: RequestId::new(),
            container_id,
            expected_revision: Revision(99),
            operation: ContainerOperation::Pop,
            transition: None,
        },
        |_| true,
    );
    assert!(matches!(
        invalid.acknowledgement.result,
        Err(ContainerRejection::StaleRevision)
    ));
    let snapshot = store.snapshot(container_id).unwrap();
    assert_eq!(snapshot.revision, Revision(1));
    assert_eq!(snapshot.pages.len(), 1);
}

#[test]
fn origin_and_resources_survive_page_revisions_until_teardown() {
    let store = ContainerStore::default();
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Preview");
    let root_surface = root.surface_id;
    let open = open_request(root);
    let container_id = open.container_id;
    store.open(
        plugin_id.clone(),
        worker_id,
        open,
        [0.0, 0.0, 640.0, 480.0],
        SurfacePoint { x: 8.0, y: 8.0 },
        |surface| surface == root_surface,
    );
    store.set_origin(container_id, SurfacePoint { x: 150.0, y: 110.0 });
    assert_eq!(store.snapshot(container_id).unwrap().origin.x, 150.0);
    assert_eq!(store.ledger(container_id).unwrap().pages.len(), 1);
    assert_eq!(store.remove_plugin(&plugin_id), vec![root_surface]);
    assert_eq!(store.len(), 0);
}

#[test]
fn removing_a_legacy_surface_releases_its_implicit_container() {
    let store = ContainerStore::default();
    let plugin_id = PluginId::parse("dev.warpeer.legacy").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Legacy Preview");
    let surface_id = root.surface_id;
    let request = open_request(root);
    store.open(
        plugin_id,
        worker_id,
        request,
        [0.0, 0.0, 640.0, 480.0],
        SurfacePoint { x: 8.0, y: 8.0 },
        |candidate| candidate == surface_id,
    );

    assert_eq!(store.remove_surface(surface_id), vec![surface_id]);
    assert_eq!(store.len(), 0);
}

#[test]
fn a_legacy_surface_can_find_and_update_its_implicit_root_page() {
    let store = ContainerStore::default();
    let plugin_id = PluginId::parse("dev.warpeer.legacy").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Legacy Preview");
    let surface_id = root.surface_id;
    let request = open_request(root);
    let container_id = request.container_id;
    store.open(
        plugin_id.clone(),
        worker_id,
        request,
        [0.0, 0.0, 640.0, 480.0],
        SurfacePoint { x: 8.0, y: 8.0 },
        |candidate| candidate == surface_id,
    );
    let snapshot = store.for_surface(surface_id).unwrap();
    let mut root = snapshot.active_page().unwrap().clone();
    root.revision = root.revision.next();
    root.preferred_content_size = SurfaceContentSize {
        width: 420.0,
        height: 280.0,
    };
    let commit = store.transact(
        &plugin_id,
        worker_id,
        ContainerTransaction {
            request_id: RequestId::new(),
            container_id,
            expected_revision: snapshot.revision,
            operation: ContainerOperation::Update { page: root },
            transition: None,
        },
        |candidate| candidate == surface_id,
    );

    assert_eq!(
        commit
            .snapshot
            .unwrap()
            .active_page()
            .unwrap()
            .preferred_content_size
            .width,
        420.0
    );
}

#[test]
fn single_page_update_does_not_retain_a_navigation_snapshot() {
    let store = ContainerStore::default();
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Preview");
    let root_surface = root.surface_id;
    let request = open_request(root.clone());
    let container_id = request.container_id;
    store.open(
        plugin_id.clone(),
        worker_id,
        request,
        [0.0, 0.0, 640.0, 480.0],
        SurfacePoint { x: 8.0, y: 8.0 },
        |candidate| candidate == root_surface,
    );
    let mut updated = root;
    updated.revision = Revision(2);
    updated.chrome = chrome("Updated");
    let commit = store.transact(
        &plugin_id,
        worker_id,
        ContainerTransaction {
            request_id: RequestId::new(),
            container_id,
            expected_revision: Revision(1),
            operation: ContainerOperation::Update { page: updated },
            transition: Some(ContainerTransition {
                style: ContainerTransitionStyle::Slide,
                duration_ms: 220,
                easing: SurfaceResizeEasing::EaseOut,
            }),
        },
        |candidate| candidate == root_surface,
    );
    let revision = commit.snapshot.unwrap().revision;
    assert!(
        store
            .ledger(container_id)
            .unwrap()
            .transition_page
            .is_none()
    );
    assert!(store.complete_transition(container_id, revision));
    assert!(
        store
            .ledger(container_id)
            .unwrap()
            .transition_page
            .is_none()
    );
}

#[test]
fn capability_handles_follow_the_owning_container_ledger() {
    let store = ContainerStore::default();
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Preview");
    let surface_id = root.surface_id;
    let request = open_request(root);
    let context_id = request.context_id;
    let container_id = request.container_id;
    store.open(
        plugin_id.clone(),
        worker_id,
        request,
        [0.0, 0.0, 640.0, 480.0],
        SurfacePoint { x: 8.0, y: 8.0 },
        |candidate| candidate == surface_id,
    );
    let handle = ResourceHandleId::new();

    assert!(store.attach_capability_handle(context_id, handle));
    assert!(
        store
            .ledger(container_id)
            .unwrap()
            .capability_handles
            .contains(&handle)
    );
    assert!(store.release_capability_handle(&plugin_id, handle));
    assert!(
        store
            .ledger(container_id)
            .unwrap()
            .capability_handles
            .is_empty()
    );
}

#[test]
fn store_enforces_retained_pixel_and_message_rate_budgets() {
    let pixel_limited = ContainerStore::new(ContainerLimits {
        max_retained_pixels: 1,
        ..ContainerLimits::default()
    });
    let plugin_id = PluginId::parse("dev.warpeer.budgeted").unwrap();
    let worker_id = WorkerId::new();
    let root = page("Too Large");
    let surface_id = root.surface_id;
    let rejected = pixel_limited.open(
        plugin_id.clone(),
        worker_id,
        open_request(root),
        [0.0, 0.0, 640.0, 480.0],
        SurfacePoint { x: 8.0, y: 8.0 },
        |candidate| candidate == surface_id,
    );
    assert!(matches!(
        rejected.acknowledgement.result,
        Err(ContainerRejection::ResourceLimit(ref limit))
            if limit == "retained container pixels"
    ));

    let rate_limited = ContainerStore::new(ContainerLimits {
        max_transactions_per_second: 1,
        ..ContainerLimits::default()
    });
    let root = page("Preview");
    let surface_id = root.surface_id;
    let request = open_request(root);
    let container_id = request.container_id;
    assert!(
        rate_limited
            .open(
                plugin_id.clone(),
                worker_id,
                request,
                [0.0, 0.0, 640.0, 480.0],
                SurfacePoint { x: 8.0, y: 8.0 },
                |candidate| candidate == surface_id,
            )
            .acknowledgement
            .result
            .is_ok()
    );
    let rejected = rate_limited.transact(
        &plugin_id,
        worker_id,
        ContainerTransaction {
            request_id: RequestId::new(),
            container_id,
            expected_revision: Revision(1),
            operation: ContainerOperation::Pop,
            transition: None,
        },
        |_| true,
    );
    assert!(matches!(
        rejected.acknowledgement.result,
        Err(ContainerRejection::ResourceLimit(ref limit))
            if limit == "container transaction rate"
    ));
}
