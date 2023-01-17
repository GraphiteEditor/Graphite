use std::error::Error;
use std::{collections::HashMap, sync::Arc};

use borrow_stack::{BorrowStack, FixedSizeStack};
use graph_craft::document::value::Value;
use graph_craft::document::NodeId;
use graph_craft::proto::{ConstructionArgs, ProtoNode, ProtoNodeInput};
use graphene_core::Node;
use graphene_std::any::{Any, IntoTypeErasedNode, TypeErasedNode};

use crate::node_registry::push_node;
use graph_craft::{executor::Executor, proto::ProtoNetwork};

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

#[derive(Default)]
struct BorrowTree {
	nodes: HashMap<NodeId, Arc<TypeErasedNode<'static>>>,
}

impl BorrowTree {
	pub fn new(proto_network: ProtoNetwork) -> Self {
		let mut nodes = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node)
		}
		nodes
	}
	fn node_refs(&self, nodes: &[NodeId]) -> Vec<&Arc<TypeErasedNode<'static>>> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap()).collect()
	}

	pub fn push_node(&mut self, id: NodeId, proto_node: ProtoNode) {
		assert_eq!(
			proto_node.input,
			ProtoNodeInput::None,
			"Only nodes without inputs are supported. Any inputs should already be resolved by placing ComposeNodes"
		);

		let ProtoNode { input, construction_args, identifier } = proto_node;

		match construction_args {
			ConstructionArgs::Value(value) => {
				let node = graphene_core::generic::FnNode::new(move |_| value.clone().up_box() as Any<'static>);

				let node = node.into_type_erased();
				self.nodes.insert(id, Arc::new(node));
			}
			ConstructionArgs::Nodes(ids) => {
				let construction_nodes = self.node_refs(ids.as_slice());
			}
		}

		/*
		if let Some((_id, f)) = self.nodes.get(proto_node.identifier) {
			f(proto_node, stack);
		} else {
			let other_types = NODE_REGISTRY
				.iter()
				.map(|(id, _)| id)
				.filter(|id| id.name.as_ref() == proto_node.identifier.name.as_ref())
				.collect::<Vec<_>>();
			panic!(
				"NodeImplementation: {:?} not found in Registry. Types for which the node is implemented:\n {:#?}",
				proto_node.identifier, other_types
			);
		}*/
	}
}
