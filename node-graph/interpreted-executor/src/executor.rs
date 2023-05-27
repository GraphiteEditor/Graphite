use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, RwLock};

use dyn_any::StaticType;
use graph_craft::document::value::UpcastNode;
use graph_craft::document::NodeId;
use graph_craft::executor::Executor;
use graph_craft::proto::{ConstructionArgs, LocalFuture, ProtoNetwork, ProtoNode, TypingContext};
use graph_craft::Type;
use graphene_std::any::{Any, TypeErasedPinned, TypeErasedPinnedRef};

use crate::node_registry;

#[derive(Debug, Clone)]
pub struct DynamicExecutor {
	output: NodeId,
	tree: BorrowTree,
	typing_context: TypingContext,
	// This allows us to keep the nodes around for one more frame which is used for introspection
	orphaned_nodes: Vec<NodeId>,
}

impl Default for DynamicExecutor {
	fn default() -> Self {
		Self {
			output: Default::default(),
			tree: Default::default(),
			typing_context: TypingContext::new(&node_registry::NODE_REGISTRY),
			orphaned_nodes: Vec::new(),
		}
	}
}

impl DynamicExecutor {
	pub async fn new(proto_network: ProtoNetwork) -> Result<Self, String> {
		let mut typing_context = TypingContext::new(&node_registry::NODE_REGISTRY);
		typing_context.update(&proto_network)?;
		let output = proto_network.output;
		let tree = BorrowTree::new(proto_network, &typing_context).await?;

		Ok(Self {
			tree,
			output,
			typing_context,
			orphaned_nodes: Vec::new(),
		})
	}

	pub async fn update(&mut self, proto_network: ProtoNetwork) -> Result<(), String> {
		self.output = proto_network.output;
		self.typing_context.update(&proto_network)?;
		trace!("setting output to {}", self.output);
		let mut orphans = self.tree.update(proto_network, &self.typing_context).await?;
		core::mem::swap(&mut self.orphaned_nodes, &mut orphans);
		for node_id in orphans {
			if self.orphaned_nodes.contains(&node_id) {
				self.tree.free_node(node_id)
			}
		}
		Ok(())
	}

	pub fn introspect(&self, node_path: &[NodeId]) -> Option<Option<Arc<dyn std::any::Any>>> {
		self.tree.introspect(node_path)
	}

	pub fn input_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.input.clone())
	}

	pub fn output_type(&self) -> Option<Type> {
		self.typing_context.type_of(self.output).map(|node_io| node_io.output.clone())
	}
}

impl Executor for DynamicExecutor {
	fn execute<'a>(&'a self, input: Any<'a>) -> LocalFuture<Result<Any<'a>, Box<dyn Error>>> {
		Box::pin(async move { self.tree.eval_any(self.output, input).await.ok_or_else(|| "Failed to execute".into()) })
	}
}

pub struct NodeContainer<'n> {
	pub node: TypeErasedPinned<'n>,
	// the dependencies are only kept to ensure that the nodes are not dropped while still in use
	_dependencies: Vec<Arc<RwLock<NodeContainer<'static>>>>,
}

impl<'a> core::fmt::Debug for NodeContainer<'a> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeContainer").finish()
	}
}

impl<'a> NodeContainer<'a> {
	pub fn new(node: TypeErasedPinned<'a>, _dependencies: Vec<Arc<RwLock<NodeContainer<'static>>>>) -> Self {
		Self { node, _dependencies }
	}

	/// Return a static reference to the TypeErasedNode
	/// # Safety
	/// This is unsafe because the returned reference is only valid as long as the NodeContainer is alive
	pub unsafe fn erase_lifetime(self) -> NodeContainer<'static> {
		std::mem::transmute(self)
	}
}
impl NodeContainer<'static> {
	pub unsafe fn static_ref(&self) -> TypeErasedPinnedRef<'static> {
		let s = &*(self as *const Self);
		*(&s.node.as_ref() as *const TypeErasedPinnedRef<'static>)
	}
}

#[derive(Default, Debug, Clone)]
pub struct BorrowTree {
	nodes: HashMap<NodeId, Arc<RwLock<NodeContainer<'static>>>>,
	source_map: HashMap<Vec<NodeId>, NodeId>,
}

impl BorrowTree {
	pub async fn new(proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<Self, String> {
		let mut nodes = BorrowTree::default();
		for (id, node) in proto_network.nodes {
			nodes.push_node(id, node, typing_context).await?
		}
		Ok(nodes)
	}

	/// Pushes new nodes into the tree and return orphaned nodes
	pub async fn update(&mut self, proto_network: ProtoNetwork, typing_context: &TypingContext) -> Result<Vec<NodeId>, String> {
		let mut old_nodes: HashSet<_> = self.nodes.keys().copied().collect();
		for (id, node) in proto_network.nodes {
			if !self.nodes.contains_key(&id) {
				self.push_node(id, node, typing_context).await?;
			} else {
				let Some(node_container) = self.nodes.get_mut(&id) else { continue };
				let mut node_container_writer = node_container.write().unwrap();
				let node = node_container_writer.node.as_mut();
				node.reset();
			}
			old_nodes.remove(&id);
		}
		self.source_map.retain(|_, nid| !old_nodes.contains(nid));
		Ok(old_nodes.into_iter().collect())
	}

	fn node_refs(&self, nodes: &[NodeId]) -> Vec<TypeErasedPinnedRef<'static>> {
		self.node_deps(nodes).into_iter().map(|node| unsafe { node.read().unwrap().static_ref() }).collect()
	}
	fn node_deps(&self, nodes: &[NodeId]) -> Vec<Arc<RwLock<NodeContainer<'static>>>> {
		nodes.iter().map(|node| self.nodes.get(node).unwrap().clone()).collect()
	}

	fn store_node(&mut self, node: Arc<RwLock<NodeContainer<'static>>>, id: NodeId) -> Arc<RwLock<NodeContainer<'static>>> {
		self.nodes.insert(id, node.clone());
		node
	}

	pub fn introspect(&self, node_path: &[NodeId]) -> Option<Option<Arc<dyn std::any::Any>>> {
		let id = self.source_map.get(node_path)?;
		let node = self.nodes.get(id)?;
		let reader = node.read().unwrap();
		let node = reader.node.as_ref();
		Some(node.serialize())
	}

	pub fn get(&self, id: NodeId) -> Option<Arc<RwLock<NodeContainer<'static>>>> {
		self.nodes.get(&id).cloned()
	}

	pub async fn eval<'i, I: StaticType + 'i + Send + Sync, O: StaticType + Send + Sync + 'i>(&'i self, id: NodeId, input: I) -> Option<O> {
		let node = self.nodes.get(&id).cloned()?;
		let reader = node.read().unwrap();
		let output = reader.node.eval(Box::new(input));
		dyn_any::downcast::<O>(output.await).ok().map(|o| *o)
	}
	pub async fn eval_any<'i>(&'i self, id: NodeId, input: Any<'i>) -> Option<Any<'i>> {
		let node = self.nodes.get(&id)?;
		// TODO: Comments by @TrueDoctor before this was merged:
		// TODO: Oof I dislike the evaluation being an unsafe operation but I guess its fine because it only is a lifetime extension
		// TODO: We should ideally let miri run on a test that evaluates the nodegraph multiple times to check if this contains any subtle UB but this looks fine for now
		Some(unsafe { (*((&*node.read().unwrap()) as *const NodeContainer)).node.eval(input).await })
	}

	pub fn free_node(&mut self, id: NodeId) {
		self.nodes.remove(&id);
	}

	pub async fn push_node(&mut self, id: NodeId, proto_node: ProtoNode, typing_context: &TypingContext) -> Result<(), String> {
		let ProtoNode {
			construction_args,
			identifier,
			document_node_path,
			..
		} = proto_node;
		self.source_map.insert(document_node_path, id);

		match construction_args {
			ConstructionArgs::Value(value) => {
				let upcasted = UpcastNode::new(value);
				let node = Box::pin(upcasted) as TypeErasedPinned<'_>;
				let node = NodeContainer { node, _dependencies: vec![] };
				let node = unsafe { node.erase_lifetime() };
				self.store_node(Arc::new(node.into()), id);
			}
			ConstructionArgs::Inline(_) => unimplemented!("Inline nodes are not supported yet"),
			ConstructionArgs::Nodes(ids) => {
				let ids: Vec<_> = ids.iter().map(|(id, _)| *id).collect();
				let construction_nodes = self.node_refs(&ids);
				let constructor = typing_context.constructor(id).ok_or(format!("No constructor found for node {:?}", identifier))?;
				let node = constructor(construction_nodes).await;
				let node = NodeContainer {
					node,
					_dependencies: self.node_deps(&ids),
				};
				let node = unsafe { node.erase_lifetime() };
				self.store_node(Arc::new(node.into()), id);
			}
		};
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use graph_craft::document::value::TaggedValue;

	use super::*;

	#[tokio::test]
	async fn push_node() {
		let mut tree = BorrowTree::default();
		let val_1_protonode = ProtoNode::value(ConstructionArgs::Value(TaggedValue::U32(2u32)), vec![]);
		tree.push_node(0, val_1_protonode, &TypingContext::default()).await.unwrap();
		let _node = tree.get(0).unwrap();
		assert_eq!(tree.eval(0, ()).await, Some(2u32));
	}
}
