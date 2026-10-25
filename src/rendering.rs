//! Automatic render density. Layout and pointer coordinates remain in window units.
use macroquad::prelude::*;

const QUALITY_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
uniform mat4 Model;
uniform mat4 Projection;
varying highp vec2 uv;
varying lowp vec4 color;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
    color = color0 / 255.0;
}"#;
const SCENE_FRAGMENT: &str = r#"#version 100
precision highp float;
varying highp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
void main() { gl_FragColor = color * texture2D(Texture, uv); }
"#;
const PRESENT_FRAGMENT: &str = r#"#version 100
precision highp float;
varying highp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
uniform vec2 SourceSize;
uniform vec2 OutputSize;
void main() {
    vec2 ratio = SourceSize / OutputSize;
    if (max(ratio.x, ratio.y) < 1.05) {
        gl_FragColor = color * texture2D(Texture, uv);
        return;
    }
    vec2 q = 0.25 / OutputSize;
    vec3 a = texture2D(Texture, uv + vec2(-q.x,-q.y)).rgb;
    vec3 b = texture2D(Texture, uv + vec2( q.x,-q.y)).rgb;
    vec3 c = texture2D(Texture, uv + vec2(-q.x, q.y)).rgb;
    vec3 d = texture2D(Texture, uv + vec2( q.x, q.y)).rgb;
    vec3 base = (a+b+c+d)*0.25;
    vec2 p = 1.0 / OutputSize;
    vec3 around = (texture2D(Texture,uv+vec2(p.x,0.0)).rgb
        + texture2D(Texture,uv-vec2(p.x,0.0)).rgb
        + texture2D(Texture,uv+vec2(0.0,p.y)).rgb
        + texture2D(Texture,uv-vec2(0.0,p.y)).rgb)*0.25;
    // Restrained local contrast, clamped to real source colors to avoid light/dark halos.
    vec3 sharp = clamp(base + 0.12*(base-around),min(min(a,b),min(c,d)),max(max(a,b),max(c,d)));
    gl_FragColor = color * vec4(sharp,1.0);
}"#;

fn quality_materials() -> (Option<Material>, Option<Material>) {
    use macroquad::miniquad::{
        Backend, BlendFactor, BlendState, BlendValue, Equation, PipelineParams,
    };
    if unsafe {
        macroquad::window::get_internal_gl()
            .quad_context
            .info()
            .backend
    } != Backend::OpenGl
    {
        return (None, None); // Metal's built-in path already uses full-precision texture coordinates.
    }
    let build = |fragment, uniforms| {
        load_material(
            ShaderSource::Glsl {
                vertex: QUALITY_VERTEX,
                fragment,
            },
            MaterialParams {
                uniforms,
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .map_err(|e| crate::diagnostics::record("quality-shader", &e.to_string()))
        .ok()
    };
    (
        build(SCENE_FRAGMENT, vec![]),
        build(
            PRESENT_FRAGMENT,
            vec![
                UniformDesc::new("SourceSize", UniformType::Float2),
                UniformDesc::new("OutputSize", UniformType::Float2),
            ],
        ),
    )
}

pub fn render_size(width: f32, height: f32, dpi: f32, max_texture: u32) -> (u32, u32) {
    let w = (width * dpi).round().max(1.);
    let h = (height * dpi).round().max(1.);
    let scale = (1920. / w.max(h)).max(1080. / w.min(h)).max(1.);
    let (w, h) = (w * scale, h * scale);
    // Respect the GPU limit and bound memory at roughly two 4K color buffers.
    let cap = (max_texture as f32 / w.max(h))
        .min((16_777_216. / (w * h)).sqrt())
        .min(1.);
    let round = |n: f32| ((n * cap).ceil() as u32).min(max_texture).max(1);
    (round(w), round(h))
}

/// Sustained pressure changes quality; short hitches and idle frame pacing do not.
#[derive(Default)]
pub struct RenderBudget {
    tier: u8,
    slow_seconds: f32,
    fast_seconds: f32,
}
impl RenderBudget {
    pub fn observe(&mut self, frame_ms: f32, dt: f32) {
        if !frame_ms.is_finite() || !dt.is_finite() {
            return;
        }
        let dt = dt.clamp(0., 0.1);
        if frame_ms > 23. {
            self.slow_seconds += dt;
            self.fast_seconds = 0.;
        } else if frame_ms < 15. {
            self.fast_seconds += dt;
            self.slow_seconds = 0.;
        } else {
            self.slow_seconds = 0.;
            self.fast_seconds = 0.;
        }
        if self.slow_seconds >= 2. && self.tier < 2 {
            self.tier += 1;
            self.slow_seconds = 0.;
        }
        if self.fast_seconds >= 10. && self.tier > 0 {
            self.tier -= 1;
            self.fast_seconds = 0.;
        }
    }
    pub fn size(
        &self,
        width: f32,
        height: f32,
        dpi: f32,
        max_texture: u32,
        mobile: bool,
    ) -> (u32, u32) {
        let (w, h) = render_size(width, height, dpi, max_texture);
        let native = (width.max(1.) * height.max(1.) * dpi.max(0.5).powi(2)).max(1.);
        let mobile_cap = if mobile {
            (native * 2.25 / (w as f32 * h as f32)).sqrt().min(1.)
        } else {
            1.
        };
        // Leave room for the previous target during resize, board/piece caches,
        // the window swapchain and font atlases instead of spending the budget on one FBO.
        let color_pixels = if mobile { 4_194_304. } else { 8_388_608. };
        let memory_cap = (color_pixels / (w as f32 * h as f32)).sqrt().min(1.);
        let scale = [1., 0.82, 0.67][usize::from(self.tier)] * mobile_cap.min(memory_cap);
        (
            (w as f32 * scale).round().max(1.) as u32,
            (h as f32 * scale).round().max(1.) as u32,
        )
    }
}

pub struct AdaptiveScene {
    pub target: RenderTarget,
    max_texture: u32,
    resolve: bool,
    smoke_size: Option<(u32, u32)>,
    scene_material: Option<Material>,
    present_material: Option<Material>,
    resize_candidate: Option<((u32, u32), std::time::Instant)>,
    pub budget: RenderBudget,
    completion_sample: u16,
    gpu_ms: Option<f32>,
}
impl Default for AdaptiveScene {
    fn default() -> Self {
        Self::new()
    }
}
impl AdaptiveScene {
    pub fn new() -> Self {
        let (max_texture, resolve) = unsafe {
            let gl = macroquad::window::get_internal_gl();
            let info = gl.quad_context.info();
            let mut max = 8192;
            if info.backend == macroquad::miniquad::Backend::OpenGl {
                macroquad::miniquad::gl::glGetIntegerv(
                    macroquad::miniquad::gl::GL_MAX_TEXTURE_SIZE,
                    &mut max,
                );
            }
            (max.max(1) as u32, info.features.resolve_attachments)
        };
        let budget = RenderBudget::default();
        let size = budget.size(
            screen_width(),
            screen_height(),
            screen_dpi_scale(),
            max_texture,
            cfg!(any(target_os = "android", target_os = "ios")),
        );
        let target = Self::allocate(size, resolve);
        let (scene_material, present_material) = quality_materials();
        Self {
            target,
            max_texture,
            resolve,
            smoke_size: None,
            scene_material,
            present_material,
            resize_candidate: None,
            budget,
            completion_sample: 0,
            gpu_ms: None,
        }
    }
    fn allocate((w, h): (u32, u32), _resolve: bool) -> RenderTarget {
        // Supersampling is portable to GLES2; avoid driver-dependent MSAA FBO resolves.
        let sample_count = 1;
        let target = render_target_ex(
            w,
            h,
            RenderTargetParams {
                sample_count,
                depth: false,
            },
        );
        target.texture.set_filter(FilterMode::Linear);
        crate::diagnostics::record("render-target", &format!("{w}x{h} samples={sample_count}"));
        target
    }
    pub fn resize(&mut self) -> bool {
        let mut changed = false;
        let size = self.smoke_size.unwrap_or_else(|| {
            self.budget.size(
                screen_width(),
                screen_height(),
                screen_dpi_scale(),
                self.max_texture,
                cfg!(any(target_os = "android", target_os = "ios")),
            )
        });
        if (
            self.target.texture.width() as u32,
            self.target.texture.height() as u32,
        ) != size
        {
            if self.smoke_size.is_none() {
                match self.resize_candidate {
                    Some((candidate, started)) if candidate == size => {
                        if started.elapsed() < std::time::Duration::from_millis(120) {
                            return false;
                        }
                    }
                    _ => {
                        self.resize_candidate = Some((size, std::time::Instant::now()));
                        return false;
                    }
                }
            }
            set_default_camera();
            self.target = Self::allocate(size, self.resolve);
            changed = true;
        }
        self.resize_candidate = None;
        changed
    }
    pub(crate) fn smoke_resolution(&mut self, width: u32, height: u32) {
        // Exercise high detail independently of monitor/window size; no user-facing setting.
        self.smoke_size = Some((width.min(self.max_texture), height.min(self.max_texture)));
    }
    pub fn use_scene_material(&self) {
        if let Some(material) = &self.scene_material {
            gl_use_material(material);
        } else {
            gl_use_default_material();
        }
    }
    pub fn scale(&self) -> Vec2 {
        vec2(
            self.target.texture.width() / screen_width(),
            self.target.texture.height() / screen_height(),
        )
    }
    pub fn present(&self) {
        set_default_camera();
        clear_background(super::ui::PAPER);
        if let Some(material) = &self.present_material {
            material.set_uniform(
                "SourceSize",
                vec2(self.target.texture.width(), self.target.texture.height()),
            );
            material.set_uniform(
                "OutputSize",
                vec2(
                    screen_width() * screen_dpi_scale(),
                    screen_height() * screen_dpi_scale(),
                ),
            );
            gl_use_material(material);
        }
        draw_texture_ex(
            &self.target.texture,
            0.,
            0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }
    /// Sparse completion samples include queued GPU work, rather than treating
    /// fast CPU submission as proof that the GPU kept up. No timestamp support
    /// is assumed on GLES2; unsupported backends retain CPU-based feedback.
    pub fn completion_ms(&mut self, continuous: bool) -> Option<f32> {
        if !continuous {
            self.gpu_ms = None;
            return None;
        }
        self.completion_sample = self.completion_sample.wrapping_add(1);
        if self.completion_sample.is_multiple_of(30) {
            unsafe {
                let mut gl = macroquad::window::get_internal_gl();
                if gl.quad_context.info().backend == macroquad::miniquad::Backend::OpenGl {
                    let started = std::time::Instant::now();
                    gl.flush();
                    macroquad::miniquad::gl::glFinish();
                    let elapsed = started.elapsed().as_secs_f32() * 1000.;
                    self.gpu_ms = Some(elapsed);
                    crate::diagnostics::measure("gpu_completion_ms", f64::from(elapsed));
                }
            }
        }
        self.gpu_ms
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn density_has_full_hd_floor_and_preserves_higher_native_resolution() {
        for (w, h, dpi) in [
            (390., 844., 1.),
            (1100., 800., 1.),
            (1920., 1080., 1.),
            (2560., 1440., 1.),
            (3840., 2160., 1.),
            (1080., 1920., 1.),
            (390., 844., 3.),
        ] {
            let (rw, rh) = render_size(w, h, dpi, 8192);
            assert!(rw.min(rh) >= 1080 && rw.max(rh) >= 1920);
            assert!(rw as f32 >= w * dpi && rh as f32 >= h * dpi);
            assert!((rw as f32 / rh as f32 - w / h).abs() < 0.04);
        }
        assert_eq!(render_size(3840., 2160., 1., 8192), (3840, 2160));
    }
    fn assert_limit(w: f32, h: f32, max: u32) {
        let (rw, rh) = render_size(w, h, 1., max);
        assert!(rw > 0 && rh > 0 && rw <= max && rh <= max);
        assert!(u64::from(rw) * u64::from(rh) <= 17_100_000);
    }
    #[test]
    fn gpu_and_memory_limits_are_automatic() {
        assert_limit(7680., 4320., 8192);
        assert_limit(100_000., 1000., 4096);
        assert_limit(1920., 1080., 1024);
    }
    #[test]
    fn one_pixel_resize_never_multiplies_render_work() {
        for height in [1079., 1080., 1081., 799., 800., 801.] {
            let (w, h) = render_size(1920., height, 1., 8192);
            let (nw, nh) = render_size(1920., height + 1., 1., 8192);
            let ratio = f64::from(w) * f64::from(h) / (f64::from(nw) * f64::from(nh));
            assert!((0.99..1.01).contains(&ratio));
        }
    }
}
