mod context;
mod executor;

pub use context::Context;
pub use executor::GpuExecutor;
use gpu_executor::{ComputePassDimensions, Shader, ShaderInput, StorageBufferOptions, ToStorageBuffer, ToUniformBuffer};
use graph_craft::Type;

use anyhow::{bail, Result};
use futures::Future;
use std::pin::Pin;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::{Buffer, BufferDescriptor, CommandBuffer, ShaderModule};

#[derive(Debug, Clone)]
pub struct NewExecutor {
	context: Context,
}

impl gpu_executor::GpuExecutor for NewExecutor {
	type ShaderHandle = ShaderModule;
	type BufferHandle = Buffer;
	type CommandBuffer = CommandBuffer;

	fn load_shader(&self, shader: Shader) -> Result<Self::ShaderHandle> {
		let shader_module = self.context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some(shader.name),
			source: wgpu::ShaderSource::SpirV(shader.source),
		});
		Ok(shader_module)
	}

	fn create_uniform_buffer<T: ToUniformBuffer>(&self, data: T) -> Result<ShaderInput<Self::BufferHandle>> {
		let bytes = data.to_bytes();
		let buffer = self.context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytes.as_ref(),
			usage: wgpu::BufferUsages::UNIFORM,
		});
		Ok(ShaderInput::UniformBuffer(buffer, Type::new::<T>()))
	}

	fn create_storage_buffer<T: ToStorageBuffer>(&self, data: T, options: StorageBufferOptions) -> Result<ShaderInput<Self::BufferHandle>> {
		let bytes = data.to_bytes();
		let mut usage = wgpu::BufferUsages::empty();

		if options.storage {
			usage |= wgpu::BufferUsages::STORAGE;
		}
		if options.gpu_writable {
			usage |= wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
		}
		if options.cpu_readable {
			usage |= wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
		}
		if options.cpu_writable {
			usage |= wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC;
		}

		log::debug!("Creating storage buffer with usage {:?} and len: {}", usage, bytes.len());
		let buffer = self.context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: None,
			contents: bytes.as_ref(),
			usage,
		});
		Ok(ShaderInput::StorageBuffer(buffer, data.ty()))
	}

	fn create_output_buffer(&self, len: usize, ty: Type, cpu_readable: bool) -> Result<ShaderInput<Self::BufferHandle>> {
		log::debug!("Creating output buffer with len: {}", len);
		let create_buffer = |usage| {
			Ok::<_, anyhow::Error>(self.context.device.create_buffer(&BufferDescriptor {
				label: None,
				size: len as u64 * ty.size().ok_or_else(|| anyhow::anyhow!("Cannot create buffer of type {:?}", ty))? as u64,
				usage,
				mapped_at_creation: false,
			}))
		};
		let buffer = match cpu_readable {
			true => ShaderInput::ReadBackBuffer(create_buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ)?, ty),
			false => ShaderInput::OutputBuffer(create_buffer(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC)?, ty),
		};
		Ok(buffer)
	}
	fn create_compute_pass(&self, layout: &gpu_executor::PipelineLayout<Self>, read_back: Option<Arc<ShaderInput<Self::BufferHandle>>>, instances: ComputePassDimensions) -> Result<CommandBuffer> {
		let compute_pipeline = self.context.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
			label: None,
			layout: None,
			module: &layout.shader,
			entry_point: layout.entry_point.as_str(),
		});
		let bind_group_layout = compute_pipeline.get_bind_group_layout(0);

		let entries = layout
			.bind_group
			.buffers
			.iter()
			.chain(std::iter::once(&layout.output_buffer))
			.flat_map(|input| input.buffer())
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

		let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
		{
			let dimensions = instances.get();
			let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
			cpass.set_pipeline(&compute_pipeline);
			cpass.set_bind_group(0, &bind_group, &[]);
			cpass.insert_debug_marker("compute node network evaluation");
			cpass.dispatch_workgroups(dimensions.0, dimensions.1, dimensions.2); // Number of cells to run, the (x,y,z) size of item being processed
		}
		// Sets adds copy operation to command encoder.
		// Will copy data from storage buffer on GPU to staging buffer on CPU.
		if let Some(buffer) = read_back {
			let ShaderInput::ReadBackBuffer(output, ty) = buffer.as_ref() else {
				bail!("Tried to read back from a non read back buffer");
			};
			let size = output.size();
			assert_eq!(size, layout.output_buffer.buffer().unwrap().size());
			assert_eq!(ty, &layout.output_buffer.ty());
			encoder.copy_buffer_to_buffer(
				layout.output_buffer.buffer().ok_or_else(|| anyhow::anyhow!("Tried to use an non buffer as the shader output"))?,
				0,
				&output,
				0,
				size,
			);
		}

		// Submits command encoder for processing
		Ok(encoder.finish())
	}

	fn execute_compute_pipeline(&self, encoder: Self::CommandBuffer) -> Result<()> {
		self.context.queue.submit(Some(encoder));

		// Poll the device in a blocking manner so that our future resolves.
		// In an actual application, `device.poll(...)` should
		// be called in an event loop or on another thread.
		self.context.device.poll(wgpu::Maintain::Wait);
		Ok(())
	}

	fn read_output_buffer(&self, buffer: Arc<ShaderInput<Self::BufferHandle>>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>>>> {
		let future = Box::pin(async move {
			if let ShaderInput::ReadBackBuffer(buffer, _) = buffer.as_ref() {
				let buffer_slice = buffer.slice(..);

				// Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
				let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
				buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

				// Wait for the mapping to finish.
				#[cfg(feature = "profiling")]
				nvtx::range_push!("compute");
				let result = receiver.receive().await;
				#[cfg(feature = "profiling")]
				nvtx::range_pop!();

				if result == Some(Ok(())) {
					// Gets contents of buffer
					let data = buffer_slice.get_mapped_range();
					// Since contents are got in bytes, this converts these bytes back to u32
					let result = bytemuck::cast_slice(&data).to_vec();

					// With the current interface, we have to make sure all mapped views are
					// dropped before we unmap the buffer.
					drop(data);
					buffer.unmap(); // Unmaps buffer from memory

					// Returns data from buffer
					Ok(result)
				} else {
					bail!("failed to run compute on gpu!")
				}
			} else {
				bail!("Tried to read a non readback buffer")
			}
		});
		future
	}
}

impl NewExecutor {
	pub async fn new() -> Option<Self> {
		let context = Context::new().await?;
		Some(Self { context })
	}
}
