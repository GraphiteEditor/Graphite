use crate::document::{DocumentNode, NetworkId, NetworkLocation, NodeId, NodeInput, NodeNetwork};
use core_types::Type;

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
///
/// Each delta names the network it changed by the location that network had when the write happened.
/// A path through instances would be resolved later and could reach a different network by then.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeDelta {
	/// A new entry of the definitions table. Its nodes name the networks they implement by ID, so each entry they name is added first.
	AddNetwork {
		id: NetworkId,
		network: Box<NodeNetwork>,
	},
	/// Drops the definitions table entry `id`, which no node reaches any more.
	RemoveNetwork {
		id: NetworkId,
	},
	/// The node's nested network, if it implements one, rides inside the [`DocumentNode`].
	AddNode {
		network: NetworkLocation,
		node_id: NodeId,
		node: Box<DocumentNode>,
	},
	ReplaceNode {
		network: NetworkLocation,
		node_id: NodeId,
		node: Box<DocumentNode>,
	},
	/// Address-only: removal snapshots come from the storage layer's working registry.
	RemoveNode {
		network: NetworkLocation,
		node_id: NodeId,
	},
	SetInput {
		network: NetworkLocation,
		node_id: NodeId,
		input_index: usize,
		input: NodeInput,
	},
	/// A node's whole input list, which is how a change to the number or order of its slots is
	/// expressed, since [`SetInput`](Self::SetInput) addresses one index and cannot move the others.
	SetInputs {
		network: NetworkLocation,
		node_id: NodeId,
		inputs: Vec<NodeInput>,
	},
	/// `None` removes the slot, matching the storage op this converts to.
	SetExport {
		network: NetworkLocation,
		export_index: usize,
		input: Option<NodeInput>,
	},
	SetVisibility {
		network: NetworkLocation,
		node_id: NodeId,
		visible: bool,
	},
	/// The [`Type`] of argument the node can be evaluated with.
	SetCallArgument {
		network: NetworkLocation,
		node_id: NodeId,
		call_argument: Type,
	},
}
