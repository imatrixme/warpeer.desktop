use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Cursor, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use image::{DynamicImage, ImageFormat};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tempfile::tempdir;
use url::Url;
use warpeer_plugin_protocol::{
    AppearanceKind, BridgePayload, CURRENT_PROTOCOL_VERSION, CURRENT_RUNTIME_ABI,
    CapabilityResponse, ContainerAcknowledgement, ContainerApplied, ContainerExtension, ContextId,
    Envelope, ExtensionSupport, FrameTransport, HostToPlugin, PluginId, PluginToHost,
    ProtocolLimits, Revision, SurfaceEnvironment, SurfaceFrame, TerminalHoverContext, WorkerId,
    WorkerState, read_framed, write_framed,
};
use warpeer_plugin_runtime::validate_archive;

#[test]
fn worker_process_completes_framed_lifecycle_handshake() {
    let package = tempdir().unwrap();
    fs::create_dir_all(package.path().join("dist")).unwrap();
    fs::write(
        package.path().join("dist/main.pocketjs"),
        b"globalThis.frame = function() {};",
    )
    .unwrap();
    fs::write(package.path().join("dist/assets.pak"), b"pak").unwrap();
    fs::write(package.path().join("plugin.toml"), manifest()).unwrap();

    let plugin_id = PluginId::parse("dev.warpeer.worker-fixture").unwrap();
    let worker_id = WorkerId::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_worker_fixture"))
        .arg(plugin_id.as_str())
        .arg(package.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = BufWriter::new(child.stdin.take().unwrap());
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let limits = ProtocolLimits::default();

    write_framed(
        &mut input,
        &Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            abi: CURRENT_RUNTIME_ABI,
            message: HostToPlugin::Start {
                plugin_id: plugin_id.clone(),
                worker_id,
                extensions: Vec::new(),
                container_limits: Default::default(),
            },
        },
        limits,
    )
    .unwrap();
    let ready = read_framed(&mut output, limits)
        .unwrap()
        .unwrap_or_else(|| {
            let status = child.wait().unwrap();
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            panic!("worker exited before Ready ({status}): {stderr}");
        });
    assert_lifecycle(ready, WorkerState::Ready);

    write_framed(
        &mut input,
        &Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            abi: CURRENT_RUNTIME_ABI,
            message: HostToPlugin::Stop,
        },
        limits,
    )
    .unwrap();
    assert_lifecycle(
        read_framed(&mut output, limits).unwrap().unwrap(),
        WorkerState::Stopped,
    );
    assert!(child.wait().unwrap().success());
}

#[test]
fn bundled_image_preview_opens_frames_and_closes_repeatedly() {
    let package = tempdir().unwrap();
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/image-preview/dist/image-preview.warpeerplugin");
    let validated = validate_archive(File::open(archive).unwrap()).unwrap();
    for (path, bytes) in validated.files {
        let destination = package.path().join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, bytes).unwrap();
    }
    let image_path = package.path().join("fixture.png");
    let encoded_image = encoded_png(3, 2);
    fs::write(&image_path, &encoded_image).unwrap();
    let image_source_url = Url::from_file_path(&image_path).unwrap().to_string();

    let plugin_id = PluginId::parse("dev.warpeer.image-preview").unwrap();
    let worker_id = WorkerId::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_worker_fixture"))
        .arg(plugin_id.as_str())
        .arg(package.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut input = BufWriter::new(child.stdin.take().unwrap());
    let output = child.stdout.take().unwrap();
    let (output_tx, output_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut output = BufReader::new(output);
        loop {
            match read_framed::<Envelope<PluginToHost>>(&mut output, ProtocolLimits::default()) {
                Ok(Some(message)) => {
                    if output_tx.send(Ok(message)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = output_tx.send(Err(error.to_string()));
                    break;
                }
            }
        }
    });

    send(
        &mut input,
        HostToPlugin::Start {
            plugin_id: plugin_id.clone(),
            worker_id,
            extensions: vec![ExtensionSupport {
                extension: ContainerExtension::Chrome,
                version: 1,
            }],
            container_limits: Default::default(),
        },
    );
    receive_matching(&output_rx, |message| match message {
        PluginToHost::Lifecycle(event) if event.state == WorkerState::Ready => Some(()),
        _ => None,
    });
    send(
        &mut input,
        HostToPlugin::Configuration {
            revision: Revision(1),
            values: serde_json::json!({
                "hover_delay_ms": 0,
                "trigger_mode": "Hover"
            }),
        },
    );

    let mut idle_memory = Vec::new();
    for revision in 1..=3 {
        let context_id = ContextId::new();
        send(
            &mut input,
            HostToPlugin::TerminalHover(TerminalHoverContext {
                context_id,
                revision: Revision(revision),
                terminal_id: "terminal-fixture".into(),
                pane_id: "pane-fixture".into(),
                workspace_id: None,
                working_directory: Some("/tmp".into()),
                resource_text: "fixture.png".into(),
                resolved_resource: Some("/tmp/fixture.png".into()),
                explicit_hyperlink: false,
                explicit_preview: revision == 1,
                anchor: [20.0, 20.0, 8.0, 16.0],
                viewport: [0.0, 0.0, 800.0, 600.0],
            }),
        );
        let request_id = receive_matching(&output_rx, |message| match message {
            PluginToHost::CapabilityRequest(request) => Some(request.request_id),
            _ => None,
        });
        send(
            &mut input,
            HostToPlugin::CapabilityResponse(CapabilityResponse {
                request_id,
                result: Ok(BridgePayload::ImageResource {
                    url: format!("warpeer-resource://preview/{request_id}"),
                    source_url: image_source_url.clone(),
                    max_bytes: encoded_image.len() as u64,
                    max_width: 560,
                    max_height: 420,
                }),
            }),
        );
        let descriptor = receive_matching(&output_rx, |message| match message {
            PluginToHost::OpenSurface(descriptor) if descriptor.revision == Revision(revision) => {
                Some(descriptor.clone())
            }
            _ => None,
        });
        assert_eq!(descriptor.bounds.width, 264.);
        assert_eq!(descriptor.bounds.height, 212.);
        assert!(!descriptor.accepts_pointer);
        let surface_id = descriptor.id;
        let pinned_container = (revision == 1).then(|| {
            receive_matching(&output_rx, |message| match message {
                PluginToHost::OpenContainer(request) => Some(request.clone()),
                _ => None,
            })
        });
        if let Some(request) = pinned_container.as_ref() {
            send(
                &mut input,
                HostToPlugin::ContainerAcknowledgement(ContainerAcknowledgement {
                    request_id: request.request_id,
                    container_id: request.container_id,
                    result: Ok(ContainerApplied {
                        revision: Revision(1),
                        content_size: request.root.preferred_content_size,
                        clamped: false,
                        transition: request.transition,
                    }),
                }),
            );
        }
        let mut environment = SurfaceEnvironment {
            surface_id,
            revision: Revision(1),
            bounds: descriptor.bounds,
            container: warpeer_plugin_protocol::SurfaceContainerMetrics::default(),
            appearance: AppearanceKind::Dark,
            palette: warpeer_plugin_protocol::SurfacePalette::dark(),
            reduced_motion: false,
            focused: false,
            visible: revision != 3,
            occluded: revision == 3,
        };
        environment.bounds.scale = 2.;
        send(
            &mut input,
            HostToPlugin::SurfaceEnvironment(environment.clone()),
        );
        if revision == 2 {
            environment.revision = Revision(2);
            environment.bounds.width -= 20.;
            send(
                &mut input,
                HostToPlugin::SurfaceEnvironment(environment.clone()),
            );
        }
        if revision == 3 {
            assert_no_frame_for_surface(&output_rx, surface_id, Duration::from_millis(100));
            environment.revision = Revision(2);
            environment.visible = true;
            environment.occluded = false;
            send(
                &mut input,
                HostToPlugin::SurfaceEnvironment(environment.clone()),
            );
        }
        let frame = receive_matching(&output_rx, |message| match message {
            PluginToHost::Frame(frame)
                if frame.surface_id == surface_id
                    && frame.surface_revision == Revision(revision)
                    && frame.environment_revision == environment.revision
                    && frame.width_px == environment.bounds.width.round() as u32
                    && frame.height_px == environment.bounds.height.round() as u32 =>
            {
                Some(frame.clone())
            }
            _ => None,
        });
        assert_complete_preview(&frame);
        send(
            &mut input,
            HostToPlugin::CancelContext {
                context_id,
                revision: Revision(revision),
            },
        );
        receive_matching(&output_rx, |message| match message {
            PluginToHost::CloseSurface {
                surface_id: closed,
                revision: closed_revision,
            } if *closed == surface_id && *closed_revision == Revision(revision) => Some(()),
            _ => None,
        });
        thread::sleep(Duration::from_millis(50));
        idle_memory.push(process_memory(child.id()));
    }

    assert!(
        idle_memory[2] <= idle_memory[0].saturating_add(8 * 1024 * 1024),
        "worker idle memory grew across identical preview cycles: {idle_memory:?}"
    );

    send(&mut input, HostToPlugin::Stop);
    receive_matching(&output_rx, |message| match message {
        PluginToHost::Lifecycle(event) if event.state == WorkerState::Stopped => Some(()),
        _ => None,
    });
    assert!(child.wait().unwrap().success());
}

#[cfg(target_os = "macos")]
fn assert_complete_preview(frame: &SurfaceFrame) {
    let expected = [
        ((52, 52), [255, 0, 0, 255]),
        ((132, 52), [0, 255, 0, 255]),
        ((212, 52), [0, 0, 255, 255]),
        ((52, 132), [0, 255, 255, 255]),
        ((132, 132), [255, 0, 255, 255]),
        ((212, 132), [255, 255, 0, 255]),
    ];
    for (point, color) in expected {
        assert_eq!(
            preview_pixel(frame, point),
            color,
            "preview image did not preserve its complete 3 x 2 sampling area at {point:?}"
        );
    }
}

#[cfg(target_os = "macos")]
fn preview_pixel(frame: &SurfaceFrame, point: (usize, usize)) -> [u8; 4] {
    let FrameTransport::IoSurface { surface_id, seed } = frame.transport else {
        panic!("macOS Image Preview frames should use IOSurface transport");
    };
    let readback = warpeer_plugin_surface_transport::read_surface(
        surface_id,
        seed,
        frame.width_px,
        frame.height_px,
    )
    .unwrap();
    let (x, y) = point;
    let offset = y * readback.stride_bytes as usize + x * 4;
    let bgra = &readback.bgra[offset..offset + 4];
    [bgra[2], bgra[1], bgra[0], bgra[3]]
}

#[cfg(not(target_os = "macos"))]
fn assert_complete_preview(_: &SurfaceFrame) {}

#[cfg(not(target_os = "macos"))]
fn preview_pixel(_: &SurfaceFrame, _: (usize, usize)) -> [u8; 4] {
    [30, 60, 90, 255]
}

fn encoded_png(width: u32, height: u32) -> Vec<u8> {
    let pixels = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 255, 255, 255, 255, 0, 255, 255, 255,
        255, 0, 255,
    ];
    let image = image::RgbaImage::from_raw(width, height, pixels.to_vec()).unwrap();
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn process_memory(process_id: u32) -> u64 {
    let pid = Pid::from_u32(process_id);
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    system
        .process(pid)
        .expect("plugin worker should still be running")
        .memory()
}

fn send(input: &mut BufWriter<impl std::io::Write>, message: HostToPlugin) {
    write_framed(
        input,
        &Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            abi: CURRENT_RUNTIME_ABI,
            message,
        },
        ProtocolLimits::default(),
    )
    .unwrap();
}

fn receive_matching<T>(
    output: &mpsc::Receiver<Result<Envelope<PluginToHost>, String>>,
    mut select: impl FnMut(&PluginToHost) -> Option<T>,
) -> T {
    let mut recent = VecDeque::with_capacity(8);
    loop {
        let envelope = match output.recv_timeout(Duration::from_secs(5)) {
            Ok(envelope) => envelope.expect("plugin worker framing failed"),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("plugin worker response timed out; recent messages: {recent:#?}")
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("plugin worker output disconnected; recent messages: {recent:#?}")
            }
        };
        if let Some(value) = select(&envelope.message) {
            return value;
        }
        if recent.len() == recent.capacity() {
            recent.pop_front();
        }
        recent.push_back(format!("{:?}", envelope.message));
    }
}

fn assert_no_frame_for_surface(
    output: &mpsc::Receiver<Result<Envelope<PluginToHost>, String>>,
    surface_id: warpeer_plugin_protocol::SurfaceId,
    duration: Duration,
) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match output.recv_timeout(remaining.min(Duration::from_millis(20))) {
            Ok(Ok(envelope)) => assert!(
                !matches!(
                    envelope.message,
                    PluginToHost::Frame(ref frame) if frame.surface_id == surface_id
                ),
                "occluded surface produced a presentation frame"
            ),
            Ok(Err(error)) => panic!("plugin worker framing failed: {error}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("plugin worker output disconnected")
            }
        }
    }
}

fn assert_lifecycle(envelope: Envelope<PluginToHost>, expected: WorkerState) {
    assert_eq!(envelope.protocol, CURRENT_PROTOCOL_VERSION);
    assert_eq!(envelope.abi, CURRENT_RUNTIME_ABI);
    match envelope.message {
        PluginToHost::Lifecycle(event) => assert_eq!(event.state, expected),
        message => panic!("expected lifecycle message, got {message:?}"),
    }
}

fn manifest() -> &'static str {
    r#"id = "dev.warpeer.worker-fixture"
name = "Worker Fixture"
description = "Exercises the production worker protocol."
version = "1.0.0"

[runtime]
protocol = 1
abi = ">=1.0.0, <2.0.0"
engine = "pocket-surface"

[entrypoints]
bundle = "dist/main.pocketjs"
assets = "dist/assets.pak"

[hashes]

[budgets]
memory_bytes = 100663296
execution_slice_ms = 16
queued_messages = 32
active_surfaces = 1
max_surface_pixels = 1048576
max_frame_rate = 60
cache_bytes = 0
"#
}
