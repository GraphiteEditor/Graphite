use graph_craft::document::*;
use graphene_core::raster::*;
use graphene_core::value::ValueNode;
use graphene_core::*;

use bytemuck::Pod;
use core::marker::PhantomData;
use dyn_any::StaticTypeSized;

pub struct MapGpuNode<O, Network> {
	network: Network,
	_o: PhantomData<O>,
}

#[node_macro::node_fn(MapGpuNode<_O>)]
fn map_gpu<I: IntoIterator<Item = S>, S: StaticTypeSized + Sync + Send + Pod, _O: StaticTypeSized + Sync + Send + Pod>(input: I, network: &'any_input NodeNetwork) -> Vec<_O> {
	use graph_craft::executor::Executor;
	let bytes = compilation_client::compile_sync::<S, _O>(network.clone()).unwrap();
	let words = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u32, bytes.len() / 4) };
	use wgpu_executor::{Context, GpuExecutor};
	let executor: GpuExecutor<S, _O> = GpuExecutor::new(Context::new_sync().unwrap(), words.into(), "gpu::eval".into()).unwrap();
	let data: Vec<_> = input.into_iter().collect();
	let result = executor.execute(Box::new(data)).unwrap();
	let result = dyn_any::downcast::<Vec<_O>>(result).unwrap();
	*result
}

pub struct MapGpuSingleImageNode<N> {
	node: N,
}

#[node_macro::node_fn(MapGpuSingleImageNode)]
fn map_gpu_single_image(input: Image, node: String) -> Image {
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
				name: "Image filter Node".into(),
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
