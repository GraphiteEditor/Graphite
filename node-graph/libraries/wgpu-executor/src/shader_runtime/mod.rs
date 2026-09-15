use crate::shader_runtime::per_pixel_adjust_runtime::PerPixelAdjustShaderRuntime;

pub mod per_pixel_adjust_runtime;

pub const FULLSCREEN_VERTEX_SHADER_NAME: &str = "fullscreen_vertex_fullscreen_vertex";

#[derive(Default)]
pub struct ShaderRuntime {
	per_pixel_adjust: PerPixelAdjustShaderRuntime,
}
