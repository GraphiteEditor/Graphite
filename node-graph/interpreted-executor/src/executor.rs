use std::error::Error;

use borrow_stack::{BorrowStack, FixedSizeStack};
use graphene_core::Node;
use graphene_std::any::{Any, TypeErasedNode};

use graph_craft::{executor::Executor, proto::ProtoNetwork};
use crate::node_registry::push_node;

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
