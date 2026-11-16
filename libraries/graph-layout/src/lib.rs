//! Lays out a node graph in one pass: every node is placed after all the nodes reading it, then its bounds, ports, and wires follow from its position.

use glam::IVec2;

/// How many grid cells a chain steps left for each node.
pub const CHAIN_WIDTH: i32 = 7;
/// How many grid cells tall a layer is.
pub const LAYER_HEIGHT: i32 = 2;

/// Where a node sits, as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
	Absolute(IVec2),
	/// One chain width left of the node reading it through that node's chain input.
	Chain,
	/// Below the node reading it, past that node's height and a gap of this many cells.
	Stack(u32),
	/// An offset from the top left corner of the positions of the nodes reading it, or from the origin when none does.
	Relative(IVec2),
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
	pub placement: Placement,
	pub is_layer: bool,
	/// The grid rows a node's ports take up, which sets its height unless it is a layer.
	pub rows: u32,
	/// The width in grid cells, measured by the caller.
	pub width: u32,
	pub inputs: u32,
	pub outputs: u32,
	/// Whether the node can be drawn in a layer's chain, which redirect dots and nodes with a hidden primary output can't.
	pub chainable: bool,
}

impl Node {
	pub fn height(&self) -> i32 {
		if self.is_layer { LAYER_HEIGHT } else { self.rows as i32 + 1 }
	}

	/// The input a chain enters this node through.
	fn chain_input(&self) -> u32 {
		if self.is_layer { 1 } else { 0 }
	}
}

/// The output `output` of node `source` read by input `input` of node `target`.
#[derive(Clone, Copy, Debug)]
pub struct Wire {
	pub source: u32,
	pub output: u32,
	pub target: u32,
	pub input: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
	pub nodes: Vec<Node>,
	pub wires: Vec<Wire>,
}

/// The wires of a graph grouped by one of their ends, so each node's group is one contiguous slice.
struct WiresByNode {
	starts: Vec<u32>,
	wires: Vec<u32>,
}

impl WiresByNode {
	fn grouped(node_count: usize, wires: &[Wire], end: impl Fn(&Wire) -> u32) -> Self {
		let mut starts = vec![0_u32; node_count + 1];
		for wire in wires {
			starts[end(wire) as usize + 1] += 1;
		}
		for node in 0..node_count {
			starts[node + 1] += starts[node];
		}

		let mut filled = starts.clone();
		let mut grouped = vec![0_u32; wires.len()];
		for (index, wire) in wires.iter().enumerate() {
			let slot = &mut filled[end(wire) as usize];
			grouped[*slot as usize] = index as u32;
			*slot += 1;
		}

		Self { starts, wires: grouped }
	}

	fn of(&self, node: usize) -> &[u32] {
		&self.wires[self.starts[node] as usize..self.starts[node + 1] as usize]
	}
}

/// Where everything of a graph is drawn, in grid cells.
#[derive(Clone, Debug, Default)]
pub struct Layout {
	pub positions: Vec<IVec2>,
	/// Each node's top left and bottom right corner.
	pub bounds: Vec<[IVec2; 2]>,
	/// Each wire's output port and input port.
	pub wire_ends: Vec<[IVec2; 2]>,
	/// The layer whose chain each node is drawn in, if any.
	pub chain_layers: Vec<Option<u32>>,
	/// Nodes on a cycle, placed only from the readers placed before them.
	pub cyclic: Vec<u32>,
}

impl Layout {
	/// The topmost node whose bounds contain `point`.
	pub fn node_at(&self, point: IVec2) -> Option<u32> {
		let contains = |[top_left, bottom_right]: &[IVec2; 2]| point.cmpge(*top_left).all() && point.cmplt(*bottom_right).all();
		self.bounds.iter().rposition(contains).map(|node| node as u32)
	}
}

pub fn layout(graph: &Graph) -> Layout {
	let node_count = graph.nodes.len();
	let readers = WiresByNode::grouped(node_count, &graph.wires, |wire| wire.source);
	let inputs = WiresByNode::grouped(node_count, &graph.wires, |wire| wire.target);

	let mut positions = vec![IVec2::ZERO; node_count];
	let mut placed = vec![false; node_count];
	let mut unplaced_readers: Vec<u32> = (0..node_count).map(|node| readers.of(node).len() as u32).collect();
	let mut ready: Vec<u32> = (0..node_count as u32).filter(|node| unplaced_readers[*node as usize] == 0).collect();

	let mut place = |node: usize, positions: &mut Vec<IVec2>, placed: &mut Vec<bool>, ready: &mut Vec<u32>| {
		positions[node] = position(graph, node, readers.of(node), positions, placed);
		placed[node] = true;

		for wire in inputs.of(node) {
			let source = graph.wires[*wire as usize].source as usize;
			unplaced_readers[source] = unplaced_readers[source].saturating_sub(1);
			if unplaced_readers[source] == 0 && !placed[source] {
				ready.push(source as u32);
			}
		}
	};

	while let Some(node) = ready.pop() {
		place(node as usize, &mut positions, &mut placed, &mut ready);
	}

	let mut cyclic = Vec::new();
	for node in 0..node_count {
		if placed[node] {
			continue;
		}
		cyclic.push(node as u32);
		place(node, &mut positions, &mut placed, &mut ready);
		while let Some(node) = ready.pop() {
			place(node as usize, &mut positions, &mut placed, &mut ready);
		}
	}

	let bounds = graph
		.nodes
		.iter()
		.zip(&positions)
		.map(|(node, position)| [*position, *position + IVec2::new(node.width as i32, node.height())])
		.collect();
	let wire_ends = graph
		.wires
		.iter()
		.map(|wire| {
			let output = output_port(&graph.nodes[wire.source as usize], positions[wire.source as usize], wire.output);
			let input = input_port(&graph.nodes[wire.target as usize], positions[wire.target as usize], wire.input);
			[output, input]
		})
		.collect();
	let chain_layers = chain_layers(graph, &inputs, &readers, &positions);

	Layout {
		positions,
		bounds,
		wire_ends,
		chain_layers,
		cyclic,
	}
}

/// Walks back from each layer through the chain input of each node in turn, while the node it reaches is chainable,
/// is read only by that node through its primary output, and sits one chain width to its left.
fn chain_layers(graph: &Graph, inputs: &WiresByNode, readers: &WiresByNode, positions: &[IVec2]) -> Vec<Option<u32>> {
	let mut chain_layers = vec![None; graph.nodes.len()];

	for layer in (0..graph.nodes.len()).filter(|node| graph.nodes[*node].is_layer) {
		let mut reader = layer;
		loop {
			let chain_input = graph.nodes[reader].chain_input();
			let Some(wire) = inputs.of(reader).iter().map(|wire| graph.wires[*wire as usize]).find(|wire| wire.input == chain_input) else {
				break;
			};

			let source = wire.source as usize;
			let primary_readers = readers.of(source).iter().filter(|wire| graph.wires[**wire as usize].output == 0).count();
			let beside = positions[source] == positions[reader] - IVec2::new(CHAIN_WIDTH, 0);
			let joins = graph.nodes[source].chainable && !graph.nodes[source].is_layer && wire.output == 0 && primary_readers == 1 && beside;
			if !joins || chain_layers[source].is_some() {
				break;
			}

			chain_layers[source] = Some(layer as u32);
			reader = source;
		}
	}

	chain_layers
}

/// Where `node` sits, given the wires reading it and the positions of the readers placed so far.
fn position(graph: &Graph, node: usize, readers: &[u32], positions: &[IVec2], placed: &[bool]) -> IVec2 {
	let placed_readers = readers.iter().map(|wire| graph.wires[*wire as usize]).filter(|wire| placed[wire.target as usize]);

	match graph.nodes[node].placement {
		Placement::Absolute(position) => position,
		Placement::Relative(offset) => placed_readers.map(|wire| positions[wire.target as usize]).reduce(IVec2::min).unwrap_or(IVec2::ZERO) + offset,
		Placement::Chain => {
			let mut chain_readers = placed_readers.filter(|wire| wire.output == 0 && wire.input == graph.nodes[wire.target as usize].chain_input());
			chain_readers.next().map_or(IVec2::ZERO, |wire| positions[wire.target as usize] - IVec2::new(CHAIN_WIDTH, 0))
		}
		Placement::Stack(gap) => {
			let mut stack_readers = placed_readers.filter(|wire| wire.output == 0);
			stack_readers.next().map_or(IVec2::ZERO, |wire| {
				let below = graph.nodes[wire.target as usize].height() + 1 + gap as i32;
				positions[wire.target as usize] + IVec2::new(0, below)
			})
		}
	}
}

fn input_port(node: &Node, position: IVec2, input: u32) -> IVec2 {
	match (node.is_layer, input) {
		(true, 0) => position + IVec2::new(2, LAYER_HEIGHT),
		(true, _) => position + IVec2::new(0, 1),
		(false, input) => position + IVec2::new(0, 1 + input as i32),
	}
}

fn output_port(node: &Node, position: IVec2, output: u32) -> IVec2 {
	if node.is_layer {
		position + IVec2::new(2, 0)
	} else {
		position + IVec2::new(node.width as i32, 1 + output as i32)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn node(placement: Placement, is_layer: bool) -> Node {
		Node {
			placement,
			is_layer,
			rows: 1,
			width: 5,
			inputs: 2,
			outputs: 1,
			chainable: true,
		}
	}

	fn wire(source: u32, target: u32, input: u32) -> Wire {
		Wire { source, output: 0, target, input }
	}

	#[test]
	fn placements_resolve_from_their_readers() {
		let graph = Graph {
			nodes: vec![
				node(Placement::Absolute(IVec2::new(30, 10)), true),
				node(Placement::Stack(1), true),
				node(Placement::Chain, false),
				node(Placement::Chain, false),
				node(Placement::Relative(IVec2::new(-7, 3)), false),
			],
			wires: vec![wire(1, 0, 0), wire(2, 0, 1), wire(3, 2, 0), wire(4, 2, 1), wire(4, 1, 1)],
		};

		let layout = layout(&graph);

		assert_eq!(layout.positions[1], IVec2::new(30, 10 + LAYER_HEIGHT + 1 + 1));
		assert_eq!(layout.positions[2], IVec2::new(30 - CHAIN_WIDTH, 10));
		assert_eq!(layout.positions[3], IVec2::new(30 - 2 * CHAIN_WIDTH, 10));
		assert_eq!(layout.positions[4], IVec2::new(30 - CHAIN_WIDTH - 7, 10 + 3));
		assert!(layout.cyclic.is_empty());

		// The chained nodes join the upper layer's chain, but the relative node is read twice and sits below
		assert_eq!(layout.chain_layers, vec![None, None, Some(0), Some(0), None]);
	}

	#[test]
	fn a_node_beside_a_layer_joins_its_chain_however_it_is_placed() {
		let graph = Graph {
			nodes: vec![
				node(Placement::Absolute(IVec2::new(30, 10)), true),
				node(Placement::Absolute(IVec2::new(30 - CHAIN_WIDTH, 10)), false),
				node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 0)), false),
				node(Placement::Absolute(IVec2::new(0, 0)), false),
			],
			wires: vec![wire(1, 0, 1), wire(2, 1, 0), wire(3, 2, 0)],
		};

		let layout = layout(&graph);

		assert_eq!(layout.chain_layers, vec![None, Some(0), Some(0), None]);
	}

	#[test]
	fn a_cycle_still_places_every_node() {
		let graph = Graph {
			nodes: vec![node(Placement::Relative(IVec2::new(-7, 0)), false), node(Placement::Relative(IVec2::new(-7, 0)), false)],
			wires: vec![wire(0, 1, 0), wire(1, 0, 0)],
		};

		let layout = layout(&graph);

		assert_eq!(layout.cyclic.len(), 1);
		assert_eq!(layout.positions.len(), 2);
	}

	#[test]
	fn the_topmost_node_is_hit() {
		let graph = Graph {
			nodes: vec![node(Placement::Absolute(IVec2::ZERO), false), node(Placement::Absolute(IVec2::new(2, 0)), false)],
			wires: Vec::new(),
		};

		let layout = layout(&graph);

		assert_eq!(layout.node_at(IVec2::new(3, 1)), Some(1));
		assert_eq!(layout.node_at(IVec2::new(1, 1)), Some(0));
		assert_eq!(layout.node_at(IVec2::new(20, 20)), None);
	}
}
