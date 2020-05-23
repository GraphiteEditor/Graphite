// use crate::bind_group_resource::BindGroupResource;

pub struct DrawCommand {
	pub pipeline_name: String,
	pub bind_groups: Vec<wgpu::BindGroup>,
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
	pub index_count: u32,
}

impl DrawCommand {
	pub fn new(device: &wgpu::Device, pipeline_name: String, bind_groups: Vec<wgpu::BindGroup>, vertices: &[[f32; 2]], indices: &[u16]) -> Self {
		let vertex_buffer = device.create_buffer_with_data(bytemuck::cast_slice(vertices), wgpu::BufferUsage::VERTEX);
		let index_buffer = device.create_buffer_with_data(bytemuck::cast_slice(indices), wgpu::BufferUsage::INDEX);
		let index_count = indices.len() as u32;

		Self {
			pipeline_name,
			bind_groups,
			vertex_buffer,
			index_buffer,
			index_count,
		}
	}
}