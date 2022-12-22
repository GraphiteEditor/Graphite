use bytemuck::Pod;
use core::marker::PhantomData;
use dyn_any::StaticTypeSized;
use graph_craft::document::*;
use graph_craft::proto::*;
use graphene_core::{raster::Image, value::ValueNode, Node};

pub struct MapGpuNode<NN: Node<()>, I: IntoIterator<Item = S>, S: StaticTypeSized + Sync + Send + Pod, O: StaticTypeSized + Sync + Send + Pod>(pub NN, PhantomData<(S, I, O)>);

impl<'n, I: IntoIterator<Item = S>, NN: Node<(), Output = &'n NodeNetwork> + Copy, S: StaticTypeSized + Sync + Send + Pod, O: StaticTypeSized + Sync + Send + Pod> Node<I>
	for &MapGpuNode<NN, I, S, O>
{
	type Output = Vec<O>;
	fn eval(self, input: I) -> Self::Output {
		let network = self.0.eval(());

		use graph_craft::executor::Compiler;
		use graph_craft::executor::Executor;
		let compiler = Compiler {};
		let bytes = gpu_compiler_bin_wrapper::compile_spirv(&network, std::any::type_name::<S>(), std::any::type_name::<O>(), None);

		use vulkan_executor::{Context, GpuExecutor};
		let executor: GpuExecutor<S, O> = GpuExecutor::new(Context::new(), &bytes.unwrap(), "gpu::eval".into()).unwrap();

		let data: Vec<_> = input.into_iter().collect();
		let result = executor.execute(Box::new(data)).unwrap();
		let result = dyn_any::downcast::<Vec<O>>(result).unwrap();
		*result
	}
}
impl<'n, I: IntoIterator<Item = S>, NN: Node<(), Output = &'n NodeNetwork> + Copy, S: StaticTypeSized + Sync + Send + Pod, O: StaticTypeSized + Sync + Send + Pod> Node<I> for MapGpuNode<NN, I, S, O> {
	type Output = Vec<O>;
	fn eval(self, input: I) -> Self::Output {
		let network = self.0.eval(());

		use graph_craft::executor::Compiler;
		use graph_craft::executor::Executor;
		let compiler = Compiler {};
		let bytes = gpu_compiler_bin_wrapper::compile_spirv(&network, std::any::type_name::<S>(), std::any::type_name::<O>(), None);

		use vulkan_executor::{Context, GpuExecutor};
		let executor: GpuExecutor<S, O> = GpuExecutor::new(Context::new(), &bytes.unwrap(), "gpu::eval".into()).unwrap();

		let data: Vec<_> = input.into_iter().collect();
		let result = executor.execute(Box::new(data)).unwrap();
		let result = dyn_any::downcast::<Vec<O>>(result).unwrap();
		*result
	}
}

impl<I: IntoIterator<Item = S>, NN: Node<()>, S: StaticTypeSized + Sync + Pod + Send, O: StaticTypeSized + Sync + Send + Pod> MapGpuNode<NN, I, S, O> {
	pub const fn new(network: NN) -> Self {
		MapGpuNode(network, PhantomData)
	}
}

pub struct MapGpuSingleImageNode<NN: Node<(), Output = String>>(pub NN);

impl<NN: Node<(), Output = String> + Copy> Node<Image> for MapGpuSingleImageNode<NN> {
	type Output = Image;
	fn eval(self, input: Image) -> Self::Output {
		let node = self.0.eval(());
		use graph_craft::document::*;

		let identifier = NodeIdentifier {
			name: std::borrow::Cow::Owned(node),
			types: std::borrow::Cow::Borrowed(&[]),
		};

		let network = NodeNetwork {
			inputs: vec![0],
			output: 0,
			nodes: [(
				0,
				DocumentNode {
					name: "Image filter Node".into(),
					inputs: vec![NodeInput::Network],
					implementation: DocumentNodeImplementation::Unresolved(identifier),
					metadata: DocumentNodeMetadata::default(),
				},
			)]
			.into_iter()
			.collect(),
		};

		let value_network = ValueNode::new(network);
		let map_node = MapGpuNode::new(&value_network);
		let data = map_node.eval(input.data.clone());
		Image { data, ..input }
	}
}

impl<NN: Node<(), Output = String> + Copy> Node<Image> for &MapGpuSingleImageNode<NN> {
	type Output = Image;
	fn eval(self, input: Image) -> Self::Output {
		let node = self.0.eval(());
		use graph_craft::document::*;

		let identifier = NodeIdentifier {
			name: std::borrow::Cow::Owned(node),
			types: std::borrow::Cow::Borrowed(&[]),
		};

		let network = NodeNetwork {
			inputs: vec![0],
			output: 0,
			nodes: [(
				0,
				DocumentNode {
					name: "Image filter Node".into(),
					inputs: vec![NodeInput::Network],
					implementation: DocumentNodeImplementation::Unresolved(identifier),
					metadata: DocumentNodeMetadata::default(),
				},
			)]
			.into_iter()
			.collect(),
		};

		let value_network = ValueNode::new(network);
		let map_node = MapGpuNode::new(&value_network);
		let data = map_node.eval(input.data.clone());
		Image { data, ..input }
	}
}
