//! A port of `interpreted_executor::dynamic_executor` with mock nodes standing in for the real node constructors.

use super::NodeId;
use super::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};
use super::types::Type;
use super::typing::{Registry, TypingContext};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub trait MockNode: Send + Sync {
	fn input_count(&self) -> usize;
}

pub type SharedNode = Arc<dyn MockNode>;

/// Stands in for the boxed downcast wrapper each registry constructor puts around an input.
struct DowncastInput(#[expect(dead_code, reason = "the mock only models the allocation, it never evaluates")] SharedNode);

struct ConstructedNode {
	inputs: Vec<Box<DowncastInput>>,
}

impl MockNode for ConstructedNode {
	fn input_count(&self) -> usize {
		self.inputs.len()
	}
}

struct ValueNode(#[expect(dead_code, reason = "the mock only models the allocation, it never evaluates")] super::document::MemoValue);

impl MockNode for ValueNode {
	fn input_count(&self) -> usize {
		0
	}
}

/// Stands in for `ObservedNode`, which wraps every node with a slot for the last evaluated context.
struct ObservedNode {
	node: Box<dyn MockNode>,
	_last_context: Mutex<Option<Arc<()>>>,
}

impl MockNode for ObservedNode {
	fn input_count(&self) -> usize {
		self.node.input_count()
	}
}

pub type NodeConstructor = fn(Vec<SharedNode>) -> Box<dyn MockNode>;

pub fn construct(inputs: Vec<SharedNode>) -> Box<dyn MockNode> {
	Box::new(ConstructedNode {
		inputs: inputs.into_iter().map(|input| Box::new(DowncastInput(input))).collect(),
	})
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeTypes {
	pub inputs: Vec<Type>,
	pub output: Type,
}

type Path = Box<[NodeId]>;

#[derive(Default)]
pub struct BorrowTree {
	nodes: HashMap<NodeId, (SharedNode, Path, Box<[NodeId]>)>,
	source_map: HashMap<Path, (NodeId, NodeTypes)>,
	context_features: HashMap<NodeId, u32>,
}

impl BorrowTree {
	pub fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<Self, String> {
		let mut tree = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			tree.push_node(id, node, typing_context)?
		}
		Ok(tree)
	}

	pub fn len(&self) -> usize {
		self.nodes.len()
	}

	fn update_source_map(&mut self, id: NodeId, typing_context: &TypingContext, proto_node: &ProtoNode) -> bool {
		let Some(node_io) = typing_context.type_of(id) else { return false };
		let inputs = [&node_io.call_argument].into_iter().chain(&node_io.inputs).cloned().collect();

		let node_path = proto_node.original_location.path.as_ref().unwrap_or(const { &vec![] });
		let entry = self.source_map.entry(node_path.to_vec().into()).or_default();

		let update = (
			id,
			NodeTypes {
				inputs,
				output: node_io.return_value.clone(),
			},
		);
		let modified = *entry != update;
		*entry = update;
		modified
	}

	fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), String> {
		self.update_source_map(id, typing_context, &proto_node);
		self.context_features.insert(id, proto_node.upstream_context_features);
		let path: Path = proto_node.original_location.path.clone().unwrap_or_default().into();

		let observe = |node: Box<dyn MockNode>| -> SharedNode {
			Arc::new(ObservedNode {
				node,
				_last_context: Mutex::new(None),
			})
		};

		match &proto_node.construction_args {
			ConstructionArgs::Value(value) => {
				let node = observe(Box::new(ValueNode(value.clone())));
				self.nodes.insert(id, (node, path, Box::default()));
			}
			ConstructionArgs::Nodes(ids) => {
				let ids = ids.to_vec();
				let mut construction_nodes: Vec<SharedNode> = ids.iter().map(|node| self.nodes.get(node).unwrap().0.clone()).collect();

				if let Some(promotions) = typing_context.promotions(id) {
					for (argument_index, promotion) in promotions {
						let adapter_constructor = typing_context.adapter_constructor(&promotion.adapter_identifier()).ok_or("No promotion adapter constructor")?;
						let adapter = adapter_constructor(vec![construction_nodes[*argument_index].clone()]);
						construction_nodes[*argument_index] = Arc::from(adapter);
					}
				}

				let constructor = typing_context.constructor(id).ok_or("No constructor")?;
				let node = observe(constructor(construction_nodes));
				self.nodes.insert(id, (node, path, ids.into()));
			}
		}
		Ok(())
	}
}

pub struct DynamicExecutor<'a> {
	pub tree: BorrowTree,
	pub output: NodeId,
	pub typing_context: TypingContext<'a>,
	pub sources: Vec<u64>,
}

impl<'a> DynamicExecutor<'a> {
	pub fn new(proto_network: ProtoNetwork, registry: &'a Registry) -> Result<Self, String> {
		let mut typing_context = TypingContext::new(registry);
		typing_context.update(&proto_network)?;
		let output = proto_network.output;
		let sources = proto_network
			.nodes
			.iter()
			.filter_map(|(_, node)| match &node.construction_args {
				ConstructionArgs::Value(value) => value.source_id(),
				_ => None,
			})
			.collect();
		let tree = BorrowTree::new(proto_network, &typing_context)?;

		Ok(Self {
			tree,
			output,
			typing_context,
			sources,
		})
	}

	pub fn output_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.return_value.clone())
	}
}
