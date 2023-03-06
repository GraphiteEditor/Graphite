mod context;
mod executor;

use anyhow::Result;
pub use context::Context;
pub use executor::GpuExecutor;
use gpu_executor::{Shader, StorageBufferOptions, ToStorageBuffer, ToUniformBuffer};
use wgpu::{util::DeviceExt, BindGroup, Buffer, BufferDescriptor, CommandBuffer, CommandEncoder, ComputePipeline, ShaderModule};

struct NewExecutor {
	context: Context,
}

impl gpu_executor::GpuExecutor for NewExecutor {
	type ShaderHandle = ShaderModule;
	type BufferHandle = Buffer;
	type ComputePipelineHandle = ComputePipeline;
	type BindGroup = BindGroup;
	type CommandBuffer = CommandBuffer;

	fn load_shader(&mut self, shader: &Shader) -> Result<Self::ShaderHandle> {
		let shader = self.context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some(shader.name),
			source: wgpu::ShaderSource::SpirV(shader.source),
		});
		Ok(shader)
	}

	fn create_uniform_buffer<T: ToUniformBuffer>(&mut self, data: T) -> Result<Self::BufferHandle> {
		let bytes = data.to_bytes();
		let buffer = self.context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytes.as_ref(),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		Ok(buffer)
	}

	fn create_storage_buffer<T: ToStorageBuffer>(&mut self, data: T, options: StorageBufferOptions) -> Result<Self::BufferHandle> {
		let bytes = data.to_bytes();
		let mut usage = wgpu::BufferUsages::STORAGE;

		if options.gpu_writable {
			usage |= wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
		}
		if options.cpu_readable {
			usage |= wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
		}
		if options.cpu_writable {
			usage |= wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC;
		}

		let buffer = self.context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytes.as_ref(),
			usage,
		});
		Ok(buffer)
	}

	fn create_output_buffer(&mut self, size: u64) -> Result<Self::BufferHandle> {
		let mut usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ;

		let buffer = self.context.device.create_buffer(&BufferDescriptor {
			label: None,
			size,
			usage,
			mapped_at_creation: false,
		});
		Ok(buffer)
	}

	fn create_compute_pass(&mut self, layout: &gpu_executor::PipelineLayout<Self::ShaderHandle, Self::BufferHandle>, output: Buffer) -> Result<Encoder> {
		let compute_pipeline = self.context.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
			label: None,
			layout: None,
			module: &layout.shader,
			entry_point: layout.entry_point.as_str(),
		});
		let bind_group_layout = compute_pipeline.get_bind_group_layout(0);

		let entries = layout
			.uniform_buffers
			.iter()
			.chain(layout.storage_buffers.iter())
			.chain(std::iter::once(&layout.output_buffer))
			.enumerate()
			.map(|(i, buffer)| wgpu::BindGroupEntry {
				binding: i as u32,
				resource: buffer.as_entire_binding(),
			})
			.collect::<Vec<_>>();

		let bind_group = self.context.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: None,
			layout: &bind_group_layout,
			entries: entries.as_slice(),
		});
	}

	fn execute_compute_pipeline(&mut self, pipeline: Self::CommandBuffer, bind_group: BindGroup, intstances: u32) -> Result<()> {
		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
		{
			let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
			cpass.set_pipeline(&pipeline);
			cpass.set_bind_group(0, &bind_group, &[]);
			cpass.insert_debug_marker("compute node network evaluation");
			cpass.dispatch_workgroups(intstances, 1, 1); // Number of cells to run, the (x,y,z) size of item being processed
		}
		// Sets adds copy operation to command encoder.
		// Will copy data from storage buffer on GPU to staging buffer on CPU.
		encoder.copy_buffer_to_buffer(&dest_buffer, 0, &staging_buffer, 0, size);

		// Submits command encoder for processing
		Ok(encoder.finish())
	}

	fn read_output_buffer(&mut self, buffer: Self::BufferHandle) -> Result<Vec<u8>> {
		let buffer_slice = buffer.slice(..);

		// Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
		let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
		buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

		// Poll the device in a blocking manner so that our future resolves.
		// In an actual application, `device.poll(...)` should
		// be called in an event loop or on another thread.
		self.context.device.poll(wgpu::Maintain::Wait);

		// Wait for the mapping to finish.
		let buffer_slice = receiver.await.unwrap();

		// Get the data from the buffer slice.
		let data = buffer_slice.get_mapped_range();

		// Copy the data into a Vec.
		let data = data.to_vec();

		// Unmap the buffer slice.
		buffer_slice.unmap();

		Ok(data)
	}
}
