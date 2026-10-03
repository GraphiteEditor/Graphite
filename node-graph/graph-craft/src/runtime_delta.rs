use crate::document::{DocumentNode, NodeId, NodeInput};
use core_types::{ContextDependencies, Type};

/// One mutation's worth of structural graph change, carrying its post-change data as plain runtime
/// types. Constructed by the mutation itself; a compound mutation emits several. Consumed typed and
/// unserialized by the compiler, and paired with `EditorDelta` metadata for storage staging.
///
/// Every variant names a write that a peer actually made, never a difference observed between two
/// states: a delta derived by comparison would assert values nobody wrote, and its timestamp would
/// win against a concurrent peer that did write them.
///
/// A node's inputs are addressed by index, which is stable only while its arity is. Changing arity is
/// therefore a [`SetInputs`](Self::SetInputs), which restates the whole list, rather than a
/// [`SetInput`](Self::SetInput) naming one index.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeDelta {
	/// The node's nested network, if it implements one, rides inside the [`DocumentNode`].
	AddNode {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		node: Box<DocumentNode>,
	},
	ReplaceNode {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		node: Box<DocumentNode>,
	},
	/// Address-only: removal snapshots come from the storage layer's working registry.
	RemoveNode {
		network_path: Vec<NodeId>,
		node_id: NodeId,
	},
	SetInput {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		input_index: usize,
		input: NodeInput,
	},
	/// A node's whole input list, which is how a change to the number or order of its slots is
	/// expressed, since [`SetInput`](Self::SetInput) addresses one index and cannot move the others.
	SetInputs {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		inputs: Vec<NodeInput>,
	},
	/// `None` removes the slot, matching the storage op this converts to.
	SetExport {
		network_path: Vec<NodeId>,
		export_index: usize,
		input: Option<NodeInput>,
	},
	SetVisibility {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		visible: bool,
	},
	/// The [`Type`] of argument the node can be evaluated with.
	SetCallArgument {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		call_argument: Type,
	},
	/// The Extract and Inject annotations the node declares for the Context, as [`ContextDependencies`].
	SetContextFeatures {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		context_features: ContextDependencies,
	},
}
