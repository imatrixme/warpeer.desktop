use std::collections::HashMap;
use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::mem;
use std::ptr::NonNull;
use std::sync::Once;

use dispatch2::DispatchData;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
#[cfg(feature = "external_surfaces")]
use objc2_io_surface::IOSurfaceRef;
#[cfg(feature = "external_surfaces")]
use objc2_metal::MTLTextureUsage;
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLBuffer, MTLClearColor, MTLCommandBuffer,
    MTLCommandEncoder, MTLCommandQueue, MTLDevice, MTLDrawable, MTLFunction, MTLIndexType,
    MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion,
    MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState, MTLResourceOptions, MTLScissorRect, MTLSize, MTLStoreAction,
    MTLTexture, MTLTextureDescriptor, MTLViewport,
};
use objc2_quartz_core::CAMetalDrawable;
use pathfinder_color::{ColorF, ColorU};
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warp_errors::report_error;
use warpui_core::fonts::{self, RasterizedGlyph, SubpixelAlignment, canvas};
use warpui_core::platform::CapturedFrame;
use warpui_core::rendering::texture_cache::TextureCache;
use warpui_core::rendering::{self};
use warpui_core::scene::{CornerRadius, GlyphFade, GlyphKey, Icon, Image, Layer, Scene};
#[cfg(feature = "terminal_visuals")]
use warpui_core::terminal_visuals::TerminalVisualLayer;

use super::frame_capture::capture_frame;
use crate::platform::mac::rendering::renderer::Device;
use crate::platform::mac::window::WindowState;
use crate::rendering::atlas::{AllocatedRegion, TextureId};
use crate::rendering::{GlyphCache, GlyphRasterBoundsFn, RasterizeGlyphFn, get_best_dash_gap};

const METAL_LIB_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaders.metallib"));
static WRITE_LIB_TO_FILE: Once = Once::new();

/// A structure to help manage a single rendering pass.
struct RenderPass<'a> {
    drawable: &'a ProtocolObject<dyn CAMetalDrawable>,
    buffer: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    encoder: Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>,
    encoding_finished: bool,
}

impl<'a> RenderPass<'a> {
    fn new(
        command_queue: &ProtocolObject<dyn MTLCommandQueue>,
        drawable: &'a ProtocolObject<dyn CAMetalDrawable>,
    ) -> Self {
        let buffer = command_queue
            .commandBuffer()
            .expect("command queue should always vend a command buffer");
        let encoder = buffer
            .renderCommandEncoderWithDescriptor(&Self::create_descriptor(drawable))
            .expect("command buffer should always vend a render command encoder");
        Self {
            drawable,
            buffer,
            encoder,
            encoding_finished: false,
        }
    }

    /// Finishes a render pass with optional frame capture.
    ///
    /// If this is not called, the encoded commands will not be executed and the
    /// drawable will not be updated.
    ///
    /// Returns the captured frame data if capture was requested.
    fn finish_with_capture(
        mut self,
        drawable_size: pathfinder_geometry::vector::Vector2F,
        should_capture: bool,
        presents_with_transaction: bool,
    ) -> Option<CapturedFrame> {
        self.encoder.endEncoding();
        self.encoding_finished = true;

        // If we're able to do asynchronous presentation, do so - it allows us to avoid
        // blocking on the GPU for the duration of the frame.
        if !should_capture && !presents_with_transaction {
            self.buffer
                .presentDrawable(ProtocolObject::from_ref(self.drawable));
            self.buffer.commit();
            return None;
        }

        // Otherwise, commit the buffer and wait for it to complete before continuing.
        self.buffer.commit();
        self.buffer.waitUntilCompleted();

        let capture = if should_capture {
            let texture = self.drawable.texture();
            capture_frame(&texture, drawable_size)
        } else {
            None
        };

        self.drawable.present();
        capture
    }

    /// Creates a descriptor for a pass that renders into the provided drawable.
    fn create_descriptor(
        drawable: &ProtocolObject<dyn CAMetalDrawable>,
    ) -> Retained<MTLRenderPassDescriptor> {
        let descriptor = MTLRenderPassDescriptor::new();

        // SAFETY: index 0 is always a valid color attachment slot for a CAMetalLayer's drawable.
        let color_attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
        color_attachment.setTexture(Some(&drawable.texture()));
        color_attachment.setLoadAction(MTLLoadAction::Clear);
        color_attachment.setStoreAction(MTLStoreAction::Store);
        color_attachment.setClearColor(MTLClearColor {
            red: 0.,
            green: 0.,
            blue: 0.,
            alpha: 0.,
        });

        descriptor
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        // Make sure that `end_encoding()` is called, even if a panic occurs
        // during rendering.
        if !self.encoding_finished {
            self.encoder.endEncoding();
        }
    }
}

/// A set of resources necessary for rendering that retain state across frames.
struct Resources {
    draw_rects_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    draw_images_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    draw_glyphs_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    #[cfg(feature = "terminal_visuals")]
    draw_terminal_glyphs_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    #[cfg(feature = "terminal_visuals")]
    draw_terminal_substrate_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    #[cfg(feature = "terminal_visuals")]
    draw_terminal_blocks_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    #[cfg(feature = "terminal_visuals")]
    draw_terminal_overlay_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    quad_vertices: Retained<ProtocolObject<dyn MTLBuffer>>,
    quad_indices: Retained<ProtocolObject<dyn MTLBuffer>>,
    glyph_cache: GlyphCache<Retained<ProtocolObject<dyn MTLTexture>>>,
    texture_cache: TextureCache<Retained<ProtocolObject<dyn MTLTexture>>>,
}

/// A structure that manages rendering scenes using a particular hardware
/// device.
pub struct Renderer {
    resources: Resources,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
}

impl Renderer {
    pub fn new(
        device: &ProtocolObject<dyn MTLDevice>,
        color_pixel_format: MTLPixelFormat,
        glyph_config: rendering::GlyphConfig,
    ) -> Self {
        let library = if cfg!(feature = "enable-metal-frame-capture") {
            let temp_lib_path = std::env::temp_dir().join("shaders.metallib");
            WRITE_LIB_TO_FILE.call_once(|| {
                let mut file = File::create(&temp_lib_path).unwrap();
                file.write_all(METAL_LIB_BYTES).unwrap();
            });
            let path = NSString::from_str(temp_lib_path.to_str().unwrap());
            // `newLibraryWithURL:` is the non-deprecated replacement, but we
            // load the shader library from a file path here.
            #[allow(deprecated)]
            let library = device.newLibraryWithFile_error(&path).unwrap();
            library
        } else {
            let data = DispatchData::from_static_bytes(METAL_LIB_BYTES);
            device.newLibraryWithData_error(&data).unwrap()
        };

        let rect_vertex_shader = library
            .newFunctionWithName(&NSString::from_str("rect_vertex_shader"))
            .unwrap();
        let rect_fragment_shader = library
            .newFunctionWithName(&NSString::from_str("rect_fragment_shader"))
            .unwrap();
        let rect_pipeline = Self::create_pipeline(
            "Rects",
            color_pixel_format,
            &rect_vertex_shader,
            &rect_fragment_shader,
        );
        let draw_rects_pipeline_state = device
            .newRenderPipelineStateWithDescriptor_error(&rect_pipeline)
            .unwrap();

        let image_fragment_shader = library
            .newFunctionWithName(&NSString::from_str("image_fragment_shader"))
            .unwrap();
        let image_pipeline = Self::create_pipeline(
            "Images",
            color_pixel_format,
            &rect_vertex_shader,
            &image_fragment_shader,
        );
        let draw_images_pipeline_state = device
            .newRenderPipelineStateWithDescriptor_error(&image_pipeline)
            .unwrap();

        let glyph_vertex_shader = library
            .newFunctionWithName(&NSString::from_str("glyph_vertex_shader"))
            .unwrap();
        let glyph_fragment_shader = library
            .newFunctionWithName(&NSString::from_str("glyph_fragment_shader"))
            .unwrap();
        let glyph_pipeline = Self::create_pipeline(
            "Glyphs",
            color_pixel_format,
            &glyph_vertex_shader,
            &glyph_fragment_shader,
        );
        let draw_glyphs_pipeline_state = device
            .newRenderPipelineStateWithDescriptor_error(&glyph_pipeline)
            .unwrap();

        #[cfg(feature = "terminal_visuals")]
        let draw_terminal_glyphs_pipeline_state = {
            let vertex = library
                .newFunctionWithName(&NSString::from_str("terminal_glyph_vertex_shader"))
                .unwrap();
            let fragment = library
                .newFunctionWithName(&NSString::from_str("terminal_glyph_fragment_shader"))
                .unwrap();
            let pipeline =
                Self::create_pipeline("Terminal glyphs", color_pixel_format, &vertex, &fragment);
            device
                .newRenderPipelineStateWithDescriptor_error(&pipeline)
                .unwrap()
        };

        #[cfg(feature = "terminal_visuals")]
        let draw_terminal_substrate_pipeline_state = {
            let vertex = library
                .newFunctionWithName(&NSString::from_str("terminal_substrate_vertex_shader"))
                .unwrap();
            let fragment = library
                .newFunctionWithName(&NSString::from_str("terminal_substrate_fragment_shader"))
                .unwrap();
            let pipeline =
                Self::create_pipeline("Terminal substrate", color_pixel_format, &vertex, &fragment);
            device
                .newRenderPipelineStateWithDescriptor_error(&pipeline)
                .unwrap()
        };

        #[cfg(feature = "terminal_visuals")]
        let draw_terminal_blocks_pipeline_state = {
            let vertex = library
                .newFunctionWithName(&NSString::from_str("terminal_block_vertex_shader"))
                .unwrap();
            let fragment = library
                .newFunctionWithName(&NSString::from_str("terminal_block_fragment_shader"))
                .unwrap();
            let pipeline = Self::create_pipeline(
                "Terminal command blocks",
                color_pixel_format,
                &vertex,
                &fragment,
            );
            device
                .newRenderPipelineStateWithDescriptor_error(&pipeline)
                .unwrap()
        };

        #[cfg(feature = "terminal_visuals")]
        let draw_terminal_overlay_pipeline_state = {
            let vertex = library
                .newFunctionWithName(&NSString::from_str("terminal_overlay_vertex_shader"))
                .unwrap();
            let fragment = library
                .newFunctionWithName(&NSString::from_str("terminal_overlay_fragment_shader"))
                .unwrap();
            let pipeline =
                Self::create_pipeline("Terminal overlays", color_pixel_format, &vertex, &fragment);
            device
                .newRenderPipelineStateWithDescriptor_error(&pipeline)
                .unwrap()
        };

        let quad_vertices = new_metal_buffer(
            device,
            &[
                shader::Vector2F::new(0., 0.),
                shader::Vector2F::new(1., 0.),
                shader::Vector2F::new(0., 1.),
                shader::Vector2F::new(1., 1.),
            ],
            MTLResourceOptions::StorageModeManaged,
        );

        let quad_indices = new_metal_buffer(
            device,
            &[0_u16, 1, 2, 2, 3, 1],
            MTLResourceOptions::StorageModeManaged,
        );

        let glyph_cache = GlyphCache::new(glyph_config);

        Self {
            resources: Resources {
                draw_rects_pipeline_state,
                draw_images_pipeline_state,
                draw_glyphs_pipeline_state,
                #[cfg(feature = "terminal_visuals")]
                draw_terminal_glyphs_pipeline_state,
                #[cfg(feature = "terminal_visuals")]
                draw_terminal_substrate_pipeline_state,
                #[cfg(feature = "terminal_visuals")]
                draw_terminal_blocks_pipeline_state,
                #[cfg(feature = "terminal_visuals")]
                draw_terminal_overlay_pipeline_state,
                quad_vertices,
                quad_indices,
                glyph_cache,
                texture_cache: TextureCache::new(),
            },
            command_queue: device
                .newCommandQueue()
                .expect("device should always vend a command queue"),
        }
    }

    fn create_pipeline(
        label: &str,
        color_pixel_format: MTLPixelFormat,
        vertex_shader: &ProtocolObject<dyn MTLFunction>,
        fragment_shader: &ProtocolObject<dyn MTLFunction>,
    ) -> Retained<MTLRenderPipelineDescriptor> {
        let pipeline = MTLRenderPipelineDescriptor::new();
        pipeline.setLabel(Some(&NSString::from_str(label)));
        pipeline.setVertexFunction(Some(vertex_shader));
        pipeline.setFragmentFunction(Some(fragment_shader));

        // SAFETY: index 0 is always a valid color attachment slot for a render pipeline.
        let attachment = unsafe { pipeline.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setPixelFormat(color_pixel_format);
        attachment.setBlendingEnabled(true);
        attachment.setRgbBlendOperation(MTLBlendOperation::Add);
        attachment.setAlphaBlendOperation(MTLBlendOperation::Add);
        attachment.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);
        attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
        attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);

        pipeline
    }

    fn render(
        &mut self,
        scene: &Scene,
        ctx: &MetalDrawContext,
        should_capture: bool,
        presents_with_transaction: bool,
    ) -> Option<CapturedFrame> {
        self.resources
            .glyph_cache
            .update_config(&scene.rendering_config().glyphs);

        let render_pass = RenderPass::new(&self.command_queue, ctx.drawable);

        Frame::new(scene, &render_pass.encoder, &mut self.resources, ctx).draw();

        render_pass.finish_with_capture(
            ctx.drawable_size,
            should_capture,
            presents_with_transaction,
        )
    }
}

/// A struct that manages rendering a single frame: the encoding of a scene into
/// a set of GPU draw calls to rasterize the scene description into a bitmap
/// image.
pub struct Frame<'a> {
    scene: &'a Scene,
    command_encoder: &'a ProtocolObject<dyn MTLRenderCommandEncoder>,
    resources: &'a mut Resources,
    ctx: &'a MetalDrawContext<'a>,
}

impl<'a> Frame<'a> {
    fn new(
        scene: &'a Scene,
        command_encoder: &'a ProtocolObject<dyn MTLRenderCommandEncoder>,
        resources: &'a mut Resources,
        ctx: &'a MetalDrawContext<'a>,
    ) -> Self {
        Self {
            scene,
            resources,
            command_encoder,
            ctx,
        }
    }

    fn draw(&mut self) {
        self.command_encoder.setViewport(MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: self.ctx.drawable_size.x() as f64,
            height: self.ctx.drawable_size.y() as f64,
            znear: 0.0,
            zfar: 1.0,
        });

        for layer in self.scene.layers() {
            if let Some(bounds) = layer.clip_bounds {
                // Make sure the scissor rect doesn't extend beyond the boundaries
                // of the window, as required by the Metal API.
                // API docs: https://developer.apple.com/documentation/metal/mtlrendercommandencoder/1515583-setscissorrect?language=objc
                // Scissor test background reading: https://developer.mozilla.org/en-US/docs/Web/API/WebGL_API/By_example/Basic_scissoring
                let device_bounds = RectF::new(Vector2F::zero(), self.ctx.drawable_size);
                let bounds = (bounds * self.scene.scale_factor()).intersection(device_bounds);
                if let Some(intersection) = bounds {
                    self.command_encoder.setScissorRect(MTLScissorRect {
                        x: intersection.origin_x().round() as usize,
                        y: intersection.origin_y().round() as usize,
                        width: intersection.width().round() as usize,
                        height: intersection.height().round() as usize,
                    });
                } else {
                    // The layer's clip bounds don't intersect the window bounds
                    // at all; we can skip drawing anything in this layer.
                    continue;
                }
            } else {
                self.command_encoder.setScissorRect(MTLScissorRect {
                    x: 0_usize,
                    y: 0_usize,
                    width: self.ctx.drawable_size.x() as usize,
                    height: self.ctx.drawable_size.y() as usize,
                });
            }
            self.draw_rects(layer);
            #[cfg(feature = "terminal_visuals")]
            self.draw_terminal_substrate(layer);
            #[cfg(feature = "terminal_visuals")]
            self.draw_terminal_blocks(layer);
            self.draw_images(layer);
            self.draw_glyphs(layer);
            #[cfg(feature = "terminal_visuals")]
            self.draw_terminal_overlays(layer);
        }
    }

    // Utility function to render image or icon in Metal.
    fn render_image_or_icon(&mut self, image: Option<&Image>, icon: Option<&Icon>) {
        let opacity;
        let bounds;
        let asset;
        let is_icon;
        let icon_color;
        let ui_corner_radius;

        if let Some(to_render) = image {
            opacity = to_render.opacity;
            bounds = to_render.bounds;
            asset = &to_render.asset;
            is_icon = false;
            icon_color = ColorF::new(0.0, 0.0, 0.0, opacity).into();
            ui_corner_radius = to_render.corner_radius;
        } else {
            let to_render = icon.unwrap();
            opacity = to_render.opacity;
            bounds = to_render.bounds;
            asset = &to_render.asset;
            is_icon = true;
            icon_color = to_render.color.to_f32().into();
            ui_corner_radius = CornerRadius::default();
        }

        let mut per_rect_uniforms = Vec::new();
        let scale_factor = self.scene.scale_factor();
        let bounds = bounds * scale_factor;
        let min_dimension = f32::min(bounds.height(), bounds.width());
        let corner_radius = crate::rendering::CornerRadius::from_ui_corner_radius(
            ui_corner_radius,
            scale_factor,
            min_dimension,
        );
        per_rect_uniforms.push(shader::PerRectUniforms::new(
            bounds.origin().into(),
            bounds.size().into(),
            corner_radius,
            0.,
            0.,
            0.,
            0.,
            vec2f(0.0, 0.0).into(),
            vec2f(1.0, 0.0).into(),
            ColorF::new(0.0, 0.0, 0.0, opacity).into(),
            ColorF::new(0.0, 0.0, 0.0, opacity).into(),
            vec2f(0.0, 0.0).into(),
            vec2f(1.0, 0.0).into(),
            ColorU::transparent_black().to_f32().into(),
            ColorU::transparent_black().to_f32().into(),
            is_icon,
            icon_color,
            Vector2F::zero().into(),
            ColorU::transparent_black().to_f32().into(),
            0_f32,
            0_f32,
            0.,
            vec2f(0.0, 0.0).into(),
        ));
        let per_rect_uniforms_buffer = new_metal_buffer(
            self.ctx.device,
            &per_rect_uniforms,
            MTLResourceOptions::StorageModeManaged,
        );

        let uniforms = shader::Uniforms::new(self.ctx.drawable_size.into());
        let uniforms_ptr = NonNull::from(&uniforms).cast::<c_void>();
        let uniforms_len = mem::size_of::<shader::Uniforms>();

        // SAFETY: the per-rect uniform buffer and `uniforms` value outlive this encoded draw
        // call, and the bound buffer/byte sizes and indices match the shader bindings.
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&per_rect_uniforms_buffer),
                0,
                1,
            );
            self.command_encoder
                .setVertexBytes_length_atIndex(uniforms_ptr, uniforms_len, 2);
            self.command_encoder
                .setFragmentBytes_length_atIndex(uniforms_ptr, uniforms_len, 0);
        }

        let (_, texture) = self
            .resources
            .texture_cache
            .get_or_insert_by_asset(asset, |asset| {
                let width = asset.size().x() as usize;
                let height = asset.size().y() as usize;

                let texture_descriptor = MTLTextureDescriptor::new();
                #[cfg(feature = "external_surfaces")]
                if let Some(image) = asset.io_surface()
                    && let Some(surface) = IOSurfaceRef::lookup(image.surface_id)
                    && surface.seed() >= image.seed
                {
                    texture_descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
                    texture_descriptor.setUsage(MTLTextureUsage::ShaderRead);
                    unsafe {
                        texture_descriptor.setWidth(width);
                        texture_descriptor.setHeight(height);
                    }
                    if let Some(texture) = self.ctx.device.newTextureWithDescriptor_iosurface_plane(
                        &texture_descriptor,
                        &surface,
                        0,
                    ) {
                        return texture;
                    }
                }
                texture_descriptor.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
                // SAFETY: width/height come from a decoded asset and are within Metal limits.
                unsafe {
                    texture_descriptor.setWidth(width);
                    texture_descriptor.setHeight(height);
                }
                let texture = self
                    .ctx
                    .device
                    .newTextureWithDescriptor(&texture_descriptor)
                    .expect("device should create an RGBA8 texture");
                let region = MTLRegion {
                    origin: MTLOrigin { x: 0, y: 0, z: 0 },
                    size: MTLSize {
                        width,
                        height,
                        depth: 1,
                    },
                };

                let bytes_per_row: usize = 4 * width;
                // SAFETY: rgba_bytes holds width*height*4 bytes laid out to match the region
                // and row stride.
                unsafe {
                    texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                        region,
                        0,
                        NonNull::new(asset.rgba_bytes().as_ptr() as *mut c_void)
                            .expect("asset rgba bytes pointer is non-null"),
                        bytes_per_row,
                    );
                }

                texture
            });

        // SAFETY: the bound texture and quad index buffer outlive this encoded draw call.
        unsafe {
            self.command_encoder
                .setFragmentTexture_atIndex(Some(&**texture), 0);

            self.command_encoder
                .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                    MTLPrimitiveType::Triangle,
                    6,
                    MTLIndexType::UInt16,
                    &self.resources.quad_indices,
                    0,
                    per_rect_uniforms.len(),
                );
        }
    }

    fn draw_images(&mut self, layer: &Layer) {
        if layer.images.is_empty() && layer.icons.is_empty() {
            // It's a mac assertion error to create an empty metal buffer, so exit early
            return;
        }

        self.command_encoder
            .setRenderPipelineState(&self.resources.draw_images_pipeline_state);
        // SAFETY: index 0 binds the shared quad vertex buffer, which outlives the draw calls.
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&self.resources.quad_vertices),
                0,
                0,
            );
        }

        for image in &layer.images {
            self.render_image_or_icon(Some(image), None);
        }

        // Another iteration for rendering icons.
        for icon in &layer.icons {
            self.render_image_or_icon(None, Some(icon));
        }
    }

    fn draw_rects(&self, layer: &Layer) {
        if layer.rects.is_empty() {
            // It's a mac assertion error to create an empty metal buffer, so exit early
            return;
        }

        self.command_encoder
            .setRenderPipelineState(&self.resources.draw_rects_pipeline_state);
        // SAFETY: index 0 binds the shared quad vertex buffer, which outlives the draw call.
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&self.resources.quad_vertices),
                0,
                0,
            );
        }

        let mut per_rect_uniforms = Vec::new();
        for rect in &layer.rects {
            let scale_factor = self.scene.scale_factor();
            let bounds = rect.bounds * scale_factor;

            let dash = rect
                .border
                .dash
                .map(|mut dash| {
                    dash.dash_length *= scale_factor;
                    dash.gap_length *= scale_factor;
                    dash
                })
                .unwrap_or_default();
            let horizontal_gap = get_best_dash_gap(bounds.width(), dash);
            let vertical_gap = get_best_dash_gap(bounds.height(), dash);
            let dash_length = dash.dash_length;
            let gap_lengths = Vector2F::new(horizontal_gap, vertical_gap);

            if let Some(drop_shadow) = rect.drop_shadow {
                let sigma = drop_shadow.blur_radius;
                let padding = drop_shadow.spread_radius * self.scene.scale_factor();
                let shadow_origin =
                    bounds.origin() + drop_shadow.offset * self.scene.scale_factor() - padding;
                let shadow_size = bounds.size() + vec2f(2. * padding, 2. * padding);

                let min_dimension = f32::min(shadow_size.x(), shadow_size.y());

                let corner_radius = crate::rendering::CornerRadius::from_ui_corner_radius(
                    rect.corner_radius,
                    scale_factor,
                    min_dimension,
                );

                // For the drop shadow case, we pass in a rect with the bounds
                // of the shadow and render that before rendering the actual rect.
                per_rect_uniforms.push(shader::PerRectUniforms::new(
                    shadow_origin.into(),
                    shadow_size.into(),
                    corner_radius,
                    0_f32,
                    0_f32,
                    0_f32,
                    0_f32,
                    Vector2F::zero().into(),
                    Vector2F::zero().into(),
                    ColorU::transparent_black().to_f32().into(),
                    ColorU::transparent_black().to_f32().into(),
                    Vector2F::zero().into(),
                    Vector2F::zero().into(),
                    ColorU::transparent_black().to_f32().into(),
                    ColorU::transparent_black().to_f32().into(),
                    false,
                    ColorU::transparent_black().to_f32().into(),
                    (drop_shadow.offset * self.scene.scale_factor()).into(),
                    drop_shadow.color.to_f32().into(),
                    sigma * self.scene.scale_factor(),
                    padding,
                    dash_length,
                    gap_lengths.into(),
                ));
            }

            let min_dimension = f32::min(bounds.height(), bounds.width());
            let corner_radius = crate::rendering::CornerRadius::from_ui_corner_radius(
                rect.corner_radius,
                scale_factor,
                min_dimension,
            );

            per_rect_uniforms.push(shader::PerRectUniforms::new(
                bounds.origin().into(),
                bounds.size().into(),
                corner_radius,
                rect.border.top_width() * scale_factor,
                rect.border.right_width() * scale_factor,
                rect.border.bottom_width() * scale_factor,
                rect.border.left_width() * scale_factor,
                rect.background.start().into(),
                rect.background.end().into(),
                rect.background.start_color().to_f32().into(),
                rect.background.end_color().to_f32().into(),
                rect.border.color.start().into(),
                rect.border.color.end().into(),
                rect.border.color.start_color().to_f32().into(),
                rect.border.color.end_color().to_f32().into(),
                false,
                ColorU::transparent_black().to_f32().into(),
                Vector2F::zero().into(),
                ColorU::transparent_black().to_f32().into(),
                0_f32,
                0_f32,
                dash_length,
                gap_lengths.into(),
            ));
        }
        let per_rect_uniforms_buffer = new_metal_buffer(
            self.ctx.device,
            &per_rect_uniforms,
            MTLResourceOptions::StorageModeManaged,
        );

        let uniforms = shader::Uniforms::new(self.ctx.drawable_size.into());
        let uniforms_ptr = NonNull::from(&uniforms).cast::<c_void>();
        let uniforms_len = mem::size_of::<shader::Uniforms>();

        // SAFETY: the per-rect uniform buffer and `uniforms` value outlive this encoded draw
        // call, and the bound buffer/byte sizes and indices match the shader bindings.
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&per_rect_uniforms_buffer),
                0,
                1,
            );
            self.command_encoder
                .setVertexBytes_length_atIndex(uniforms_ptr, uniforms_len, 2);
            self.command_encoder
                .setFragmentBytes_length_atIndex(uniforms_ptr, uniforms_len, 0);

            self.command_encoder
                .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                    MTLPrimitiveType::Triangle,
                    6,
                    MTLIndexType::UInt16,
                    &self.resources.quad_indices,
                    0,
                    per_rect_uniforms.len(),
                );
        }
    }

    fn draw_glyphs(&mut self, layer: &Layer) {
        #[cfg(feature = "terminal_visuals")]
        if let Some(visuals) = layer.terminal_visuals.as_deref() {
            self.draw_terminal_glyphs(layer, visuals);
            return;
        }

        if layer.glyphs.is_empty() {
            // It's a mac assertion error to create an empty metal buffer, so exit early
            return;
        }

        self.command_encoder
            .setRenderPipelineState(&self.resources.draw_glyphs_pipeline_state);
        // SAFETY: index 0 binds the shared quad vertex buffer, which outlives the draw calls.
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&self.resources.quad_vertices),
                0,
                0,
            );
        }

        let scale_factor = self.scene.scale_factor();

        let mut texture_to_glyph: HashMap<TextureId, Vec<shader::PerGlyphUniforms>> =
            HashMap::new();
        for glyph in &layer.glyphs {
            let glyph_position = glyph.position * scale_factor;
            let subpixel_alignment = SubpixelAlignment::new(glyph_position);

            match self.resources.glyph_cache.get(
                glyph.glyph_key,
                self.scene.scale_factor(),
                subpixel_alignment,
                &|atlas_size| create_new_texture_atlas(atlas_size, self.ctx.device),
                &insert_glyph_into_texture,
                &|glyph_key, scale, alignment| {
                    self.ctx.glyph_raster_bounds(glyph_key, scale, alignment)
                },
                &|glyph_key, scale, subpixel_alignment, glyph_config, format| {
                    self.ctx.rasterize_glyph(
                        glyph_key,
                        scale,
                        subpixel_alignment,
                        glyph_config,
                        format,
                    )
                },
            ) {
                Ok(Some(gto)) => {
                    let (fade_start, fade_end) = match &glyph.fade {
                        None => (&0.0, &-1.0),
                        Some(GlyphFade::Horizontal { start, end }) => (start, end),
                    };

                    // Adjust the horizontal position by the subpixel alignment
                    // so that we only shift the glyph over by the amount that
                    // isn't accounted for in the subpixel-rasterized glyph.
                    let glyph_position = glyph_position - subpixel_alignment.to_offset();

                    // Make sure to pass the glyph size in the atlas
                    // Not the size of the render bounds (which may be smaller)
                    // If you pass the render bounds as the size, the shader
                    // will try to sample from a smaller area than the size
                    // in the atlas, leading to artifacts.
                    let uv_region = gto.allocated_region.uv_region;
                    let uniform = shader::PerGlyphUniforms::new(
                        (glyph_position + gto.raster_bounds.origin()).into(),
                        gto.allocated_region.pixel_region.size().to_f32().into(),
                        uv_region.origin_x(),
                        uv_region.origin_y(),
                        uv_region.width(),
                        uv_region.height(),
                        fade_start * scale_factor,
                        fade_end * scale_factor,
                        glyph.color.to_f32().into(),
                        gto.is_emoji,
                    );

                    if let Some(per_glyph_uniforms) = texture_to_glyph.get_mut(&gto.texture_id) {
                        per_glyph_uniforms.push(uniform);
                    } else {
                        texture_to_glyph.insert(gto.texture_id, vec![uniform]);
                    }
                }
                Ok(None) => {}
                Err(_) => {
                    report_error!(
                        "Unable to get glyph out of glyph cache",
                        extra: { "glyph" => ?glyph }
                    );
                    return;
                }
            }
        }

        if texture_to_glyph.is_empty() {
            // Early exit if there are no glyphs to render, as it causes a debug assert
            // failure in the metal code to create an empty metal buffer.
            return;
        }

        for (texture_id, per_glyph_uniforms) in texture_to_glyph {
            let per_glyph_uniforms_buffer = new_metal_buffer(
                self.ctx.device,
                &per_glyph_uniforms,
                MTLResourceOptions::StorageModeManaged,
            );

            let uniforms = shader::Uniforms::new(self.ctx.drawable_size.into());
            let uniforms_ptr = NonNull::from(&uniforms).cast::<c_void>();
            let uniforms_len = mem::size_of::<shader::Uniforms>();

            let texture = self
                .resources
                .glyph_cache
                .texture(&texture_id)
                .expect("texture ID should be in atlas");

            // SAFETY: the per-glyph uniform buffer, `uniforms` value, bound texture, and quad
            // index buffer outlive this encoded draw call, and the bound sizes/indices match the
            // shader bindings.
            unsafe {
                self.command_encoder.setVertexBuffer_offset_atIndex(
                    Some(&per_glyph_uniforms_buffer),
                    0,
                    1,
                );
                self.command_encoder
                    .setVertexBytes_length_atIndex(uniforms_ptr, uniforms_len, 2);
                self.command_encoder
                    .setFragmentTexture_atIndex(Some(&**texture), 0);
                self.command_encoder
                    .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                        MTLPrimitiveType::Triangle,
                        6,
                        MTLIndexType::UInt16,
                        &self.resources.quad_indices,
                        0,
                        per_glyph_uniforms.len(),
                    );
            }
        }
    }

    #[cfg(feature = "terminal_visuals")]
    fn draw_terminal_glyphs(&mut self, layer: &Layer, visuals: &TerminalVisualLayer) {
        if layer.glyphs.is_empty() {
            return;
        }
        self.command_encoder
            .setRenderPipelineState(&self.resources.draw_terminal_glyphs_pipeline_state);
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&self.resources.quad_vertices),
                0,
                0,
            );
        }

        let scale_factor = self.scene.scale_factor();
        let mut texture_to_glyph: HashMap<TextureId, Vec<shader::TerminalPerGlyphUniforms>> =
            HashMap::new();
        for glyph in &layer.glyphs {
            let glyph_position = glyph.position * scale_factor;
            let subpixel_alignment = SubpixelAlignment::new(glyph_position);
            let cached = self.resources.glyph_cache.get(
                glyph.glyph_key,
                scale_factor,
                subpixel_alignment,
                &|atlas_size| create_new_texture_atlas(atlas_size, self.ctx.device),
                &insert_glyph_into_texture,
                &|glyph_key, scale, alignment| {
                    self.ctx.glyph_raster_bounds(glyph_key, scale, alignment)
                },
                &|glyph_key, scale, alignment, glyph_config, format| {
                    self.ctx
                        .rasterize_glyph(glyph_key, scale, alignment, glyph_config, format)
                },
            );
            let gto = match cached {
                Ok(Some(gto)) => gto,
                Ok(None) => continue,
                Err(_) => {
                    report_error!(
                        "Unable to get terminal glyph out of glyph cache",
                        extra: { "glyph" => ?glyph }
                    );
                    return;
                }
            };

            let (fade_start, fade_end) = match &glyph.fade {
                None => (&0.0, &-1.0),
                Some(GlyphFade::Horizontal { start, end }) => (start, end),
            };
            let glyph_position = glyph_position - subpixel_alignment.to_offset();
            let uv = gto.allocated_region.uv_region;
            let motion = glyph
                .terminal_cell
                .and_then(|cell| visuals.cell(cell))
                .copied()
                .unwrap_or_else(|| warpui_core::terminal_visuals::TerminalCellMotion {
                    current_origin: glyph.position,
                    previous_origin: glyph.position,
                    effect_kind: warpui_core::terminal_visuals::TERMINAL_EFFECT_OFF,
                    ..Default::default()
                });
            let uniform = shader::TerminalPerGlyphUniforms::new(
                (glyph_position + gto.raster_bounds.origin()).into(),
                gto.allocated_region.pixel_region.size().to_f32().into(),
                uv.origin_x(),
                uv.origin_y(),
                uv.width(),
                uv.height(),
                fade_start * scale_factor,
                fade_end * scale_factor,
                glyph.color.to_f32().into(),
                gto.is_emoji,
                (motion.current_origin * scale_factor).into(),
                (motion.previous_origin * scale_factor).into(),
                motion.start_seconds,
                motion.effect_kind,
                motion.seed,
                motion.flags,
            );
            texture_to_glyph
                .entry(gto.texture_id)
                .or_default()
                .push(uniform);
        }

        let layer_uniforms = shader::TerminalLayerUniforms::new(
            self.ctx.drawable_size.into(),
            visuals,
            scale_factor,
        );
        let layer_ptr = NonNull::from(&layer_uniforms).cast::<c_void>();
        let layer_len = mem::size_of::<shader::TerminalLayerUniforms>();
        for (texture_id, glyphs) in texture_to_glyph {
            if glyphs.is_empty() {
                continue;
            }
            let glyph_buffer = new_metal_buffer(
                self.ctx.device,
                &glyphs,
                MTLResourceOptions::StorageModeManaged,
            );
            let texture = self
                .resources
                .glyph_cache
                .texture(&texture_id)
                .expect("texture ID should be in atlas");
            unsafe {
                self.command_encoder
                    .setVertexBuffer_offset_atIndex(Some(&glyph_buffer), 0, 1);
                self.command_encoder
                    .setVertexBytes_length_atIndex(layer_ptr, layer_len, 2);
                self.command_encoder
                    .setFragmentTexture_atIndex(Some(&**texture), 0);
                self.command_encoder
                    .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                        MTLPrimitiveType::Triangle,
                        6,
                        MTLIndexType::UInt16,
                        &self.resources.quad_indices,
                        0,
                        glyphs.len(),
                    );
            }
        }
    }

    #[cfg(feature = "terminal_visuals")]
    fn draw_terminal_substrate(&mut self, layer: &Layer) {
        let Some(visuals) = layer.terminal_visuals.as_deref() else {
            return;
        };
        if !visuals.material.substrate_enabled {
            return;
        }
        self.command_encoder
            .setRenderPipelineState(&self.resources.draw_terminal_substrate_pipeline_state);
        let uniforms = shader::TerminalLayerUniforms::new(
            self.ctx.drawable_size.into(),
            visuals,
            self.scene.scale_factor(),
        );
        let ptr = NonNull::from(&uniforms).cast::<c_void>();
        let len = mem::size_of::<shader::TerminalLayerUniforms>();
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&self.resources.quad_vertices),
                0,
                0,
            );
            self.command_encoder
                .setVertexBytes_length_atIndex(ptr, len, 1);
            self.command_encoder
                .setFragmentBytes_length_atIndex(ptr, len, 0);
            self.command_encoder
                .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                    MTLPrimitiveType::Triangle,
                    6,
                    MTLIndexType::UInt16,
                    &self.resources.quad_indices,
                    0,
                    1,
                );
        }
    }

    #[cfg(feature = "terminal_visuals")]
    fn draw_terminal_blocks(&mut self, layer: &Layer) {
        let Some(visuals) = layer.terminal_visuals.as_deref() else {
            return;
        };
        if visuals.alternate_screen
            || visuals.blocks.is_empty()
            || (!visuals.material.command_gutter && !visuals.material.block_depth)
        {
            return;
        }
        let scale = self.scene.scale_factor();
        let blocks = visuals
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                shader::TerminalBlockUniforms::new(
                    (block.bounds.origin() * scale).into(),
                    (block.bounds.size() * scale).into(),
                    block.start_seconds,
                    block.effect_kind,
                    block.phase,
                    index as f32 * 0.173,
                )
            })
            .collect::<Vec<_>>();
        let block_buffer = new_metal_buffer(
            self.ctx.device,
            &blocks,
            MTLResourceOptions::StorageModeManaged,
        );
        let layer_uniforms =
            shader::TerminalLayerUniforms::new(self.ctx.drawable_size.into(), visuals, scale);
        let ptr = NonNull::from(&layer_uniforms).cast::<c_void>();
        let len = mem::size_of::<shader::TerminalLayerUniforms>();
        self.command_encoder
            .setRenderPipelineState(&self.resources.draw_terminal_blocks_pipeline_state);
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&self.resources.quad_vertices),
                0,
                0,
            );
            self.command_encoder
                .setVertexBuffer_offset_atIndex(Some(&block_buffer), 0, 1);
            self.command_encoder
                .setVertexBytes_length_atIndex(ptr, len, 2);
            self.command_encoder
                .setFragmentBytes_length_atIndex(ptr, len, 0);
            self.command_encoder
                .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                    MTLPrimitiveType::Triangle,
                    6,
                    MTLIndexType::UInt16,
                    &self.resources.quad_indices,
                    0,
                    blocks.len(),
                );
        }
    }

    #[cfg(feature = "terminal_visuals")]
    fn draw_terminal_overlays(&mut self, layer: &Layer) {
        let Some(visuals) = layer.terminal_visuals.as_deref() else {
            return;
        };
        if !visuals.is_active() && visuals.cursor.is_none() {
            return;
        }
        let scale = self.scene.scale_factor();
        let mut overlays = visuals
            .particles
            .iter()
            .map(|particle| {
                shader::TerminalOverlayUniforms::new(
                    (particle.origin * scale).into(),
                    (particle.velocity * scale).into(),
                    (particle.size * scale).into(),
                    particle.start_seconds,
                    particle.seed,
                    particle.effect_kind,
                    0,
                    particle.color.into(),
                )
            })
            .collect::<Vec<_>>();
        if let Some(cursor) = visuals.cursor {
            overlays.push(shader::TerminalOverlayUniforms::new(
                (cursor.from * scale).into(),
                ((cursor.to - cursor.from) * scale).into(),
                (cursor.size * scale).into(),
                cursor.start_seconds,
                cursor.duration_scale,
                cursor.effect_kind,
                1,
                cursor.color.into(),
            ));
        }
        if overlays.is_empty() {
            return;
        }
        self.command_encoder
            .setRenderPipelineState(&self.resources.draw_terminal_overlay_pipeline_state);
        let overlay_buffer = new_metal_buffer(
            self.ctx.device,
            &overlays,
            MTLResourceOptions::StorageModeManaged,
        );
        let layer_uniforms =
            shader::TerminalLayerUniforms::new(self.ctx.drawable_size.into(), visuals, scale);
        let ptr = NonNull::from(&layer_uniforms).cast::<c_void>();
        let len = mem::size_of::<shader::TerminalLayerUniforms>();
        unsafe {
            self.command_encoder.setVertexBuffer_offset_atIndex(
                Some(&self.resources.quad_vertices),
                0,
                0,
            );
            self.command_encoder
                .setVertexBuffer_offset_atIndex(Some(&overlay_buffer), 0, 1);
            self.command_encoder
                .setVertexBytes_length_atIndex(ptr, len, 2);
            self.command_encoder
                .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset_instanceCount(
                    MTLPrimitiveType::Triangle,
                    6,
                    MTLIndexType::UInt16,
                    &self.resources.quad_indices,
                    0,
                    overlays.len(),
                );
        }
    }
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        self.resources.texture_cache.end_frame();
    }
}

fn new_metal_buffer<T>(
    device: &ProtocolObject<dyn MTLDevice>,
    data: &[T],
    options: MTLResourceOptions,
) -> Retained<ProtocolObject<dyn MTLBuffer>> {
    // SAFETY: `data` points to `size_of_val(data)` initialized bytes; Metal copies them into the
    // new buffer, so the pointer only needs to be valid for the duration of this call.
    unsafe {
        device.newBufferWithBytes_length_options(
            NonNull::new(data.as_ptr() as *mut c_void).expect("buffer data pointer is non-null"),
            std::mem::size_of_val(data),
            options,
        )
    }
    .expect("device should create a buffer")
}

mod shader {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    // Temporarily silence the warning coming from https://github.com/rust-lang/rust-bindgen/issues/1651
    #![allow(unknown_lints)]

    use pathfinder_color::ColorF;
    use pathfinder_geometry::vector::{
        Vector2F as PathfinderVector2F, Vector4F as PathfinderVector4F,
    };
    pub use shader_types::*;
    #[cfg(feature = "terminal_visuals")]
    use warpui_core::terminal_visuals::TerminalVisualLayer;

    mod shader_types {
        // Bindgen deferences null pointers in generated test code, see:
        // https://github.com/rust-lang/rust-bindgen/issues/1651
        #![allow(deref_nullptr)]
        include!(concat!(env!("OUT_DIR"), "/shader_types.rs"));
    }

    pub struct Vector2F(vector_float2);
    pub struct Vector4F(vector_float4);

    impl Vector2F {
        pub fn new(x: f32, y: f32) -> Self {
            let y = y.to_bits();
            let mut vec = (y as vector_float2) << 32;
            let x = x.to_bits();
            vec |= x as vector_float2;
            Self(vec)
        }
    }

    impl From<PathfinderVector2F> for Vector2F {
        fn from(vec: PathfinderVector2F) -> Self {
            Self::new(vec.x(), vec.y())
        }
    }

    impl Vector4F {
        pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
            let w = w.to_bits();
            let mut vec = w as vector_float4;
            vec <<= 32;
            let z = z.to_bits();
            vec |= z as vector_float4;
            vec <<= 32;
            let y = y.to_bits();
            vec |= y as vector_float4;
            vec <<= 32;
            let x = x.to_bits();
            vec |= x as vector_float4;
            Self(vec)
        }
    }

    impl From<PathfinderVector4F> for Vector4F {
        fn from(vec: PathfinderVector4F) -> Self {
            Self::new(vec.x(), vec.y(), vec.z(), vec.w())
        }
    }

    impl From<ColorF> for Vector4F {
        fn from(color: ColorF) -> Self {
            Self::new(color.r(), color.g(), color.b(), color.a())
        }
    }

    impl From<[f32; 4]> for Vector4F {
        fn from(color: [f32; 4]) -> Self {
            Self::new(color[0], color[1], color[2], color[3])
        }
    }

    impl PerRectUniforms {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            origin: Vector2F,
            size: Vector2F,
            corner_radius: crate::rendering::CornerRadius,
            border_top: f32,
            border_right: f32,
            border_bottom: f32,
            border_left: f32,
            background_start: Vector2F,
            background_end: Vector2F,
            background_start_color: Vector4F,
            background_end_color: Vector4F,
            border_start: Vector2F,
            border_end: Vector2F,
            border_start_color: Vector4F,
            border_end_color: Vector4F,
            is_icon: bool,
            icon_color: Vector4F,
            drop_shadow_offsets: Vector2F,
            drop_shadow_color: Vector4F,
            drop_shadow_sigma: f32,
            drop_shadow_padding_factor: f32,
            dash_length: f32,
            gap_lengths: Vector2F,
        ) -> Self {
            Self {
                origin: origin.0,
                size: size.0,
                corner_radius_top_left: corner_radius.top_left,
                corner_radius_top_right: corner_radius.top_right,
                corner_radius_bottom_left: corner_radius.bottom_left,
                corner_radius_bottom_right: corner_radius.bottom_right,
                border_top,
                border_right,
                border_bottom,
                border_left,
                background_start: background_start.0,
                background_end: background_end.0,
                background_start_color: background_start_color.0,
                background_end_color: background_end_color.0,
                border_start: border_start.0,
                border_end: border_end.0,
                border_start_color: border_start_color.0,
                border_end_color: border_end_color.0,
                is_icon: is_icon as i32,
                icon_color: icon_color.0,
                drop_shadow_offsets: drop_shadow_offsets.0,
                drop_shadow_color: drop_shadow_color.0,
                drop_shadow_sigma,
                drop_shadow_padding_factor,
                dash_length,
                gap_lengths: gap_lengths.0,
            }
        }
    }

    impl PerGlyphUniforms {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            origin: Vector2F,
            size: Vector2F,
            uv_left: f32,
            uv_top: f32,
            uv_width: f32,
            uv_height: f32,
            fade_start: f32,
            fade_end: f32,
            color: Vector4F,
            is_emoji: bool,
        ) -> Self {
            Self {
                origin: origin.0,
                size: size.0,
                color: color.0,
                uv_left,
                uv_top,
                uv_width,
                uv_height,
                fade_start,
                fade_end,
                is_emoji: is_emoji as i32,
                __bindgen_padding_0: Default::default(),
            }
        }
    }

    #[cfg(feature = "terminal_visuals")]
    impl TerminalPerGlyphUniforms {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            origin: Vector2F,
            size: Vector2F,
            uv_left: f32,
            uv_top: f32,
            uv_width: f32,
            uv_height: f32,
            fade_start: f32,
            fade_end: f32,
            color: Vector4F,
            is_emoji: bool,
            current_cell_origin: Vector2F,
            previous_cell_origin: Vector2F,
            start_seconds: f32,
            effect_kind: u32,
            seed: f32,
            flags: u32,
        ) -> Self {
            let mut value: Self = unsafe { std::mem::zeroed() };
            value.origin = origin.0;
            value.size = size.0;
            value.uv_left = uv_left;
            value.uv_top = uv_top;
            value.uv_width = uv_width;
            value.uv_height = uv_height;
            value.fade_start = fade_start;
            value.fade_end = fade_end;
            value.color = color.0;
            value.is_emoji = is_emoji as i32;
            value.current_cell_origin = current_cell_origin.0;
            value.previous_cell_origin = previous_cell_origin.0;
            value.start_seconds = start_seconds;
            value.effect_kind = effect_kind;
            value.seed = seed;
            value.flags = flags;
            value
        }
    }

    #[cfg(feature = "terminal_visuals")]
    impl TerminalLayerUniforms {
        pub fn new(viewport_size: Vector2F, visuals: &TerminalVisualLayer, scale: f32) -> Self {
            let mut flags = 0;
            flags |= u32::from(visuals.material.substrate_enabled);
            flags |= u32::from(visuals.material.command_gutter) << 1;
            flags |= u32::from(visuals.material.block_depth) << 2;
            let mut value: Self = unsafe { std::mem::zeroed() };
            value.viewport_size = viewport_size.0;
            value.viewport_origin = Vector2F::from(visuals.viewport.origin() * scale).0;
            value.terminal_viewport_size = Vector2F::from(visuals.viewport.size() * scale).0;
            value.cell_size = Vector2F::from(visuals.cell_size * scale).0;
            value.elapsed_seconds = visuals.elapsed_seconds;
            value.duration_seconds = visuals.duration_seconds;
            value.new_line_effect = visuals.profile.new_line;
            value.commit_effect = visuals.profile.commit;
            value.keypress_effect = visuals.profile.keypress;
            value.cursor_effect = visuals.profile.cursor;
            value.erase_effect = visuals.profile.erase;
            value.block_effect = visuals.profile.block_fold;
            value.material_style = visuals.material.style;
            value.material_flags = flags;
            value.particle_spread =
                if visuals.particle_spread.is_finite() && visuals.particle_spread > 0.0 {
                    visuals.particle_spread.clamp(0.5, 2.0)
                } else {
                    1.0
                };
            value.material_tint = Vector4F::from(visuals.material.tint).0;
            value.material_accent = Vector4F::from(visuals.material.accent).0;
            value.terminal_foreground = Vector4F::from(visuals.material.foreground).0;
            value
        }
    }

    #[cfg(feature = "terminal_visuals")]
    impl TerminalOverlayUniforms {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            origin: Vector2F,
            velocity: Vector2F,
            size: Vector2F,
            start_seconds: f32,
            seed: f32,
            effect_kind: u32,
            kind: u32,
            color: Vector4F,
        ) -> Self {
            let mut value: Self = unsafe { std::mem::zeroed() };
            value.origin = origin.0;
            value.velocity = velocity.0;
            value.size = size.0;
            value.start_seconds = start_seconds;
            value.seed = seed;
            value.effect_kind = effect_kind;
            value.kind = kind;
            value.color = color.0;
            value
        }
    }

    #[cfg(feature = "terminal_visuals")]
    impl TerminalBlockUniforms {
        pub fn new(
            origin: Vector2F,
            size: Vector2F,
            start_seconds: f32,
            effect_kind: u32,
            phase: f32,
            seed: f32,
        ) -> Self {
            let mut value: Self = unsafe { std::mem::zeroed() };
            value.origin = origin.0;
            value.size = size.0;
            value.start_seconds = start_seconds;
            value.effect_kind = effect_kind;
            value.phase = phase;
            value.seed = seed;
            value
        }
    }

    impl Uniforms {
        pub fn new(viewport_size: Vector2F) -> Self {
            Self {
                viewport_size: viewport_size.0,
            }
        }
    }
}

pub(super) struct MetalDrawContext<'a> {
    pub(super) device: &'a ProtocolObject<dyn MTLDevice>,
    pub(super) drawable: &'a ProtocolObject<dyn CAMetalDrawable>,
    pub(super) drawable_size: Vector2F,
    rasterize_glyph_fn: &'a RasterizeGlyphFn<'a>,
    glyph_raster_bounds_fn: &'a GlyphRasterBoundsFn<'a>,
}

impl MetalDrawContext<'_> {
    pub(super) fn rasterize_glyph(
        &self,
        glyph_key: GlyphKey,
        scale: Vector2F,
        subpixel_alignment: SubpixelAlignment,
        glyph_config: &rendering::GlyphConfig,
        format: canvas::RasterFormat,
    ) -> anyhow::Result<RasterizedGlyph> {
        (self.rasterize_glyph_fn)(glyph_key, scale, subpixel_alignment, glyph_config, format)
    }

    pub(super) fn glyph_raster_bounds(
        &self,
        glyph_key: GlyphKey,
        scale: Vector2F,
        glyph_config: &rendering::GlyphConfig,
    ) -> anyhow::Result<RectI> {
        (self.glyph_raster_bounds_fn)(glyph_key, scale, glyph_config)
    }
}

#[cfg(all(test, feature = "terminal_visuals"))]
mod terminal_shader_abi_tests {
    use std::mem::{align_of, size_of};

    use super::shader::{
        TerminalBlockUniforms, TerminalLayerUniforms, TerminalOverlayUniforms,
        TerminalPerGlyphUniforms,
    };

    #[test]
    fn terminal_shader_uniform_layouts_match_metal_alignment() {
        assert_eq!(size_of::<TerminalLayerUniforms>(), 128);
        assert_eq!(size_of::<TerminalPerGlyphUniforms>(), 112);
        assert_eq!(size_of::<TerminalOverlayUniforms>(), 64);
        assert_eq!(size_of::<TerminalBlockUniforms>(), 32);
        assert_eq!(align_of::<TerminalLayerUniforms>(), 16);
        assert_eq!(align_of::<TerminalPerGlyphUniforms>(), 16);
        assert_eq!(align_of::<TerminalOverlayUniforms>(), 16);
        assert_eq!(align_of::<TerminalBlockUniforms>(), 8);
    }
}

impl super::super::Renderer for Renderer {
    fn render(&mut self, scene: &Scene, window: &WindowState, font_cache: &fonts::Cache) {
        // SAFETY: `render` is called via `warp_update_layer`, which is only be invoked for
        // windows created via Window::open() and always sets a non-`None` device.
        #[allow(irrefutable_let_patterns)]
        let Device::Metal(metal_device) = window
            .device()
            .expect("render is only called for a window that has a real display")
        else {
            report_error!("Metal renderer called with non-metal device");
            return;
        };
        let metal_device: &ProtocolObject<dyn MTLDevice> = metal_device;

        let metal_layer = window.metal_layer();
        let presents_with_transaction = metal_layer.presentsWithTransaction();
        let drawable = metal_layer
            .nextDrawable()
            .expect("CAMetalLayer with allowsNextDrawableTimeout disabled always vends a drawable");

        let ctx = &MetalDrawContext {
            device: metal_device,
            drawable: &drawable,
            drawable_size: window.physical_size(),
            rasterize_glyph_fn: &|glyph_key, scale, subpixel_alignment, glyph_config, format| {
                font_cache.rasterized_glyph(
                    glyph_key,
                    scale,
                    subpixel_alignment,
                    glyph_config,
                    format,
                )
            },
            glyph_raster_bounds_fn: &|glyph_key, scale, alignment| {
                font_cache.glyph_raster_bounds(glyph_key, scale, alignment)
            },
        };

        let capture_callback = window.capture_callback.borrow_mut().take();
        let should_capture = capture_callback.is_some();
        let captured = Self::render(self, scene, ctx, should_capture, presents_with_transaction);
        if let (Some(frame), Some(callback)) = (captured, capture_callback) {
            callback(frame);
        }
    }

    fn resize(&mut self, _window: &WindowState) {
        // TODO(alokedesai): Backport the optimization to only set the size of surface when a
        // window is resized to the Metal renderer.
    }
}

/// Writes the bytes of the `glyph` into a region of the current texture identified by `region`.
fn insert_glyph_into_texture(
    region: AllocatedRegion,
    glyph: &RasterizedGlyph,
    texture: &mut Retained<ProtocolObject<dyn MTLTexture>>,
) {
    let region = MTLRegion {
        origin: MTLOrigin {
            x: region.pixel_region.origin_x() as usize,
            y: region.pixel_region.origin_y() as usize,
            z: 0,
        },
        size: MTLSize {
            width: region.pixel_region.width() as usize,
            height: region.pixel_region.height() as usize,
            depth: 1,
        },
    };

    let bytes_per_row: usize = 4 * (glyph.canvas.size.x() as usize);
    // SAFETY: the glyph canvas holds at least `bytes_per_row * region.height` bytes laid out to
    // match the destination region.
    unsafe {
        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            region,
            0,
            NonNull::new(glyph.canvas.pixels.as_slice().as_ptr() as *mut c_void)
                .expect("glyph canvas pixel pointer is non-null"),
            bytes_per_row,
        );
    }
}

/// Creates a new texture atlas for use in the cache.
fn create_new_texture_atlas(
    atlas_size: usize,
    device: &ProtocolObject<dyn MTLDevice>,
) -> Retained<ProtocolObject<dyn MTLTexture>> {
    let texture_descriptor = MTLTextureDescriptor::new();
    texture_descriptor.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
    // SAFETY: `atlas_size` is a fixed, valid texture dimension within Metal limits.
    unsafe {
        texture_descriptor.setWidth(atlas_size);
        texture_descriptor.setHeight(atlas_size);
    }
    device
        .newTextureWithDescriptor(&texture_descriptor)
        .expect("device should create an atlas texture")
}
