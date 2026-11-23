use crate::Rect;
use glam::IVec2;
use std::ops::{Index, IndexMut};

/// How many grid cells a chain steps left for each of its nodes.
pub const CHAIN_WIDTH: i32 = 7;
pub(crate) const CHAIN_OFFSET: IVec2 = IVec2::new(-CHAIN_WIDTH, 0);
pub(crate) const LAYER_BOTTOM_GAP: i32 = 1;

/// The position of a node in `Graph::nodes`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIndex(u32);

impl NodeIndex {
	#[must_use]
	pub fn index(self) -> usize {
		self.0 as usize
	}
}

impl From<u32> for NodeIndex {
	fn from(index: u32) -> Self {
		Self(index)
	}
}

impl From<NodeIndex> for u32 {
	fn from(node: NodeIndex) -> Self {
		node.0
	}
}

/// Where a node is stored, in grid cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
	/// At this position, starting a tree of its own.
	Absolute(IVec2),
	/// At this offset from the left of the nodes reading it, level with the highest of them.
	Relative(IVec2),
}

impl Placement {
	pub(crate) fn shifted(self, shift: IVec2) -> Self {
		match self {
			Self::Absolute(position) => Self::Absolute(position + shift),
			Self::Relative(offset) => Self::Relative(offset + shift),
		}
	}

	pub(crate) fn stored_offset(self) -> IVec2 {
		match self {
			Self::Absolute(_) => IVec2::ZERO,
			Self::Relative(offset) => offset,
		}
	}

	pub(crate) fn root_position(self) -> IVec2 {
		match self {
			Self::Absolute(position) | Self::Relative(position) => position,
		}
	}
}

/// How a node takes part in the layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
	Node,
	/// Reads a stack through input 0 and a chain through input 1.
	Layer,
	/// A point wires are routed through, like a redirect dot, which takes no room and is never moved to make room.
	Waypoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
	pub placement: Placement,
	pub kind: NodeKind,
	/// The width and height in grid cells.
	pub size: IVec2,
	/// Whether the node can be drawn in a layer's chain.
	pub chainable: bool,
	/// Whether the user is moving the node, so it and the connections carrying it are drawn as stored.
	pub held: bool,
}

impl Node {
	pub(crate) fn is_layer(&self) -> bool {
		self.kind == NodeKind::Layer
	}

	pub(crate) fn chain_input(&self) -> u32 {
		u32::from(self.is_layer())
	}

	pub(crate) fn can_join_chain(&self) -> bool {
		self.chainable && self.kind == NodeKind::Node
	}

	pub(crate) fn outline(&self) -> Option<Rect> {
		let bottom_gap = if self.is_layer() { LAYER_BOTTOM_GAP } else { 0 };
		(self.kind != NodeKind::Waypoint).then(|| Rect::new(IVec2::ZERO, self.size + IVec2::new(0, bottom_gap)))
	}
}

/// The output `output` of node `source` read by input `input` of node `target`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wire {
	pub source: NodeIndex,
	pub output: u32,
	pub target: NodeIndex,
	pub input: u32,
}

impl Wire {
	pub(crate) fn is_primary(&self) -> bool {
		self.output == 0
	}
}

/// The nodes and wires of a graph as the layout reads them.
#[derive(Clone, Debug, Default)]
pub struct Graph {
	pub nodes: Vec<Node>,
	pub wires: Vec<Wire>,
}

impl Graph {
	pub(crate) fn node_indices(&self) -> impl DoubleEndedIterator<Item = NodeIndex> + use<> {
		(0..u32::try_from(self.nodes.len()).unwrap_or(u32::MAX)).map(NodeIndex)
	}

	pub(crate) fn feeds_stack(&self, wire: &Wire) -> bool {
		self[wire.target].is_layer() && wire.input == 0
	}

	pub(crate) fn drawing_order(&self, wire: &Wire) -> u32 {
		if self.feeds_stack(wire) { u32::MAX } else { wire.input }
	}
}

impl Index<NodeIndex> for Graph {
	type Output = Node;

	fn index(&self, node: NodeIndex) -> &Node {
		&self.nodes[node.index()]
	}
}

impl IndexMut<NodeIndex> for Graph {
	fn index_mut(&mut self, node: NodeIndex) -> &mut Node {
		&mut self.nodes[node.index()]
	}
}
