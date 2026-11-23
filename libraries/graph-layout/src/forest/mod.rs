mod readers_first;

use crate::connections::Connections;
use crate::groups::Groups;
use crate::per_node::PerNode;
use crate::{Graph, NodeIndex, Wire};
use glam::IVec2;
use readers_first::ReadersFirst;

pub(crate) struct Forest {
	pub(crate) order: Vec<NodeIndex>,
	pub(crate) owners: PerNode<Option<NodeIndex>>,
	pub(crate) owner_inputs: PerNode<u32>,
	pub(crate) offsets: PerNode<IVec2>,
	pub(crate) detached: PerNode<bool>,
	pub(crate) children_from_the_top: Groups<NodeIndex>,
	ranks: PerNode<u32>,
}

impl Forest {
	pub(crate) fn grow(graph: &Graph, connections: &Connections) -> Self {
		let mut forest = Self {
			order: Vec::with_capacity(graph.nodes.len()),
			owners: PerNode::filled(graph, None),
			owner_inputs: PerNode::filled(graph, 0),
			offsets: PerNode::filled(graph, IVec2::ZERO),
			detached: PerNode::filled(graph, false),
			children_from_the_top: Groups::new(graph, std::iter::empty(), |node| *node),
			ranks: PerNode::filled(graph, u32::MAX),
		};
		let mut stored_positions = PerNode::filled(graph, IVec2::ZERO);
		ReadersFirst::new(graph, connections).for_each(|node| forest.add(graph, connections, node, &mut stored_positions));

		forest.children_from_the_top = forest.children_from_the_top(graph);
		forest
	}

	pub(crate) fn owner(&self, node: NodeIndex) -> Option<NodeIndex> {
		self.owners[node]
	}

	pub(crate) fn feeds_stack(&self, graph: &Graph, node: NodeIndex) -> bool {
		self.owner(node).is_some_and(|owner| graph[owner].is_layer() && self.owner_inputs[node] == 0)
	}

	pub(crate) fn anchor(&self, graph: &Graph, connections: &Connections, node: NodeIndex, position_of: impl Fn(NodeIndex) -> IVec2) -> IVec2 {
		let Some(owner) = self.owner(node) else { return IVec2::ZERO };
		let left = self
			.anchoring_wires(graph, connections, node)
			.map(|wire| position_of(wire.target).x)
			.min()
			.unwrap_or(position_of(owner).x);
		IVec2::new(left, position_of(owner).y)
	}

	fn add(&mut self, graph: &Graph, connections: &Connections, node: NodeIndex, stored_positions: &mut PerNode<IVec2>) {
		self.ranks[node] = u32::try_from(self.order.len()).unwrap_or(u32::MAX);
		self.order.push(node);

		let owner_wire = self.highest_reader(graph, connections, node, stored_positions).filter(|_| !connections.is_root(graph, node));
		stored_positions[node] = match owner_wire {
			Some(wire) => self.attach(graph, connections, wire, stored_positions),
			None => graph[node].placement.root_position(),
		};
	}

	fn attach(&mut self, graph: &Graph, connections: &Connections, owner_wire: Wire, stored_positions: &PerNode<IVec2>) -> IVec2 {
		let (node, owner) = (owner_wire.source, owner_wire.target);
		self.owners[node] = Some(owner);
		self.owner_inputs[node] = owner_wire.input;

		let stored_position = self.anchor(graph, connections, node, |reader| stored_positions[reader]) + graph[node].placement.stored_offset();
		self.offsets[node] = stored_position - stored_positions[owner];
		self.detached[node] = self.has_reader_nearer_than_owner(graph, connections, node, stored_position, stored_positions);
		stored_position
	}

	fn has_reader_nearer_than_owner(&self, graph: &Graph, connections: &Connections, node: NodeIndex, stored_position: IVec2, stored_positions: &PerNode<IVec2>) -> bool {
		let distance = |reader: NodeIndex| (stored_positions[reader] - stored_position).abs().element_sum();
		let owner_distance = self.owner(node).map_or(0, distance);
		self.anchoring_wires(graph, connections, node).any(|wire| distance(wire.target) < owner_distance)
	}

	fn highest_reader(&self, graph: &Graph, connections: &Connections, node: NodeIndex, positions: &PerNode<IVec2>) -> Option<Wire> {
		self.anchoring_wires(graph, connections, node).copied().min_by_key(|wire| {
			let position = positions[wire.target];
			(position.y, position.x, graph.drawing_order(wire))
		})
	}

	fn anchoring_wires<'a>(&'a self, graph: &'a Graph, connections: &'a Connections, node: NodeIndex) -> impl Iterator<Item = &'a Wire> {
		connections.readers(graph, node).filter(move |wire| self.ranks[wire.target] < self.ranks[node])
	}

	fn children_from_the_top(&self, graph: &Graph) -> Groups<NodeIndex> {
		let owned_in_order = self.order.iter().copied().filter(|node| self.owners[*node].is_some());
		let mut children = Groups::new(graph, owned_in_order, |node| self.owner(*node).unwrap_or_default());
		children.sort_each_by_key(|node| {
			let offset = self.offsets[*node];
			(offset.y, offset.x)
		});
		children
	}
}
