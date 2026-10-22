use std::collections::BTreeMap;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, thread};

use anyhow::{Context, Result, bail};
use pocketjs_adapter::{PocketRuntime, PocketTexture};
use warpeer_plugin_protocol::{
    BridgePayload, CURRENT_PROTOCOL_VERSION, CURRENT_RUNTIME_ABI, CapabilityResponse, Envelope,
    FrameTransport, HostToPlugin, LifecycleEvent, PluginId, PluginManifest, PluginToHost,
    ProtocolLimits, SurfaceDescriptor, SurfaceEnvironment, SurfaceFrame, SurfaceId, WorkerId,
    WorkerState, read_framed, write_framed,
};
#[cfg(target_os = "macos")]
use warpeer_plugin_surface_transport::ProducerSurface;

const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const DEFAULT_SURFACE_SIZE: (u32, u32) = (640, 480);

mod image_resource;
mod watchdog;
use image_resource::load_image_resource;
use watchdog::ExecutionWatchdog;

pub fn run(plugin_id: &str, package_root: &Path, execution_slice_ms: u32) -> Result<()> {
    let plugin_id = PluginId::parse(plugin_id.to_owned()).context("invalid plugin ID")?;
    let manifest = load_manifest(package_root)?;
    if manifest.id != plugin_id {
        bail!("worker plugin ID does not match package manifest");
    }
    let bundle = fs::read(package_root.join(&manifest.entrypoints.bundle))
        .context("read PocketJS plugin bundle")?;
    let assets = fs::read(package_root.join(&manifest.entrypoints.assets))
        .context("read PocketJS plugin assets")?;
    let limits = ProtocolLimits {
        max_payload_bytes: manifest
            .budgets
            .max_surface_pixels
            .saturating_mul(4)
            .max(8 * 1024 * 1024),
        max_queued_messages: manifest.budgets.queued_messages as usize,
    };
    WorkerEngine::new(
        plugin_id,
        bundle,
        assets,
        limits,
        execution_slice_ms.min(manifest.budgets.execution_slice_ms),
    )?
    .run_stdio()
}

fn load_manifest(package_root: &Path) -> Result<PluginManifest> {
    let source =
        fs::read_to_string(package_root.join("plugin.toml")).context("read plugin.toml")?;
    toml::from_str(&source).context("parse plugin.toml")
}

#[derive(Clone)]
struct WorkerSurface {
    descriptor: SurfaceDescriptor,
    environment: Option<SurfaceEnvironment>,
}

struct WorkerEngine {
    plugin_id: PluginId,
    worker_id: WorkerId,
    runtime: PocketRuntime,
    limits: ProtocolLimits,
    watchdog: ExecutionWatchdog,
    started: bool,
    suspended: bool,
    surfaces: BTreeMap<SurfaceId, WorkerSurface>,
    render_size: (u32, u32),
    texture: Option<PocketTexture>,
    #[cfg(target_os = "macos")]
    frame_surfaces: BTreeMap<SurfaceId, ProducerSurface>,
}

impl WorkerEngine {
    fn new(
        plugin_id: PluginId,
        bundle: Vec<u8>,
        assets: Vec<u8>,
        limits: ProtocolLimits,
        execution_slice_ms: u32,
    ) -> Result<Self> {
        Ok(Self {
            plugin_id,
            worker_id: WorkerId::new(),
            runtime: PocketRuntime::load(&bundle, &assets, DEFAULT_SURFACE_SIZE, 1)
                .context("load PocketJS runtime")?,
            limits,
            watchdog: ExecutionWatchdog::new(Duration::from_millis(
                debug_execution_slice_ms(execution_slice_ms).into(),
            )),
            started: false,
            suspended: false,
            surfaces: BTreeMap::new(),
            render_size: DEFAULT_SURFACE_SIZE,
            texture: None,
            #[cfg(target_os = "macos")]
            frame_surfaces: BTreeMap::new(),
        })
    }

    fn run_stdio(mut self) -> Result<()> {
        let (input_tx, input_rx) = mpsc::sync_channel(self.limits.max_queued_messages.max(1));
        let limits = self.limits;
        thread::Builder::new()
            .name("warpeer-plugin-ipc-reader".into())
            .spawn(move || {
                let mut input = BufReader::new(std::io::stdin());
                loop {
                    match read_framed::<Envelope<HostToPlugin>>(&mut input, limits) {
                        Ok(Some(message)) => {
                            if input_tx.send(Ok(message)).is_err() {
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            let _ = input_tx.send(Err(error.to_string()));
                            break;
                        }
                    }
                }
            })
            .context("spawn plugin IPC reader")?;

        let mut output = BufWriter::new(std::io::stdout());
        loop {
            match input_rx.recv_timeout(FRAME_INTERVAL) {
                Ok(Ok(envelope)) => {
                    validate_envelope(&envelope)?;
                    if self.handle_host_message(envelope.message, &mut output)? {
                        return Ok(());
                    }
                }
                Ok(Err(error)) => bail!("plugin IPC input failed: {error}"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            }
            if self.started {
                self.drive_frame(&mut output)?;
            }
        }
    }

    fn handle_host_message(
        &mut self,
        message: HostToPlugin,
        output: &mut BufWriter<std::io::Stdout>,
    ) -> Result<bool> {
        match message {
            start @ HostToPlugin::Start { worker_id, .. } => {
                self.worker_id = worker_id;
                self.started = true;
                self.forward(&start)?;
                self.send_lifecycle(WorkerState::Ready, None, output)?;
            }
            HostToPlugin::Stop | HostToPlugin::Uninstall => {
                self.send_lifecycle(WorkerState::Stopped, None, output)?;
                return Ok(true);
            }
            HostToPlugin::Suspend => self.suspended = true,
            HostToPlugin::Resume => self.suspended = false,
            HostToPlugin::SurfaceEnvironment(environment) => {
                self.apply_environment(&environment)?;
                self.forward(&HostToPlugin::SurfaceEnvironment(environment))?;
            }
            release @ HostToPlugin::ReleaseSurface {
                surface_id,
                revision,
            } => {
                self.surfaces.remove(&surface_id);
                #[cfg(target_os = "macos")]
                self.frame_surfaces.remove(&surface_id);
                if self.surfaces.is_empty() {
                    self.release_textures()?;
                }
                self.forward(&release)?;
                write_message(
                    output,
                    PluginToHost::SurfaceReleased {
                        surface_id,
                        revision,
                    },
                    self.limits,
                )?;
            }
            HostToPlugin::CapabilityResponse(response) => {
                let response = self.materialize_payload(response)?;
                self.forward(&HostToPlugin::CapabilityResponse(response))?;
            }
            cancel @ HostToPlugin::CancelContext { context_id, .. } => {
                let cancelled = self
                    .surfaces
                    .values()
                    .filter(|surface| surface.descriptor.target.context_id() == context_id)
                    .map(|surface| surface.descriptor.id)
                    .collect::<Vec<_>>();
                for surface_id in cancelled {
                    self.surfaces.remove(&surface_id);
                    #[cfg(target_os = "macos")]
                    self.frame_surfaces.remove(&surface_id);
                }
                self.release_textures()?;
                self.forward(&cancel)?;
            }
            other => self.forward(&other)?,
        }
        Ok(false)
    }

    fn apply_environment(&mut self, environment: &SurfaceEnvironment) -> Result<()> {
        let surface = self
            .surfaces
            .get_mut(&environment.surface_id)
            .context("environment references an unknown surface")?;
        if surface
            .environment
            .as_ref()
            .is_some_and(|current| current.revision >= environment.revision)
        {
            return Ok(());
        }
        surface.environment = Some(environment.clone());
        Ok(())
    }

    fn materialize_payload(&mut self, response: CapabilityResponse) -> Result<CapabilityResponse> {
        let result = match response.result {
            Ok(BridgePayload::ImageResource {
                url,
                source_url,
                max_bytes,
                max_width,
                max_height,
            }) => {
                let result = load_image_resource(&source_url, max_bytes, max_width, max_height)
                    .and_then(|resource| {
                        let image = resource.image;
                        let media_type = resource.media_type;
                        self.runtime
                            .register_image_url(&url, &image.rgba, image.width, image.height)
                            .context("register bridge image URL")
                            .map(|texture| (image, media_type, texture))
                    });
                let (image, media_type, texture) = result?;
                if let Some(previous) = self.texture.replace(texture) {
                    self.runtime.free_texture(previous)?;
                }
                Ok(BridgePayload::Json {
                    value: serde_json::json!({
                        "url": url,
                        "mediaType": media_type,
                        "width": image.width,
                        "height": image.height,
                        "originalWidth": image.original_width,
                        "originalHeight": image.original_height,
                    }),
                })
            }
            Ok(BridgePayload::RgbaImage {
                media_type,
                width,
                height,
                pixels,
            }) => {
                let operation = self.watchdog.begin();
                let result = self
                    .runtime
                    .upload_rgba(&pixels, width, height)
                    .context("upload bridge image");
                operation.complete();
                let texture = result?;
                if let Some(previous) = self.texture.replace(texture) {
                    self.runtime.free_texture(previous)?;
                }
                Ok(BridgePayload::Json {
                    value: serde_json::json!({
                        "texture": texture.0,
                        "mediaType": media_type,
                        "width": width,
                        "height": height,
                    }),
                })
            }
            result => result,
        };
        Ok(CapabilityResponse {
            request_id: response.request_id,
            result,
        })
    }

    fn forward(&mut self, message: &HostToPlugin) -> Result<()> {
        let operation = self.watchdog.begin();
        let result = self
            .runtime
            .post_event(&serde_json::to_string(message)?)
            .context("deliver host event to PocketJS");
        operation.complete();
        result
    }

    fn drive_frame(&mut self, output: &mut BufWriter<std::io::Stdout>) -> Result<()> {
        let operation = self.watchdog.begin();
        self.runtime.tick(&[]).context("tick PocketJS")?;
        operation.complete();
        for effect in self.runtime.drain_effects() {
            let message: PluginToHost = serde_json::from_str(&effect)
                .with_context(|| format!("invalid plugin effect: {effect}"))?;
            match &message {
                PluginToHost::OpenSurface(descriptor) => {
                    self.surfaces.insert(
                        descriptor.id,
                        WorkerSurface {
                            descriptor: descriptor.clone(),
                            environment: None,
                        },
                    );
                }
                PluginToHost::CloseSurface { surface_id, .. } => {
                    self.surfaces.remove(surface_id);
                    #[cfg(target_os = "macos")]
                    self.frame_surfaces.remove(surface_id);
                    if self.surfaces.is_empty() {
                        self.release_textures()?;
                    }
                }
                _ => {}
            }
            write_message(output, message, self.limits)?;
        }
        if self.suspended || self.surfaces.is_empty() {
            return Ok(());
        }
        let mut by_size =
            BTreeMap::<(u32, u32), Vec<(SurfaceDescriptor, SurfaceEnvironment)>>::new();
        for surface in self.surfaces.values() {
            let Some(environment) = surface.environment.as_ref() else {
                continue;
            };
            if !environment.visible || environment.occluded {
                continue;
            }
            let size = (
                environment.bounds.width.round().max(1.) as u32,
                environment.bounds.height.round().max(1.) as u32,
            );
            by_size
                .entry(size)
                .or_default()
                .push((surface.descriptor.clone(), environment.clone()));
        }
        for (size, surfaces) in by_size {
            if self.render_size != size {
                let operation = self.watchdog.begin();
                let result = self.runtime.resize(size).context("resize PocketJS surface");
                operation.complete();
                result?;
                self.render_size = size;
            }
            let operation = self.watchdog.begin();
            let frame = self.runtime.render().context("render PocketJS")?;
            operation.complete();
            let Some(frame) = frame else {
                continue;
            };
            for (descriptor, environment) in surfaces {
                let transport = self.frame_transport(descriptor.id, &frame)?;
                write_message(
                    output,
                    PluginToHost::Frame(SurfaceFrame {
                        surface_id: descriptor.id,
                        surface_revision: descriptor.revision,
                        environment_revision: environment.revision,
                        frame_revision: frame.revision,
                        width_px: frame.width_px,
                        height_px: frame.height_px,
                        dirty_regions: frame
                            .dirty_regions
                            .iter()
                            .map(|region| region.map(|value| value.max(0) as u32))
                            .collect(),
                        transport,
                    }),
                    self.limits,
                )?;
            }
        }
        Ok(())
    }

    fn frame_transport(
        &mut self,
        surface_id: SurfaceId,
        frame: &pocketjs_adapter::OwnedFrame,
    ) -> Result<FrameTransport> {
        #[cfg(target_os = "macos")]
        {
            if !self
                .frame_surfaces
                .get(&surface_id)
                .is_some_and(|surface| surface.matches(frame.width_px, frame.height_px))
            {
                self.frame_surfaces.insert(
                    surface_id,
                    ProducerSurface::new(frame.width_px, frame.height_px)
                        .context("create plugin IOSurface")?,
                );
            }
            let token = self
                .frame_surfaces
                .get(&surface_id)
                .expect("plugin IOSurface was initialized")
                .write_bgra(&frame.pixels, frame.stride_bytes)
                .context("write plugin IOSurface")?;
            Ok(FrameTransport::IoSurface {
                surface_id: token.surface_id,
                seed: token.seed,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(FrameTransport::SharedBuffer {
                bytes: frame.pixels.clone(),
                stride_bytes: frame.stride_bytes,
            })
        }
    }

    fn send_lifecycle(
        &self,
        state: WorkerState,
        detail: Option<String>,
        output: &mut BufWriter<std::io::Stdout>,
    ) -> Result<()> {
        write_message(
            output,
            PluginToHost::Lifecycle(LifecycleEvent {
                plugin_id: self.plugin_id.clone(),
                worker_id: self.worker_id,
                state,
                detail,
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            }),
            self.limits,
        )
    }

    fn release_textures(&mut self) -> Result<()> {
        if let Some(texture) = self.texture.take() {
            self.runtime.free_texture(texture)?;
        }
        Ok(())
    }
}

fn debug_execution_slice_ms(execution_slice_ms: u32) -> u32 {
    if cfg!(debug_assertions) {
        execution_slice_ms.saturating_mul(4).max(1)
    } else {
        execution_slice_ms.max(1)
    }
}

impl Drop for WorkerEngine {
    fn drop(&mut self) {
        let _ = self.release_textures();
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

fn validate_envelope(envelope: &Envelope<HostToPlugin>) -> Result<()> {
    if envelope.protocol != CURRENT_PROTOCOL_VERSION || envelope.abi != CURRENT_RUNTIME_ABI {
        bail!("incompatible plugin IPC protocol or runtime ABI");
    }
    Ok(())
}

fn write_message(
    output: &mut BufWriter<std::io::Stdout>,
    message: PluginToHost,
    limits: ProtocolLimits,
) -> Result<()> {
    write_framed(
        output,
        &Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            abi: CURRENT_RUNTIME_ABI,
            message,
        },
        limits,
    )
    .context("write plugin IPC message")
}
