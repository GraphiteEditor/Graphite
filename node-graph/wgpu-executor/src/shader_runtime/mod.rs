use crate::Context;
use crate::shader_runtime::per_pixel_adjust_runtime::PerPixelAdjustShaderRuntime;

pub mod per_pixel_adjust_runtime;

pub const FULLSCREEN_VERTEX_SHADER_NAME: &str = "fullscreen_vertexfullscreen_vertex";

pub struct ShaderRuntime {
	context: Context,
	per_pixel_adjust: PerPixelAdjustShaderRuntime,
}

impl ShaderRuntime {
	pub fn new(context: &Context) -> Self {
		Self {
			context: context.clone(),
			per_pixel_adjust: PerPixelAdjustShaderRuntime::new(),
		}
	}
}
