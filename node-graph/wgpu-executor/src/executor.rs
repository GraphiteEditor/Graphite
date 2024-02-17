use std::sync::Arc;
use std::{borrow::Cow, error::Error};
use wgpu::util::DeviceExt;

use super::context::Context;
use bytemuck::Pod;
use dyn_any::StaticTypeSized;
use graph_craft::{graphene_compiler::Executor, proto::LocalFuture};

#[derive(Debug)]
pub struct GpuExecutor<'a, I: StaticTypeSized, O> {
	context: Context,
	entry_point: String,
	shader: Cow<'a, [u32]>,
	_phantom: std::marker::PhantomData<(I, O)>,
}

impl<'a, I: StaticTypeSized, O> GpuExecutor<'a, I, O> {
	pub fn new(context: Context, shader: Cow<'a, [u32]>, entry_point: String) -> anyhow::Result<Self> {
		Ok(Self {
			context,
			entry_point,
			shader,
			_phantom: std::marker::PhantomData,
		})
	}
}

impl<'a, I: StaticTypeSized + Sync + Pod + Send, O: StaticTypeSized + Send + Sync + Pod> Executor<Vec<I>, Vec<O>> for GpuExecutor<'a, I, O> {
	fn execute(&self, input: Vec<I>) -> LocalFuture<Result<Vec<O>, Box<dyn Error>>> {
		let context = &self.context;
		let future = execute_shader(context.device.clone(), context.queue.clone(), self.shader.to_vec(), input, self.entry_point.clone());
		Box::pin(async move {
			let result = future.await;

			let result: Vec<O> = result.ok_or_else(|| String::from("Failed to execute shader"))?;
			Ok(result)
		})
	}
}

async fn execute_shader<I: Pod + Send + Sync, O: Pod + Send + Sync>(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, shader: Vec<u32>, data: Vec<I>, entry_point: String) -> Option<Vec<O>> {
	// Loads the shader from WGSL
	dbg!(&shader);
	//write shader to file
	use std::io::Write;
	let mut file = std::fs::File::create("/tmp/shader.spv").unwrap();
	file.write_all(bytemuck::cast_slice(&shader)).unwrap();
	let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: None,
		source: wgpu::ShaderSource::SpirV(shader.into()),
	});

	// Gets the size in bytes of the buffer.
	let slice_size = data.len() * std::mem::size_of::<O>();
	let size = slice_size as wgpu::BufferAddress;

	// Instantiates buffer without data.
	// `usage` of buffer specifies how it can be used:
	//   `BufferUsages::MAP_READ` allows it to be read (outside the shader).
	//   `BufferUsages::COPY_DST` allows it to be the destination of the copy.
	let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
		label: None,
		size,
		usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
		mapped_at_creation: false,
	});

	// Instantiates buffer with data (`numbers`).
	// Usage allowing the buffer to be:
	//   A storage buffer (can be bound within a bind group and thus available to a shader).
	//   The destination of a copy.
	//   The source of a copy.
	let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Storage Buffer"),
		contents: bytemuck::cast_slice(&data),
		usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
	});

	// Instantiates empty buffer for the result.
	// Usage allowing the buffer to be:
	//  A storage buffer (can be bound within a bind group and thus available to a shader).
	//  The destination of a copy.
	//  The source of a copy.
	let dest_buffer = device.create_buffer(&wgpu::BufferDescriptor {
		label: Some("Destination Buffer"),
		size,
		usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
		mapped_at_creation: false,
	});

	// A bind group defines how buffers are accessed by shaders.
	// It is to WebGPU what a descriptor set is to Vulkan.
	// `binding` here refers to the `binding` of a buffer in the shader (`layout(set = 0, binding = 0) buffer`).

	// A pipeline specifies the operation of a shader

	// Instantiates the pipeline.
	let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
		label: None,
		layout: None,
		module: &cs_module,
		entry_point: entry_point.as_str(),
	});

	// Instantiates the bind group, once again specifying the binding of buffers.
	let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: None,
		layout: &bind_group_layout,
		entries: &[
			wgpu::BindGroupEntry {
				binding: 0,
				resource: storage_buffer.as_entire_binding(),
			},
			wgpu::BindGroupEntry {
				binding: 1,
				resource: dest_buffer.as_entire_binding(),
			},
		],
	});

	// A command encoder executes one or many pipelines.
	// It is to WebGPU what a command buffer is to Vulkan.
	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
	{
		let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
		cpass.set_pipeline(&compute_pipeline);
		cpass.set_bind_group(0, &bind_group, &[]);
		cpass.insert_debug_marker("compute node network evaluation");
		cpass.dispatch_workgroups(data.len().min(65535) as u32, 1, 1); // Number of cells to run, the (x,y,z) size of item being processed
	}
	// Sets adds copy operation to command encoder.
	// Will copy data from storage buffer on GPU to staging buffer on CPU.
	encoder.copy_buffer_to_buffer(&dest_buffer, 0, &staging_buffer, 0, size);

	// Submits command encoder for processing
	queue.submit(Some(encoder.finish()));

	// Note that we're not calling `.await` here.
	let buffer_slice = staging_buffer.slice(..);
	// Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
	let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
	buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

	// Poll the device in a blocking manner so that our future resolves.
	// In an actual application, `device.poll(...)` should
	// be called in an event loop or on another thread.
	device.poll(wgpu::Maintain::Wait);

	// Awaits until `buffer_future` can be read from
	#[cfg(feature = "profiling")]
	nvtx::range_push!("compute");
	let result = receiver.receive().await;
	#[cfg(feature = "profiling")]
	nvtx::range_pop!();
	if let Some(Ok(())) = result {
		// Gets contents of buffer
		let data = buffer_slice.get_mapped_range();
		// Since contents are got in bytes, this converts these bytes back to u32
		let result = bytemuck::cast_slice(&data).to_vec();

		// With the current interface, we have to make sure all mapped views are
		// dropped before we unmap the buffer.
		drop(data);
		staging_buffer.unmap(); // Unmaps buffer from memory
						// If you are familiar with C++ these 2 lines can be thought of similarly to:
						//   delete myPointer;
						//   myPointer = NULL;
						// It effectively frees the memory

		// Returns data from buffer
		Some(result)
	} else {
		panic!("failed to run compute on gpu!")
	}
}

// TODO: Fix this test
// #[cfg(test)]
// mod test {
// 	use super::*;
// 	use graph_craft::concrete;
// 	use graph_craft::generic;
// 	use graph_craft::proto::*;

// 	#[test]
// 	fn add_on_gpu() {
// 		use crate::executor::Executor;
// 		let m = compiler::Metadata::new("project".to_owned(), vec!["test@example.com".to_owned()]);
// 		let network = inc_network();
// 		let temp_dir = tempfile::tempdir().expect("failed to create tempdir");

// 		let executor: GpuExecutor<u32, u32> = GpuExecutor::new(Context::new(), network, m, temp_dir.path()).unwrap();

// 		let data: Vec<_> = (0..1024).map(|x| x as u32).collect();
// 		let result = executor.execute(Box::new(data)).unwrap();
// 		let result = dyn_any::downcast::<Vec<u32>>(result).unwrap();
// 		for (i, r) in result.iter().enumerate() {
// 			assert_eq!(*r, i as u32 + 3);
// 		}
// 	}

// 	fn inc_network() -> ProtoNetwork {
// 		let mut construction_network = ProtoNetwork {
// 			inputs: vec![NodeId(10)],
// 			output: NodeId(1),
// 			nodes: [
// 				(
// 					NodeId(1),
// 					ProtoNode {
// 						identifier: ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode", &[generic!("u32")]),
// 						input: ProtoNodeInput::Node(11),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 					},
// 				),
// 				(
// 					NodeId(10),
// 					ProtoNode {
// 						identifier: ProtoNodeIdentifier::new("graphene_core::structural::ConsNode", &[generic!("&ValueNode<u32>"), generic!("()")]),
// 						input: ProtoNodeInput::Network,
// 						construction_args: ConstructionArgs::Nodes(vec![14]),
// 					},
// 				),
// 				(
// 					NodeId(11),
// 					ProtoNode {
// 						identifier: ProtoNodeIdentifier::new("graphene_core::ops::AddPairNode", &[generic!("u32"), generic!("u32")]),
// 						input: ProtoNodeInput::Node(10),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 					},
// 				),
// 				(
// 					NodeId(14),
// 					ProtoNode {
// 						identifier: ProtoNodeIdentifier::new("graphene_core::value::ValueNode", &[concrete!("u32")]),
// 						input: ProtoNodeInput::None,
// 						construction_args: ConstructionArgs::Value(Box::new(3_u32)),
// 					},
// 				),
// 			]
// 			.into_iter()
// 			.collect(),
// 		};
// 		construction_network.resolve_inputs();
// 		construction_network.reorder_ids();
// 		construction_network
// 	}
// }
