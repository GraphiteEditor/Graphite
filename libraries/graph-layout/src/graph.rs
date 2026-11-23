use crate::Rect;
use glam::IVec2;
use std::ops::{Index, IndexMut};

pub const CHAIN_WIDTH: i32 = 7;
pub(crate) const CHAIN_OFFSET: IVec2 = IVec2::new(-CHAIN_WIDTH, 0);
pub(crate) const LAYER_BOTTOM_GAP: i32 = 1;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
	Absolute(IVec2),
	Chain,
	Stack(u32),
	Relative(IVec2),
}

impl Placement {
	pub(crate) fn shifted(self, shift: IVec2) -> Option<Self> {
		match self {
			Self::Absolute(position) => Some(Self::Absolute(position + shift)),
			Self::Relative(offset) => Some(Self::Relative(offset + shift)),
			Self::Chain | Self::Stack(_) => None,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
	Node,
	Layer,
	Waypoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
	pub placement: Placement,
	pub kind: NodeKind,
	pub size: IVec2,
	pub chainable: bool,
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

	pub(crate) fn root_position(&self) -> IVec2 {
		match self.placement {
			Placement::Absolute(position) | Placement::Relative(position) => position,
			Placement::Chain | Placement::Stack(_) => IVec2::ZERO,
		}
	}

	pub(crate) fn stored_offset(&self, owner: &Node) -> IVec2 {
		match self.placement {
			Placement::Relative(offset) => offset,
			Placement::Chain => CHAIN_OFFSET,
			Placement::Stack(gap) => IVec2::new(0, owner.size.y + LAYER_BOTTOM_GAP + gap.cast_signed()),
			Placement::Absolute(_) => IVec2::ZERO,
		}
	}

	pub(crate) fn outline(&self) -> Option<Rect> {
		let bottom_gap = if self.is_layer() { LAYER_BOTTOM_GAP } else { 0 };
		(self.kind != NodeKind::Waypoint).then(|| Rect::new(IVec2::ZERO, self.size + IVec2::new(0, bottom_gap)))
	}
}

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
