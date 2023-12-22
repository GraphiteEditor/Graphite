use std::error::Error;

use super::context::Context;

use graph_craft::graphene_compiler::Executor;

use graph_craft::proto::LocalFuture;
use graphene_core::gpu::PushConstants;

use bytemuck::Pod;
use dyn_any::StaticTypeSized;
use vulkano::buffer::{self, BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::Device;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::{ComputePipeline, Pipeline, PipelineBindPoint};
use vulkano::sync::GpuFuture;

#[derive(Debug)]
pub struct GpuExecutor<I: StaticTypeSized, O> {
	context: Context,
	entry_point: String,
	shader: std::sync::Arc<vulkano::shader::ShaderModule>,
	_phantom: std::marker::PhantomData<(I, O)>,
}

impl<I: StaticTypeSized, O> GpuExecutor<I, O> {
	pub fn new(context: Context, shader: &[u8], entry_point: String) -> anyhow::Result<Self> {
		let shader = unsafe { vulkano::shader::ShaderModule::from_bytes(context.device.clone(), shader)? };

		Ok(Self {
			context,
			entry_point,
			shader,
			_phantom: std::marker::PhantomData,
		})
	}
}

impl<'a, I: StaticTypeSized + Sync + Pod + Send + 'a, O: StaticTypeSized + Send + Sync + Pod + 'a> Executor<Vec<I>, Vec<O>> for &'a GpuExecutor<I, O> {
	fn execute(&self, input: Vec<I>) -> LocalFuture<Result<Vec<O>, Box<dyn Error>>> {
		let context = &self.context;
		let result: Vec<O> = execute_shader(
			context.device.clone(),
			context.queue.clone(),
			self.shader.entry_point(&self.entry_point).expect("Entry point not found in shader"),
			&context.allocator,
			&context.command_buffer_allocator,
			input,
		);
		Box::pin(async move { Ok(result) })
	}
}

// TODO: make async
fn execute_shader<I: Pod + Send + Sync, O: Pod + Send + Sync>(
	device: std::sync::Arc<Device>,
	queue: std::sync::Arc<vulkano::device::Queue>,
	entry_point: vulkano::shader::EntryPoint,
	alloc: &StandardMemoryAllocator,
	calloc: &StandardCommandBufferAllocator,
	data: Vec<I>,
) -> Vec<O> {
	let constants = PushConstants { n: data.len() as u32, node: 0 };

	let dest_data: Vec<_> = (0..constants.n).map(|_| O::zeroed()).collect();
	let source_buffer = create_buffer(data, alloc).expect("failed to create buffer");
	let dest_buffer = create_buffer(dest_data, alloc).expect("failed to create buffer");

	let compute_pipeline = ComputePipeline::new(device.clone(), entry_point, &(), None, |_| {}).expect("failed to create compute pipeline");
	let layout = compute_pipeline.layout().set_layouts().get(0).unwrap();
	let dalloc = StandardDescriptorSetAllocator::new(device.clone());
	let set = PersistentDescriptorSet::new(
		&dalloc,
		layout.clone(),
		[
			WriteDescriptorSet::buffer(0, source_buffer), // 0 is the binding
			WriteDescriptorSet::buffer(1, dest_buffer.clone()),
		],
	)
	.unwrap();
	let mut builder = AutoCommandBufferBuilder::primary(calloc, queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();

	builder
		.bind_pipeline_compute(compute_pipeline.clone())
		.bind_descriptor_sets(PipelineBindPoint::Compute, compute_pipeline.layout().clone(), 0, set)
		.push_constants(compute_pipeline.layout().clone(), 0, constants)
		.dispatch([((constants.n as isize - 1) / 1024 + 1) as u32 * 1024, 1, 1])
		.unwrap();
	let command_buffer = builder.build().unwrap();

	let future = vulkano::sync::now(device).then_execute(queue, command_buffer).unwrap().then_signal_fence_and_flush().unwrap();
	#[cfg(feature = "profiling")]
	nvtx::range_push!("compute");
	future.wait(None).unwrap();
	#[cfg(feature = "profiling")]
	nvtx::range_pop!();
	let content = dest_buffer.read().unwrap();
	content.to_vec()
}

fn create_buffer<T: Pod + Send + Sync>(data: Vec<T>, alloc: &StandardMemoryAllocator) -> Result<std::sync::Arc<CpuAccessibleBuffer<[T]>>, vulkano::memory::allocator::AllocationCreationError> {
	let buffer_usage = BufferUsage {
		storage_buffer: true,
		transfer_src: true,
		transfer_dst: true,
		..Default::default()
	};

	buffer::CpuAccessibleBuffer::from_iter(alloc, buffer_usage, false, data)
}

// TODO: Fix this test
// #[cfg(test)]
// mod test {
// 	use graph_craft::proto::{ConstructionArgs, ProtoNodeIdentifier, ProtoNetwork, ProtoNode, ProtoNodeInput, Type};
// 	use graph_craft::{concrete, generic};

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
// }
