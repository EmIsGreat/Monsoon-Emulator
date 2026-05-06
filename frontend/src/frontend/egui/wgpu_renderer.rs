//! GPU-accelerated NES palette-index renderer using wgpu and WGSL shaders.
//!
//! This module provides [`NesWgpuRenderer`] and [`WgpuFrameCallback`], which
//! together replace the CPU-side palette lookup with a GPU shader.
//!
//! # Pipeline overview
//!
//! ```text
//! front_buffer: Vec<u16>  (9-bit palette indices from the PPU)
//!        │
//!        │  queue.write_texture()  [in WgpuFrameCallback::prepare]
//!        ▼
//! index_texture: R32Uint (256×240 texels, one u32 per pixel)
//!        │
//!        │  WGSL fragment shader: index → palette_texture lookup
//!        ▼
//! palette_texture: Rgba8Unorm (512×1 texels, one RGBA entry per index)
//!        │
//!        │  fragment shader output
//!        ▼
//! egui render pass framebuffer  (rendered directly into the UI rect)
//! ```
//!
//! # Integration
//!
//! * [`NesWgpuRenderer`] is inserted into `egui_wgpu`'s [`CallbackResources`]
//!   on startup so it lives for the lifetime of the egui renderer.
//! * [`WgpuFrameCallback`] is constructed once per UI frame (in
//!   `render_emulator_output`) carrying the current front-buffer and palette
//!   snapshot. Its [`prepare`](egui_wgpu::CallbackTrait::prepare) method
//!   uploads both to the GPU; its
//!   [`paint`](egui_wgpu::CallbackTrait::paint) method draws the result.

use std::sync::Arc;

use eframe::egui_wgpu;
use eframe::wgpu;
use monsoon_core::emulation::palette_util::RgbPalette;
use monsoon_core::emulation::ppu_util::{TOTAL_OUTPUT_HEIGHT, TOTAL_OUTPUT_WIDTH};

/// Number of entries in the flat NES palette LUT.
///
/// 512 = 64 base colours × 8 emphasis combinations (bits 6-8 of the pixel
/// value).
const PALETTE_LUT_SIZE: u32 = 512;

/// WGSL shader source: palette-index lookup rendered into an arbitrary rect.
///
/// Vertex shader: emits a clip-space full-screen triangle; the wgpu viewport is
/// set to the target rect by [`WgpuFrameCallback::paint`], so the triangle is
/// automatically clipped to the correct region.
///
/// Fragment shader:
/// 1. Maps the fragment's NDC position to a texel in the 256×240 index texture.
/// 2. Reads the 9-bit palette index stored as a `u32`.
/// 3. Looks up the corresponding RGBA entry in the 512×1 palette texture.
const SHADER_SRC: &str = r#"
// ---- Vertex shader -------------------------------------------------------

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0)       uv:       vec2<f32>,
};

/// Full-screen triangle (3 vertices cover all of NDC space).
/// When the render-pass viewport is restricted to the emulator rect the
/// triangle is implicitly clipped to that region.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var clip = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    let c = clip[vi];
    var out: VertexOutput;
    out.position = vec4<f32>(c, 0.0, 1.0);
    // UV: x in [0,1] left→right, y in [0,1] top→bottom
    out.uv = vec2<f32>((c.x + 1.0) * 0.5, (1.0 - c.y) * 0.5);
    return out;
}

// ---- Fragment shader -------------------------------------------------------

/// 256×240 texture of u32 palette indices (R32Uint format).
@group(0) @binding(0) var index_tex:   texture_2d<u32>;
/// 512×1 texture of pre-computed RGBA colours (Rgba8Unorm format).
@group(0) @binding(1) var palette_tex: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let size   = textureDimensions(index_tex);
    let px     = vec2<i32>(i32(in.uv.x * f32(size.x)),
                            i32(in.uv.y * f32(size.y)));
    // Clamp so out-of-range UVs (from the oversized triangle) read edge pixels.
    let px_clamped = clamp(px, vec2<i32>(0, 0),
                               vec2<i32>(i32(size.x) - 1, i32(size.y) - 1));
    let index  = textureLoad(index_tex, px_clamped, 0).r & 0x1FFu;
    let rgba   = textureLoad(palette_tex, vec2<i32>(i32(index), 0), 0);
    return vec4<f32>(rgba.rgb, 1.0);
}
"#;

/// All persistent wgpu resources needed to render the NES frame.
///
/// One instance lives inside `egui_wgpu::CallbackResources` for the lifetime
/// of the egui renderer. It is inserted on startup via
/// `render_state.renderer.write().callback_resources.insert(...)`.
pub struct NesWgpuRenderer {
    pipeline: wgpu::RenderPipeline,
    index_texture: wgpu::Texture,
    palette_texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl NesWgpuRenderer {
    /// Create all GPU resources.
    ///
    /// * `device` — wgpu device (from `RenderState`).
    /// * `queue` — wgpu queue for uploading the initial palette.
    /// * `target_format` — framebuffer format (from `RenderState`).
    /// * `initial_palette` — palette to load into the LUT texture immediately.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        initial_palette: &RgbPalette,
    ) -> Self {
        // ---- Shader module -----------------------------------------------
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("nes_palette_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        // ---- Bind-group layout -------------------------------------------
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("nes_renderer_bgl"),
                entries: &[
                    // binding 0: index texture (R32Uint, integer sampling)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // binding 1: palette texture (Rgba8Unorm, float sampling)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        // ---- Pipeline layout ---------------------------------------------
        // wgpu 29: bind_group_layouts takes &[Option<&BindGroupLayout>]; no
        // push_constant_ranges field (replaced by immediate_size).
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("nes_renderer_pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // ---- Render pipeline ---------------------------------------------
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("nes_palette_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[], // geometry generated in vertex shader
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            // wgpu 29: field is `multiview_mask` (not `multiview`)
            multiview_mask: None,
            cache: None,
        });

        // ---- Index texture (256×240, R32Uint) ----------------------------
        let index_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nes_index_tex"),
            size: wgpu::Extent3d {
                width: TOTAL_OUTPUT_WIDTH as u32,
                height: TOTAL_OUTPUT_HEIGHT as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let index_view = index_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // ---- Palette texture (512×1, Rgba8Unorm) -------------------------
        let palette_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nes_palette_tex"),
            size: wgpu::Extent3d {
                width: PALETTE_LUT_SIZE,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let palette_view = palette_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // ---- Bind group -------------------------------------------------
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nes_renderer_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&index_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&palette_view),
                },
            ],
        });

        let renderer = Self {
            pipeline,
            index_texture,
            palette_texture,
            bind_group,
        };

        // Upload initial palette LUT.
        renderer.update_palette(queue, initial_palette);

        renderer
    }

    /// Upload a new frame's palette-index buffer to the GPU index texture.
    ///
    /// Each `u16` is widened to `u32` for the `R32Uint` texture format.
    pub fn update_frame(&self, queue: &wgpu::Queue, buffer: &[u16]) {
        // Widen u16 → u32 (4 bytes per texel for R32Uint).
        let data: Vec<u32> = buffer.iter().map(|&v| v as u32).collect();
        let bytes = cast_u32_slice_to_u8(&data);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.index_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(TOTAL_OUTPUT_WIDTH as u32 * 4), // 4 bytes per R32Uint texel
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: TOTAL_OUTPUT_WIDTH as u32,
                height: TOTAL_OUTPUT_HEIGHT as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Upload a new palette LUT to the GPU palette texture.
    ///
    /// Builds the same flat mapping as `LookupPaletteRenderer`:
    /// `index = color_bits | (emphasis_bits << 6)` → `[R, G, B, 255]`.
    pub fn update_palette(&self, queue: &wgpu::Queue, palette: &RgbPalette) {
        let mut data = [0u8; PALETTE_LUT_SIZE as usize * 4];
        for emph in 0..8usize {
            for color in 0..64usize {
                let idx = color | (emph << 6);
                let rgb = palette.colors[emph][color];
                data[idx * 4] = rgb.r;
                data[idx * 4 + 1] = rgb.g;
                data[idx * 4 + 2] = rgb.b;
                data[idx * 4 + 3] = 255;
            }
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.palette_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(PALETTE_LUT_SIZE * 4), // 4 bytes per Rgba8Unorm texel
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: PALETTE_LUT_SIZE,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Draw the NES frame into `render_pass`, restricting the viewport to
    /// `rect_px` (given in physical screen pixels).
    pub fn paint_into(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        render_pass.set_viewport(x, y, w, h, 0.0, 1.0);
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1); // full-screen triangle
    }
}

// ---- egui_wgpu::CallbackTrait impl ----------------------------------------

/// Per-frame callback: carries the front-buffer and palette snapshots and
/// drives the GPU upload + draw.
///
/// Created fresh each UI frame in `render_emulator_output` and passed to
/// `painter.add(egui_wgpu::Callback::new_paint_callback(...))`.
pub struct WgpuFrameCallback {
    /// Snapshot of the front buffer (~122 KB; clone is negligible at 60 fps).
    pub frame: Arc<Vec<u16>>,
    /// Current palette — uploaded every frame (~2 KB; barely ever changes).
    pub palette: RgbPalette,
}

impl egui_wgpu::CallbackTrait for WgpuFrameCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = callback_resources.get::<NesWgpuRenderer>() {
            renderer.update_frame(queue, &self.frame);
            renderer.update_palette(queue, &self.palette);
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = callback_resources.get::<NesWgpuRenderer>() {
            let vp = info.viewport_in_pixels();
            // wgpu 29 / egui 0.34: ViewportInPixels uses width_px/height_px,
            // not right_px/bottom_px.
            renderer.paint_into(
                render_pass,
                vp.left_px as f32,
                vp.top_px as f32,
                vp.width_px as f32,
                vp.height_px as f32,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Byte-view helper: reinterpret &[u32] as &[u8] for queue.write_texture.
//
// SAFETY requirements (all statically guaranteed for u32):
//   • u8 has alignment 1, so any u32 pointer is also valid as a u8 pointer.
//   • u32 has no invalid bit patterns, so every byte of the slice is a valid u8.
//   • The resulting byte count (len * 4) cannot overflow usize on any platform
//     where len*4 is computed, since len is bounded by the texture dimensions
//     (256*240 = 61 440 elements → 245 760 bytes, well within usize::MAX).
// ---------------------------------------------------------------------------
fn cast_u32_slice_to_u8(data: &[u32]) -> &[u8] {
    let byte_len = data.len().checked_mul(std::mem::size_of::<u32>())
        .expect("byte length overflow in cast_u32_slice_to_u8");
    // SAFETY: see comment above; u8 alignment ≤ u32 alignment, all bytes valid.
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), byte_len) }
}
