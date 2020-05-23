use crate::resource_cache::ResourceCache;
use crate::draw_command::DrawCommand;
use crate::color::Color;
use crate::texture::Texture;
use crate::pipeline::Pipeline;
use crate::gui_attributes::*;

pub struct GuiNode {
	pub form_factor: GuiNodeUniform,
}

impl GuiNode {
	pub fn new(width: u32, height: u32, color: Color) -> Self {
		Self {
			form_factor: GuiNodeUniform::new(width, height, color),
		}
	}

	// pub fn get_pipeline(&self, pipeline_cache: &ResourceCache<Pipeline>) -> &Pipeline {
	// 	pipeline_cache.get("gui_rect").unwrap()
	// }

	pub fn build_draw_command(&mut self, device: &wgpu::Device) -> DrawCommand {
		const VERTICES: &[[f32; 2]] = &[
			[-0.5, 0.5],
			[0.5, 0.5],
			[0.5, 1.0],
			[-0.5, 1.0],
		];
		const INDICES: &[u16] = &[
			0, 1, 2,
			0, 2, 3,
		];
		
		// Create a draw command with the vertex data then push it to the GPU command queue
		DrawCommand::new(device, VERTICES, INDICES)
	}

	pub fn build_bind_groups(&mut self, device: &wgpu::Device, queue: &mut wgpu::Queue, pipeline: &Pipeline, texture_cache: &mut ResourceCache<Texture>) -> Vec<wgpu::BindGroup> {
		// Load the cached texture
		let texture = Texture::cached_load(device, queue, "textures/grid.png", texture_cache);

		// Build a staging buffer from the uniform resource data
		let binding_staging_buffer = Pipeline::build_binding_staging_buffer(device, &self.form_factor);

		// Construct the bind group for this GUI node
		let bind_group = Pipeline::build_bind_group(device, &pipeline.bind_group_layout, vec![
			Pipeline::build_binding_resource(&binding_staging_buffer),
			wgpu::BindingResource::TextureView(&texture.texture_view),
		]);
		
		vec![
			bind_group,
		]
	}
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GuiNodeUniform {
	pub dimensions: Dimensions<u32>,
	pub corners_radius: Corners<f32>,
	pub sides_inset: Sides<f32>,
	pub border_thickness: f32,
	pub border_color: Color,
	pub fill_color: Color,
}

impl GuiNodeUniform {
	pub fn new(width: u32, height: u32, color: Color) -> Self {
		GuiNodeUniform {
			dimensions: Dimensions::<u32>::new(width, height),
			corners_radius: Corners::<f32>::new(0.0, 0.0, 0.0, 0.0),
			sides_inset: Sides::<f32>::new(0.0, 0.0, 0.0, 0.0),
			border_thickness: 0.0,
			border_color: Color::TRANSPARENT,
			fill_color: color,
		}
	}
}

unsafe impl bytemuck::Zeroable for GuiNodeUniform {}
unsafe impl bytemuck::Pod for GuiNodeUniform {}
