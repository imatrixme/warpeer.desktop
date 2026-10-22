use warpeer_plugin_protocol::{
    ContainerAcknowledgement, ContainerApplied, ContainerEvent, ContainerId, ContainerLimits,
    ContainerOperation, ContainerPage, ContainerRejection, ContainerSnapshot, ContainerTransition,
    PageLifecyclePhase, RequestId, Revision, SurfaceId,
};

use crate::ContainerCommit;

pub(super) fn apply_operation(
    snapshot: &mut ContainerSnapshot,
    operation: &ContainerOperation,
    limits: ContainerLimits,
    surface_authorized: &impl Fn(SurfaceId) -> bool,
    released: &mut Vec<SurfaceId>,
) -> Result<(), ContainerRejection> {
    let page = match operation {
        ContainerOperation::Update { page } => page,
        ContainerOperation::Push { .. }
        | ContainerOperation::Pop
        | ContainerOperation::Replace { .. }
        | ContainerOperation::Reset { .. } => {
            return Err(ContainerRejection::Compatibility(
                "page stack navigation is unavailable".into(),
            ));
        }
    };
    if !page.is_valid(limits) || !surface_authorized(page.surface_id) {
        return Err(ContainerRejection::InvalidRequest(
            "invalid or unauthorized page".into(),
        ));
    }
    update(snapshot, page, released)?;
    Ok(())
}

fn update(
    snapshot: &mut ContainerSnapshot,
    page: &ContainerPage,
    released: &mut Vec<SurfaceId>,
) -> Result<(), ContainerRejection> {
    if page.id != snapshot.active_page_id {
        return Err(ContainerRejection::InvalidRequest(
            "updated page is not active".into(),
        ));
    }
    let active = snapshot.pages.last_mut().expect("container has a root");
    if page.revision <= active.revision {
        return Err(ContainerRejection::StaleRevision);
    }
    if active.surface_id != page.surface_id {
        released.push(active.surface_id);
    }
    *active = page.clone();
    Ok(())
}

pub(super) fn navigation_events(
    previous: &ContainerSnapshot,
    next: &ContainerSnapshot,
) -> (Vec<ContainerEvent>, Vec<ContainerEvent>) {
    if previous.active_page_id == next.active_page_id {
        return (Vec::new(), Vec::new());
    }
    (
        vec![
            lifecycle_page(
                next,
                previous.active_page_id,
                PageLifecyclePhase::WillDisappear,
            ),
            lifecycle(next, PageLifecyclePhase::WillAppear),
        ],
        vec![
            lifecycle_page(
                next,
                previous.active_page_id,
                PageLifecyclePhase::DidDisappear,
            ),
            lifecycle(next, PageLifecyclePhase::DidAppear),
        ],
    )
}

pub(super) fn lifecycle(snapshot: &ContainerSnapshot, phase: PageLifecyclePhase) -> ContainerEvent {
    lifecycle_page(snapshot, snapshot.active_page_id, phase)
}

fn lifecycle_page(
    snapshot: &ContainerSnapshot,
    page_id: warpeer_plugin_protocol::PageId,
    phase: PageLifecyclePhase,
) -> ContainerEvent {
    ContainerEvent::PageLifecycle {
        container_id: snapshot.container_id,
        page_id,
        revision: snapshot.revision,
        phase,
    }
}

pub(super) fn applied(
    request_id: RequestId,
    container_id: ContainerId,
    revision: Revision,
    page: &ContainerPage,
    transition: Option<ContainerTransition>,
) -> ContainerAcknowledgement {
    ContainerAcknowledgement {
        request_id,
        container_id,
        result: Ok(ContainerApplied {
            revision,
            content_size: page.preferred_content_size,
            clamped: false,
            transition,
        }),
    }
}

pub(super) fn rejected(
    request_id: RequestId,
    container_id: ContainerId,
    reason: ContainerRejection,
) -> ContainerCommit {
    ContainerCommit {
        acknowledgement: ContainerAcknowledgement {
            request_id,
            container_id,
            result: Err(reason),
        },
        snapshot: None,
        before_events: Vec::new(),
        after_events: Vec::new(),
        released_surfaces: Vec::new(),
        replayed: false,
    }
}

pub(super) fn valid_viewport(viewport: [f32; 4]) -> bool {
    viewport.into_iter().all(f32::is_finite) && viewport[2] > 0.0 && viewport[3] > 0.0
}

pub(super) fn snapshot_size(snapshot: &ContainerSnapshot) -> u64 {
    serde_json::to_vec(snapshot).map_or(u64::MAX, |bytes| bytes.len() as u64)
}

pub(super) fn retained_pixels(snapshot: &ContainerSnapshot) -> u64 {
    snapshot
        .pages
        .iter()
        .map(|page| {
            let size = page.preferred_content_size;
            size.width.ceil().max(1.0) as u64 * size.height.ceil().max(1.0) as u64
        })
        .sum()
}
