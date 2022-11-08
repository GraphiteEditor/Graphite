use std::error::Error;

use borrow_stack::{BorrowStack, FixedSizeStack};
use graphene_core::Node;
use graphene_std::any::{Any, TypeErasedNode};

use crate::{document::NodeNetwork, node_registry::push_node, proto::ProtoNetwork};

pub struct Compiler {}

impl Compiler {
	pub fn compile(&self, mut network: NodeNetwork, resolve_inputs: bool) -> ProtoNetwork {
		let node_count = network.nodes.len();
		println!("flattening");
		for id in 0..node_count {
			network.flatten(id as u64);
		}
		let mut proto_network = network.into_proto_network();
		if resolve_inputs {
			println!("resolving inputs");
			proto_network.resolve_inputs();
		}
		println!("reordering ids");
		proto_network.reorder_ids();
		proto_network
	}
}

pub trait Executor {
	fn execute(&self, input: Any<'static>) -> Result<Any<'static>, Box<dyn Error>>;
}

pub struct DynamicExecutor {
	stack: FixedSizeStack<TypeErasedNode<'static>>,
}

impl DynamicExecutor {
	pub fn new(proto_network: ProtoNetwork) -> Self {
		assert_eq!(proto_network.inputs.len(), 1);
		let node_count = proto_network.nodes.len();
		let stack = FixedSizeStack::new(node_count);
		for (_id, node) in proto_network.nodes {
			push_node(node, &stack);
		}
		Self { stack }
	}
}

impl Executor for DynamicExecutor {
	fn execute(&self, input: Any<'static>) -> Result<Any<'static>, Box<dyn Error>> {
		let result = unsafe { self.stack.get().last().unwrap().eval(input) };
		Ok(result)
	}
}
