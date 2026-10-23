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

pub struct AdaptiveScene {
    pub target: RenderTarget,
    max_texture: u32,
    resolve: bool,
    smoke_size: Option<(u32, u32)>,
    scene_material: Option<Material>,
    present_material: Option<Material>,
    resize_candidate: Option<((u32, u32), std::time::Instant)>,
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
        let size = render_size(
            screen_width(),
            screen_height(),
            screen_dpi_scale(),
            max_texture,
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
    pub fn resize(&mut self) {
        let size = self.smoke_size.unwrap_or_else(|| {
            render_size(
                screen_width(),
                screen_height(),
                screen_dpi_scale(),
                self.max_texture,
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
                            return;
                        }
                    }
                    _ => {
                        self.resize_candidate = Some((size, std::time::Instant::now()));
                        return;
                    }
                }
            }
            set_default_camera();
            self.target = Self::allocate(size, self.resolve);
        }
        self.resize_candidate = None;
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
