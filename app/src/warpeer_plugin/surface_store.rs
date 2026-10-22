use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use warpeer_plugin_protocol::{
    AppearanceKind, ContextId, PluginId, Revision, SurfaceBounds, SurfaceContainerMetrics,
    SurfaceContentSize, SurfaceDescriptor, SurfaceEnvironment, SurfaceFrame, SurfaceId,
    SurfacePalette, SurfaceResizeRequest, SurfaceSlot, SurfaceTarget,
};

use super::surface_resize::SurfaceResizeState;

#[derive(Clone)]
pub(crate) struct SurfaceSnapshot {
    pub plugin_id: PluginId,
    pub descriptor: SurfaceDescriptor,
    pub frame: Option<SurfaceFrame>,
}

pub(crate) struct SurfaceDelivery {
    pub plugin_id: PluginId,
    pub environment: SurfaceEnvironment,
}

struct SurfaceRecord {
    plugin_id: PluginId,
    descriptor: SurfaceDescriptor,
    environment: Option<SurfaceEnvironment>,
    frame: Option<SurfaceFrame>,
    last_presented: Option<Instant>,
    resize: SurfaceResizeState,
}

impl SurfaceRecord {
    fn snapshot(&self, now: Instant) -> SurfaceSnapshot {
        let mut descriptor = self.descriptor.clone();
        let size = self.resize.current(now);
        descriptor.bounds.width = size.width;
        descriptor.bounds.height = size.height;
        SurfaceSnapshot {
            plugin_id: self.plugin_id.clone(),
            descriptor,
            frame: self.frame.clone(),
        }
    }

    fn update_environment(
        &mut self,
        bounds: SurfaceBounds,
        container: SurfaceContainerMetrics,
        appearance: AppearanceKind,
        palette: SurfacePalette,
        reduced_motion: bool,
        focused: bool,
        visible: bool,
        occluded: bool,
        retain_frame_during_resize: bool,
    ) -> Option<SurfaceEnvironment> {
        let previous_raster_size = self.environment.as_ref().map(raster_size);
        let changed = self.environment.as_ref().is_none_or(|current| {
            current.bounds != bounds
                || current.container != container
                || current.appearance != appearance
                || current.palette != palette
                || current.reduced_motion != reduced_motion
                || current.focused != focused
                || current.visible != visible
                || current.occluded != occluded
        });
        if !changed {
            return None;
        }
        let revision = self
            .environment
            .as_ref()
            .map_or(Revision(1), |current| current.revision.next());
        let environment = SurfaceEnvironment {
            surface_id: self.descriptor.id,
            revision,
            bounds,
            container,
            appearance,
            palette,
            reduced_motion,
            focused,
            visible,
            occluded,
        };
        self.environment = Some(environment.clone());
        // Visibility, focus, appearance, and motion changes may not dirty PocketJS output.
        // Keep the last complete frame until a replacement arrives unless the backing size changed.
        if !retain_frame_during_resize
            && previous_raster_size.is_some_and(|size| size != raster_size(&environment))
        {
            self.frame = None;
        }
        Some(environment)
    }
}

fn raster_size(environment: &SurfaceEnvironment) -> (u32, u32) {
    (
        environment.bounds.width.round().max(1.) as u32,
        environment.bounds.height.round().max(1.) as u32,
    )
}

#[derive(Default)]
pub(crate) struct SurfaceStore {
    records: RwLock<HashMap<SurfaceId, SurfaceRecord>>,
}

impl SurfaceStore {
    #[cfg(feature = "integration_tests")]
    pub fn len(&self) -> usize {
        self.records.read().len()
    }

    pub fn open(&self, plugin_id: PluginId, descriptor: SurfaceDescriptor) {
        let resize = SurfaceResizeState::new(
            SurfaceContentSize {
                width: descriptor.bounds.width,
                height: descriptor.bounds.height,
            },
            Instant::now(),
        );
        self.records.write().insert(
            descriptor.id,
            SurfaceRecord {
                plugin_id,
                descriptor,
                environment: None,
                frame: None,
                last_presented: None,
                resize,
            },
        );
    }

    pub fn resize(
        &self,
        plugin_id: &PluginId,
        request: SurfaceResizeRequest,
        now: Instant,
    ) -> bool {
        let mut records = self.records.write();
        let Some(record) = records.get_mut(&request.surface_id) else {
            return false;
        };
        if &record.plugin_id != plugin_id || record.descriptor.revision != request.revision {
            return false;
        }
        let reduced_motion = record
            .environment
            .as_ref()
            .is_some_and(|environment| environment.reduced_motion);
        record
            .resize
            .request(request.content_size, request.animation, reduced_motion, now);
        record.descriptor.bounds.width = request.content_size.width;
        record.descriptor.bounds.height = request.content_size.height;
        true
    }

    pub fn close(&self, plugin_id: &PluginId, id: SurfaceId, revision: Revision) -> bool {
        let mut records = self.records.write();
        if records.get(&id).is_some_and(|record| {
            &record.plugin_id == plugin_id && record.descriptor.revision <= revision
        }) {
            records.remove(&id);
            true
        } else {
            false
        }
    }

    pub fn apply_frame(&self, plugin_id: &PluginId, frame: SurfaceFrame) -> bool {
        let mut records = self.records.write();
        let Some(record) = records.get_mut(&frame.surface_id) else {
            return false;
        };
        let Some(environment) = record.environment.as_ref() else {
            return false;
        };
        let (expected_width, expected_height) = raster_size(environment);
        if &record.plugin_id != plugin_id
            || record.descriptor.revision != frame.surface_revision
            || environment.revision != frame.environment_revision
            || frame.width_px != expected_width
            || frame.height_px != expected_height
            || record
                .frame
                .as_ref()
                .is_some_and(|current| frame.frame_revision <= current.frame_revision)
        {
            return false;
        }
        record.frame = Some(frame);
        true
    }

    pub fn for_target(
        &self,
        slot: SurfaceSlot,
        target: &SurfaceTarget,
        revision: Revision,
    ) -> Option<SurfaceSnapshot> {
        self.records
            .read()
            .values()
            .find(|record| {
                record.descriptor.slot == slot
                    && &record.descriptor.target == target
                    && record.descriptor.revision == revision
            })
            .map(|record| record.snapshot(Instant::now()))
    }

    pub fn snapshot(&self, id: SurfaceId) -> Option<SurfaceSnapshot> {
        self.records
            .read()
            .get(&id)
            .map(|record| record.snapshot(Instant::now()))
    }

    pub fn for_pane(&self, pane_id: &str) -> Vec<SurfaceSnapshot> {
        let mut surfaces = self
            .records
            .read()
            .values()
            .filter(|record| record.descriptor.target.pane_id() == Some(pane_id))
            .map(|record| record.snapshot(Instant::now()))
            .collect::<Vec<_>>();
        surfaces.sort_by(|left, right| {
            left.descriptor
                .slot
                .cmp(&right.descriptor.slot)
                .then_with(|| left.plugin_id.cmp(&right.plugin_id))
        });
        surfaces
    }

    pub fn for_settings(&self, context_id: ContextId) -> Vec<SurfaceSnapshot> {
        self.records
            .read()
            .values()
            .filter(|record| {
                matches!(
                    record.descriptor.target,
                    SurfaceTarget::PluginSettings { context_id: candidate } if candidate == context_id
                )
            })
            .map(|record| record.snapshot(Instant::now()))
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &self,
        id: SurfaceId,
        bounds: SurfaceBounds,
        container: SurfaceContainerMetrics,
        appearance: AppearanceKind,
        palette: SurfacePalette,
        reduced_motion: bool,
        focused: bool,
        visible: bool,
        occluded: bool,
        now: Instant,
    ) -> Option<SurfaceDelivery> {
        let mut records = self.records.write();
        let record = records.get_mut(&id)?;
        record.last_presented = Some(now);
        let retain_frame_during_resize = record.resize.retains_previous_frame();
        record
            .update_environment(
                bounds,
                container,
                appearance,
                palette,
                reduced_motion,
                focused,
                visible,
                occluded,
                retain_frame_during_resize,
            )
            .map(|environment| SurfaceDelivery {
                plugin_id: record.plugin_id.clone(),
                environment,
            })
    }

    pub fn occlude_unpresented(&self, now: Instant, timeout: Duration) -> Vec<SurfaceDelivery> {
        let mut records = self.records.write();
        records
            .values_mut()
            .filter_map(|record| {
                let current = record.environment.as_ref()?.clone();
                let last_presented = record.last_presented?;
                if !current.visible || now.saturating_duration_since(last_presented) < timeout {
                    return None;
                }
                let environment = record.update_environment(
                    current.bounds,
                    current.container,
                    current.appearance,
                    current.palette,
                    current.reduced_motion,
                    current.focused,
                    false,
                    true,
                    false,
                )?;
                Some(SurfaceDelivery {
                    plugin_id: record.plugin_id.clone(),
                    environment,
                })
            })
            .collect()
    }

    pub fn owner(
        &self,
        id: SurfaceId,
    ) -> Option<(PluginId, SurfaceDescriptor, Option<SurfaceEnvironment>)> {
        self.records.read().get(&id).map(|record| {
            (
                record.plugin_id.clone(),
                record.snapshot(Instant::now()).descriptor,
                record.environment.clone(),
            )
        })
    }

    pub fn remove_context(&self, context_id: ContextId) -> Vec<PluginId> {
        let mut records = self.records.write();
        let owners = records
            .values()
            .filter(|record| record.descriptor.target.context_id() == context_id)
            .map(|record| record.plugin_id.clone())
            .collect::<Vec<_>>();
        records.retain(|_, record| record.descriptor.target.context_id() != context_id);
        owners
    }

    pub fn remove_plugin(&self, plugin_id: &PluginId) {
        self.records
            .write()
            .retain(|_, record| &record.plugin_id != plugin_id);
    }
}

#[cfg(test)]
#[path = "surface_store_tests.rs"]
mod tests;
