use gpu_executor::ShaderIO;
use graph_craft::document::*;
use graph_craft::proto::*;
use graphene_core::raster::*;
use graphene_core::value::ValueNode;
use graphene_core::*;

use bytemuck::Pod;
use core::marker::PhantomData;
use dyn_any::StaticTypeSized;

pub struct GpuCompiler<TypingContext, ShaderIO> {
	typing_context: TypingContext,
	io: ShaderIO,
}

// Move to graph-craft
#[node_macro::node_fn(GpuCompiler)]
fn compile_gpu(node: &'input DocumentNode, mut typing_context: TypingContext, io: ShaderIO) -> compilation_client::Shader {
	let compiler = graph_craft::executor::Compiler {};
	let DocumentNodeImplementation::Network(network) = node.implementation;
	let proto_network = compiler.compile_single(network, true).unwrap();
	typing_context.update(&proto_network);
	let input_types = proto_network.inputs.iter().map(|id| typing_context.get_type(*id).unwrap()).map(|node_io| node_io.output).collect();
	let output_type = typing_context.get_type(proto_network.output).unwrap().output;

	let bytes = compilation_client::compile_sync(proto_network, input_types, output_type, io).unwrap();
	bytes
}

pub struct MapGpuNode<Shader> {
	shader: Shader,
}
use gpu_executor::GpuExecutor;
use gpu_executor::ShaderInput;
use wgpu_executor::NewExecutor;

#[node_macro::node_fn(MapGpuNode)]
fn map_gpu(inputs: Vec<ShaderInput<<NewExecutor as GpuExecutor>::BufferHandle>>, shader: &'any_input compilation_client::Shader) {
	use graph_craft::executor::Executor;
	let executor = NewExecutor::new().unwrap();
	for input in shader.inputs.iter() {
		let buffer = executor.create_buffer(input.size).unwrap();
		executor.write_buffer(buffer, input.data).unwrap();
	}
	/*let executor: GpuExecutor= GpuExecutor::new(Context::new_sync().unwrap(), shader.into(), "gpu::eval".into()).unwrap();
	let data: Vec<_> = input.into_iter().collect();
	let result = executor.execute(Box::new(data)).unwrap();
	let result = dyn_any::downcast::<Vec<_O>>(result).unwrap();
	*result
	*/
	todo!()
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
