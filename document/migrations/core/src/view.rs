use crate::{Declaration, DeclarationResolver, InputConnector, MigrationError, NewNode, OutputConnector};
use document_graph_storage::{InputSlot, NetworkId, NodeId, NodeInput, Registry, TimeStamp};
use std::ops::Deref;

/// Which part of the registry around an anchor a view reads or edits. A migration's footprint is the union of its views' regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region(u8);

impl Region {
	pub const EMPTY: Self = Self(0);
	/// The anchor itself, which the runner switches to the target declaration.
	pub const ANCHOR: Self = Self(1);
	pub const INPUTS: Self = Self(1 << 1);
	pub const UPSTREAM: Self = Self(1 << 2);
	pub const DOWNSTREAM: Self = Self(1 << 3);
	pub const RESOURCES: Self = Self(1 << 4);
	/// The anchor's whole network, as read through [`Graph`].
	pub const NETWORK: Self = Self(1 << 5);

	pub const fn union(self, other: Self) -> Self {
		Self(self.0 | other.0)
	}

	pub const fn overlaps(self, other: Self) -> bool {
		self.0 & other.0 != 0
	}

	/// Whether no two of `regions` overlap, for checking a migration's mutable views at compile time.
	pub const fn disjoint(regions: &[Region]) -> bool {
		let mut i = 0;
		while i < regions.len() {
			let mut j = i + 1;
			while j < regions.len() {
				if regions[i].overlaps(regions[j]) {
					return false;
				}
				j += 1;
			}
			i += 1;
		}
		true
	}
}

/// The anchor before migration. Wraps the stored node to add its ID, which the node doesn't hold itself.
#[derive(Clone, Copy)]
pub struct Node<'a> {
	id: NodeId,
	node: &'a document_graph_storage::Node,
}

impl Node<'_> {
	pub fn id(&self) -> NodeId {
		self.id
	}
}

impl Deref for Node<'_> {
	type Target = document_graph_storage::Node;

	fn deref(&self) -> &Self::Target {
		self.node
	}
}

/// What one run of a migration sees: the source state around one anchor.
#[derive(Clone, Copy)]
pub struct AnchorContext<'a> {
	registry: &'a Registry,
	anchor: Node<'a>,
	source: Declaration,
	target: Declaration,
	resolver: &'a dyn DeclarationResolver,
}

impl<'a> AnchorContext<'a> {
	/// `None` when `anchor` isn't a live node of `registry`.
	pub fn new(registry: &'a Registry, anchor: NodeId, source: Declaration, target: Declaration, resolver: &'a dyn DeclarationResolver) -> Option<Self> {
		let node = registry.node_instances.get(&anchor)?;

		Some(Self {
			registry,
			anchor: Node { id: anchor, node },
			source,
			target,
			resolver,
		})
	}

	pub fn source_node(&self) -> &Node<'a> {
		&self.anchor
	}

	pub fn graph(&self) -> Graph<'a> {
		Graph {
			registry: self.registry,
			network: self.anchor.network(),
			resolver: self.resolver,
		}
	}

	/// The ID of a node this run adds. It is the same in every state the migration runs on, so a history rewrite diffs it as one node.
	fn derived_node_id(&self, declaration: Declaration, key: &str) -> NodeId {
		let mut hasher = blake3::Hasher::new();
		postcard::to_io(&(self.source, self.target, self.anchor.id.0, declaration, key), &mut hasher).expect("hashing never fails to write");

		let mut truncated = [0_u8; 8];
		truncated.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
		NodeId(u64::from_le_bytes(truncated))
	}

	fn added_node(&self, node: NewNode, key: &str) -> AddedNode {
		AddedNode {
			id: self.derived_node_id(node.declaration, key),
			key: key.to_string(),
			node,
		}
	}
}

/// Read access to the anchor's network before migration.
#[derive(Clone, Copy)]
pub struct Graph<'a> {
	registry: &'a Registry,
	network: NetworkId,
	resolver: &'a dyn DeclarationResolver,
}

impl<'a> Graph<'a> {
	pub fn network(&self) -> NetworkId {
		self.network
	}

	/// A node of this network.
	pub fn node(&self, id: NodeId) -> Option<&'a document_graph_storage::Node> {
		self.registry.node_instances.get(&id).filter(|node| node.network() == self.network)
	}

	/// The declaration a node of this network implements, if it is one a migration knows.
	pub fn declaration(&self, id: NodeId) -> Option<Declaration> {
		let document_graph_storage::Implementation::ProtoNode(resource) = self.node(id)?.implementation() else {
			return None;
		};
		self.resolver.declaration(*resource)
	}

	/// Every slot reading `output`: node inputs ordered by node ID and slot, then the network's exports.
	pub fn consumers(&self, output: OutputConnector) -> Vec<InputConnector> {
		self.readers(|node, index| node == output.node && index == output.output)
	}

	/// Every slot reading any output of `node`.
	fn consumers_of_node(&self, node: NodeId) -> Vec<InputConnector> {
		self.readers(|read_node, _| read_node == node)
	}

	fn readers(&self, reads: impl Fn(NodeId, u32) -> bool) -> Vec<InputConnector> {
		let reads_input = |input: &NodeInput| matches!(input, NodeInput::Node { id, index } if reads(*id, *index));

		let mut readers: Vec<InputConnector> = self
			.registry
			.node_instances
			.iter()
			.filter(|(_, node)| node.network() == self.network)
			.flat_map(|(id, node)| {
				node.inputs()
					.iter()
					.enumerate()
					.filter(|(_, slot)| reads_input(&slot.input))
					.map(|(input, _)| InputConnector::Node { node: *id, input: input as u32 })
			})
			.collect();
		readers.sort_unstable();

		if let Some(network) = self.registry.networks.get(&self.network) {
			let exports = network.exports.iter().enumerate().filter(|(_, slot)| slot.target.as_ref().is_some_and(reads_input));
			readers.extend(exports.map(|(index, _)| InputConnector::Export {
				network: self.network,
				index: index as u32,
			}));
		}

		readers
	}
}

/// A parameter type of a migration: a window onto one region around the anchor that records edits rather than applying them.
pub trait View<'a>: Sized {
	const REGION: Region;
	fn extract(context: &AnchorContext<'a>) -> Self;
	fn into_edits(self) -> Result<Edits, MigrationError>;
}

/// Everything one run of a migration asks the runner to change.
#[derive(Debug, Default)]
pub struct Edits {
	pub inputs: Option<InputsEdit>,
	pub added_nodes: Vec<AddedNode>,
	pub removed_nodes: Vec<NodeId>,
	pub connections: Vec<(OutputConnector, InputConnector)>,
}

impl Edits {
	pub fn merge(&mut self, other: Edits) {
		if other.inputs.is_some() {
			self.inputs = other.inputs;
		}
		self.added_nodes.extend(other.added_nodes);
		self.removed_nodes.extend(other.removed_nodes);
		self.connections.extend(other.connections);
	}
}

#[derive(Debug)]
pub struct AddedNode {
	pub id: NodeId,
	/// Disambiguates several nodes of one declaration added for one anchor.
	pub key: String,
	pub node: NewNode,
}

/// The anchor's target input layout.
#[derive(Debug)]
pub struct InputsEdit {
	pub slots: Vec<InputSlot>,
	/// For each target slot, the source slot it still holds unchanged, or `None` for an inserted or overwritten slot.
	pub origins: Vec<Option<u32>>,
}

impl InputsEdit {
	/// Where the unchanged source slot `source` ended up, or `None` if it was removed or overwritten.
	pub fn target_index(&self, source: u32) -> Option<u32> {
		self.origins.iter().position(|origin| *origin == Some(source)).map(|index| index as u32)
	}
}

/// The anchor's input slots. Operations apply in order to the target layout being built.
pub struct Inputs {
	slots: Vec<InputSlot>,
	origins: Vec<Option<u32>>,
	edited: bool,
	error: Option<MigrationError>,
}

impl Inputs {
	pub fn len(&self) -> usize {
		self.slots.len()
	}

	pub fn is_empty(&self) -> bool {
		self.slots.is_empty()
	}

	pub fn set(&mut self, index: u32, input: NodeInput) {
		if !self.check(index, self.slots.len()) {
			return;
		}

		self.slots[index as usize].input = input;
		self.origins[index as usize] = None;
	}

	pub fn insert(&mut self, index: u32, input: NodeInput) {
		if !self.check(index, self.slots.len() + 1) {
			return;
		}

		self.slots.insert(index as usize, new_slot(input));
		self.origins.insert(index as usize, None);
	}

	pub fn push(&mut self, input: NodeInput) {
		self.edited = true;
		self.slots.push(new_slot(input));
		self.origins.push(None);
	}

	pub fn remove(&mut self, index: u32) {
		if !self.check(index, self.slots.len()) {
			return;
		}

		self.slots.remove(index as usize);
		self.origins.remove(index as usize);
	}

	pub fn truncate(&mut self, len: u32) {
		self.edited = true;
		self.slots.truncate(len as usize);
		self.origins.truncate(len as usize);
	}

	/// Whether `index` is below `bound`, recording the first violation for `into_edits` to report.
	fn check(&mut self, index: u32, bound: usize) -> bool {
		self.edited = true;

		let in_range = (index as usize) < bound;
		if !in_range && self.error.is_none() {
			self.error = Some(MigrationError::SlotOutOfRange { index, len: self.slots.len() });
		}
		in_range
	}
}

fn new_slot(input: NodeInput) -> InputSlot {
	InputSlot {
		input,
		..InputSlot::unset(TimeStamp::ORIGIN)
	}
}

impl<'a> View<'a> for Inputs {
	const REGION: Region = Region::INPUTS;

	fn extract(context: &AnchorContext<'a>) -> Self {
		let slots = context.anchor.inputs().to_vec();
		let origins = (0..slots.len() as u32).map(Some).collect();

		Self {
			slots,
			origins,
			edited: false,
			error: None,
		}
	}

	fn into_edits(self) -> Result<Edits, MigrationError> {
		if let Some(error) = self.error {
			return Err(error);
		}

		let inputs = self.edited.then_some(InputsEdit {
			slots: self.slots,
			origins: self.origins,
		});
		Ok(Edits { inputs, ..Default::default() })
	}
}

/// Nodes upstream of the anchor: new ones that feed its inputs, and existing ones to remove.
pub struct Upstream<'a> {
	context: AnchorContext<'a>,
	added: Vec<AddedNode>,
	removed: Vec<NodeId>,
}

impl Upstream<'_> {
	/// Add a node in the anchor's network. Needs no key while it is the only one of its declaration this run adds.
	pub fn add_node(&mut self, node: NewNode) -> NodeId {
		self.add_node_keyed(node, "")
	}

	pub fn add_node_keyed(&mut self, node: NewNode, key: &str) -> NodeId {
		let added = self.context.added_node(node, key);
		let id = added.id;
		self.added.push(added);
		id
	}

	/// Only correct when nothing else reads the removed node, which the region can't check.
	pub fn remove_node(&mut self, id: NodeId) {
		self.removed.push(id);
	}
}

impl<'a> View<'a> for Upstream<'a> {
	const REGION: Region = Region::UPSTREAM;

	fn extract(context: &AnchorContext<'a>) -> Self {
		Self {
			context: *context,
			added: Vec::new(),
			removed: Vec::new(),
		}
	}

	fn into_edits(self) -> Result<Edits, MigrationError> {
		Ok(Edits {
			added_nodes: self.added,
			removed_nodes: self.removed,
			..Default::default()
		})
	}
}

/// The slots reading the anchor's outputs, plus new nodes downstream of it.
pub struct Downstream<'a> {
	context: AnchorContext<'a>,
	added: Vec<AddedNode>,
	connections: Vec<(OutputConnector, InputConnector)>,
}

impl Downstream<'_> {
	/// The slots reading the anchor's `output` in the source state. Nodes added by this migration never appear here.
	pub fn consumers(&self, output: u32) -> Vec<InputConnector> {
		let anchor = OutputConnector { node: self.context.anchor.id, output };
		self.context.graph().consumers(anchor)
	}

	/// Add a node in the anchor's network. Needs no key while it is the only one of its declaration this run adds.
	pub fn add_node(&mut self, node: NewNode) -> NodeId {
		self.add_node_keyed(node, "")
	}

	pub fn add_node_keyed(&mut self, node: NewNode, key: &str) -> NodeId {
		let added = self.context.added_node(node, key);
		let id = added.id;
		self.added.push(added);
		id
	}

	/// Make `consumer`, which must read an output of the anchor, read `output` instead.
	pub fn connect(&mut self, output: OutputConnector, consumer: InputConnector) {
		self.connections.push((output, consumer));
	}
}

impl<'a> View<'a> for Downstream<'a> {
	const REGION: Region = Region::DOWNSTREAM;

	fn extract(context: &AnchorContext<'a>) -> Self {
		Self {
			context: *context,
			added: Vec::new(),
			connections: Vec::new(),
		}
	}

	fn into_edits(self) -> Result<Edits, MigrationError> {
		let readers = self.context.graph().consumers_of_node(self.context.anchor.id);
		if let Some((_, consumer)) = self.connections.iter().find(|(_, consumer)| !readers.contains(consumer)) {
			return Err(MigrationError::OutsideRegion(format!("{consumer:?}")));
		}

		Ok(Edits {
			added_nodes: self.added,
			connections: self.connections,
			..Default::default()
		})
	}
}
