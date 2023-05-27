use glam::UVec3;
use gpu_executor::{Bindgroup, PipelineLayout, StorageBufferOptions};
use gpu_executor::{GpuExecutor, ShaderIO, ShaderInput};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;
use graph_craft::proto::*;
use graphene_core::raster::*;
use graphene_core::value::ValueNode;
use graphene_core::*;
use wgpu_executor::NewExecutor;

use bytemuck::Pod;
use core::marker::PhantomData;
use dyn_any::StaticTypeSized;
use std::sync::Arc;

pub struct GpuCompiler<TypingContext, ShaderIO> {
	typing_context: TypingContext,
	io: ShaderIO,
}

// TODO: Move to graph-craft
#[node_macro::node_fn(GpuCompiler)]
async fn compile_gpu(node: &'input DocumentNode, mut typing_context: TypingContext, io: ShaderIO) -> compilation_client::Shader {
	let compiler = graph_craft::executor::Compiler {};
	let DocumentNodeImplementation::Network(ref network) = node.implementation else { panic!() };
	let proto_networks: Vec<_> = compiler.compile(network.clone(), true).collect();

	for network in proto_networks.iter() {
		typing_context.update(network).expect("Failed to type check network");
	}
	// TODO: do a proper union
	let input_types = proto_networks[0]
		.inputs
		.iter()
		.map(|id| typing_context.type_of(*id).unwrap())
		.map(|node_io| node_io.output.clone())
		.collect();
	let output_types = proto_networks.iter().map(|network| typing_context.type_of(network.output).unwrap().output.clone()).collect();

	compilation_client::compile(proto_networks, input_types, output_types, io).await.unwrap()
}

pub struct MapGpuNode<Node> {
	node: Node,
}

#[node_macro::node_fn(MapGpuNode)]
async fn map_gpu(image: ImageFrame<Color>, node: DocumentNode) -> ImageFrame<Color> {
	log::debug!("Executing gpu node");
	let compiler = graph_craft::executor::Compiler {};
	let inner_network = NodeNetwork::value_network(node);

	log::debug!("inner_network: {:?}", inner_network);
	let network = NodeNetwork {
		inputs: vec![], //vec![0, 1],
		outputs: vec![NodeOutput::new(1, 0)],
		nodes: [
			DocumentNode {
				name: "Slice".into(),
				inputs: vec![NodeInput::Inline(InlineRust::new("i0[_global_index.x as usize]".into(), concrete![Color]))],
				implementation: DocumentNodeImplementation::Unresolved("graphene_core::value::CopiedNode".into()),
				..Default::default()
			},
			/*DocumentNode {
				name: "Index".into(),
				//inputs: vec![NodeInput::Network(concrete!(UVec3))],
				inputs: vec![NodeInput::Inline(InlineRust::new("i1.x as usize".into(), concrete![u32]))],
				implementation: DocumentNodeImplementation::Unresolved("graphene_core::value::CopiedNode".into()),
				..Default::default()
			},*/
			/*
			DocumentNode {
				name: "GetNode".into(),
				inputs: vec![NodeInput::node(1, 0), NodeInput::node(0, 0)],
				implementation: DocumentNodeImplementation::Unresolved("graphene_core::storage::GetNode".into()),
				..Default::default()
			},*/
			DocumentNode {
				name: "MapNode".into(),
				inputs: vec![NodeInput::node(0, 0)],
				implementation: DocumentNodeImplementation::Network(inner_network),
				..Default::default()
			},
			/*
			DocumentNode {
				name: "SaveNode".into(),
				inputs: vec![
					//NodeInput::node(0, 0),
					NodeInput::Inline(InlineRust::new(
						"o0[_global_index.x as usize] = i0[_global_index.x as usize]".into(),
						Type::Fn(Box::new(concrete!(Color)), Box::new(concrete!(()))),
					)),
				],
				implementation: DocumentNodeImplementation::Unresolved("graphene_core::value::ValueNode".into()),
				..Default::default()
			},
			*/
		]
		.into_iter()
		.enumerate()
		.map(|(i, n)| (i as u64, n))
		.collect(),
		..Default::default()
	};
	log::debug!("compiling network");
	let proto_networks = compiler.compile(network.clone(), true).collect();
	log::debug!("compiling shader");
	let shader = compilation_client::compile(
		proto_networks,
		vec![concrete!(Color)], //, concrete!(u32)],
		vec![concrete!(Color)],
		ShaderIO {
			inputs: vec![
				ShaderInput::StorageBuffer((), concrete!(Color)),
				//ShaderInput::Constant(gpu_executor::GPUConstant::GlobalInvocationId),
				ShaderInput::OutputBuffer((), concrete!(Color)),
			],
			output: ShaderInput::OutputBuffer((), concrete!(Color)),
		},
	)
	.await
	.unwrap();
	//return ImageFrame::empty();
	let len = image.image.data.len();
	log::debug!("instances: {}", len);

	let executor = NewExecutor::new().await.unwrap();
	log::debug!("creating buffer");
	let storage_buffer = executor
		.create_storage_buffer(
			image.image.data.clone(),
			StorageBufferOptions {
				cpu_writable: false,
				gpu_writable: true,
				cpu_readable: false,
				storage: true,
			},
		)
		.unwrap();
	let storage_buffer = Arc::new(storage_buffer);
	let output_buffer = executor.create_output_buffer(len, concrete!(Color), false).unwrap();
	let output_buffer = Arc::new(output_buffer);
	let readback_buffer = executor.create_output_buffer(len, concrete!(Color), true).unwrap();
	let readback_buffer = Arc::new(readback_buffer);
	log::debug!("created buffer");
	let bind_group = Bindgroup {
		buffers: vec![storage_buffer.clone()],
	};

	let shader = gpu_executor::Shader {
		source: shader.spirv_binary.into(),
		name: "gpu::eval",
		io: shader.io,
	};
	log::debug!("loading shader");
	log::debug!("shader: {:?}", shader.source);
	let shader = executor.load_shader(shader).unwrap();
	log::debug!("loaded shader");
	let pipeline = PipelineLayout {
		shader,
		entry_point: "eval".to_string(),
		bind_group,
		output_buffer: output_buffer.clone(),
	};
	log::debug!("created pipeline");
	let compute_pass = executor.create_compute_pass(&pipeline, Some(readback_buffer.clone()), len.min(65535) as u32).unwrap();
	executor.execute_compute_pipeline(compute_pass).unwrap();
	log::debug!("executed pipeline");
	log::debug!("reading buffer");
	let result = executor.read_output_buffer(readback_buffer).await.unwrap();
	let colors = bytemuck::pod_collect_to_vec::<u8, Color>(result.as_slice());
	ImageFrame {
		image: Image {
			data: colors,
			width: image.image.width,
			height: image.image.height,
		},
		transform: image.transform,
	}

	/*
	let executor: GpuExecutor = GpuExecutor::new(Context::new().await.unwrap(), shader.into(), "gpu::eval".into()).unwrap();
	let data: Vec<_> = input.into_iter().collect();
	let result = executor.execute(Box::new(data)).unwrap();
	let result = dyn_any::downcast::<Vec<_O>>(result).unwrap();
	*result
	*/
}
/*
#[node_macro::node_fn(MapGpuNode)]
async fn map_gpu(inputs: Vec<ShaderInput<<NewExecutor as GpuExecutor>::BufferHandle>>, shader: &'any_input compilation_client::Shader) {
	use graph_craft::executor::Executor;
	let executor = NewExecutor::new().unwrap();
	for input in shader.io.inputs.iter() {
		let buffer = executor.create_storage_buffer(&self, data, options)
		let buffer = executor.create_buffer(input.size).unwrap();
		executor.write_buffer(buffer, input.data).unwrap();
	}
	todo!();
	/*
	let executor: GpuExecutor = GpuExecutor::new(Context::new().await.unwrap(), shader.into(), "gpu::eval".into()).unwrap();
	let data: Vec<_> = input.into_iter().collect();
	let result = executor.execute(Box::new(data)).unwrap();
	let result = dyn_any::downcast::<Vec<_O>>(result).unwrap();
	*result
	*/
}

pub struct MapGpuSingleImageNode<N> {
	node: N,
}

#[node_macro::node_fn(MapGpuSingleImageNode)]
fn map_gpu_single_image(input: Image<Color>, node: String) -> Image<Color> {
	use graph_craft::document::*;
	use graph_craft::NodeIdentifier;

	let identifier = NodeIdentifier { name: std::borrow::Cow::Owned(node) };

	let network = NodeNetwork {
		inputs: vec![0],
		disabled: vec![],
		previous_outputs: None,
		outputs: vec![NodeOutput::new(0, 0)],
		nodes: [(
			0,
			DocumentNode {
				name: "Image Filter".into(),
				inputs: vec![NodeInput::Network(concrete!(Color))],
				implementation: DocumentNodeImplementation::Unresolved(identifier),
				metadata: DocumentNodeMetadata::default(),
				..Default::default()
			},
		)]
		.into_iter()
		.collect(),
	};

	let value_network = ValueNode::new(network);
	let map_node = MapGpuNode::new(value_network);
	let data = map_node.eval(input.data.clone());
	Image { data, ..input }
}
*/
