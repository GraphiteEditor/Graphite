//! Lays out a node graph in one pass: every node is placed after all the nodes reading it, then its bounds, ports, and wires follow from its position.

mod click_targets;
mod contour;

pub use click_targets::{ClickTarget, GRID_SIZE, Part, node_parts, reach, target_at};
use contour::{Bounds, Contour, SpanPool};
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
	/// Whether the node is a point wires are routed through, like a redirect dot, which takes no room and is never moved to make room.
	pub waypoint: bool,
	/// The width of a layer's name in pixels, measured by the caller.
	pub name_width: f64,
	pub locked: bool,
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

/// Indices, of wires or nodes, grouped by a node each belongs to, so each node's group is one contiguous slice.
struct GroupedByNode {
	starts: Vec<u32>,
	items: Vec<u32>,
}

impl GroupedByNode {
	fn grouped(node_count: usize, wires: &[Wire], end: impl Fn(&Wire) -> u32) -> Self {
		let indices: Vec<u32> = (0..wires.len() as u32).collect();
		Self::grouped_items(node_count, &indices, |index| end(&wires[*index as usize]))
	}

	/// Groups `items` by the node `group` gives each, keeping their order within a group.
	fn grouped_items(node_count: usize, items: &[u32], group: impl Fn(&u32) -> u32) -> Self {
		let mut starts = vec![0_u32; node_count + 1];
		for item in items {
			starts[group(item) as usize + 1] += 1;
		}
		for node in 0..node_count {
			starts[node + 1] += starts[node];
		}

		let mut filled = starts.clone();
		let mut grouped = vec![0_u32; items.len()];
		for item in items {
			let slot = &mut filled[group(item) as usize];
			grouped[*slot as usize] = *item;
			*slot += 1;
		}

		Self { starts, items: grouped }
	}

	fn of(&self, node: usize) -> &[u32] {
		&self.items[self.starts[node] as usize..self.starts[node + 1] as usize]
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
	/// The node each node is placed relative to, which is the first reader to reach it from the roots.
	pub owners: Vec<Option<u32>>,
	/// How far each node is drawn from its stored place so its upstream clears what its owner placed before it.
	pub elongations: Vec<IVec2>,
	/// The layer whose chain each node is drawn in, if any.
	pub chain_layers: Vec<Option<u32>>,
	/// How many nodes each layer's chain holds, and zero for any other node.
	pub chain_lengths: Vec<u32>,
	/// Nodes only reached through a cycle, placed as roots.
	pub cyclic: Vec<u32>,
}

impl Layout {
	/// The topmost node whose bounds contain `point`.
	pub fn node_at(&self, point: IVec2) -> Option<u32> {
		let contains = |[top_left, bottom_right]: &[IVec2; 2]| point.cmpge(*top_left).all() && point.cmplt(*bottom_right).all();
		self.bounds.iter().rposition(contains).map(|node| node as u32)
	}
}

/// Rows kept between the outlines of the upstream of two inputs of a node, beyond the empty row a layer keeps below it.
const VERTICAL_GAP: i32 = 0;
/// The empty row a layer keeps below it, which the layer stacked under it starts after.
const LAYER_BOTTOM_GAP: i32 = 1;
/// Empty columns kept between a node and the upstream feeding it from the left, which a chain's spacing leaves exactly.
const HORIZONTAL_GAP: i32 = CHAIN_WIDTH - 5;

/// Lays out the graph as a forest: a depth-first walk from the roots gives each node to the first reader reaching it,
/// each node's upstream is then fitted together below and left of it, and finally every node is placed from its owner.
pub fn layout(graph: &Graph) -> Layout {
	let node_count = graph.nodes.len();
	let readers = GroupedByNode::grouped(node_count, &graph.wires, |wire| wire.source);
	let inputs = GroupedByNode::grouped(node_count, &graph.wires, |wire| wire.target);

	let forest = Forest::walk(graph, &readers, &inputs);
	let offsets: Vec<IVec2> = (0..node_count).map(|node| stored_offset(graph, node, forest.owners[node])).collect();
	let elongations = elongations(graph, &forest, &offsets);

	let mut positions = vec![IVec2::ZERO; node_count];
	for &node in &forest.preorder {
		let node = node as usize;
		positions[node] = match forest.owners[node] {
			Some(owner) => positions[owner as usize] + offsets[node] + elongations[node],
			None => root_position(&graph.nodes[node]),
		};
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
	let mut chain_lengths = vec![0; node_count];
	for layer in chain_layers.iter().flatten() {
		chain_lengths[*layer as usize] += 1;
	}

	Layout {
		positions,
		bounds,
		wire_ends,
		owners: forest.owners,
		elongations,
		chain_layers,
		chain_lengths,
		cyclic: forest.cyclic,
	}
}

/// The graph as a forest of the wires along which a depth-first walk first reached each node.
struct Forest {
	owners: Vec<Option<u32>>,
	/// The input of its owner each node is reached through.
	owner_inputs: Vec<u32>,
	/// Each node after its owner and before anything it owns.
	preorder: Vec<u32>,
	/// The nodes each node owns, grouped by owner, in the order the walk reached them.
	children: GroupedByNode,
	cyclic: Vec<u32>,
}

impl Forest {
	fn walk(graph: &Graph, readers: &GroupedByNode, inputs: &GroupedByNode) -> Self {
		let node_count = graph.nodes.len();
		let mut owners = vec![None; node_count];
		let mut owner_inputs = vec![0; node_count];
		let mut preorder = Vec::with_capacity(node_count);
		let mut cyclic = Vec::new();
		let mut reached = vec![false; node_count];

		// The wires into each node in the order they are drawn from the top: a node's inputs top to bottom, and a layer's stack input after the rest
		let mut ordered_inputs = inputs.items.clone();
		for node in 0..node_count {
			let is_layer = graph.nodes[node].is_layer;
			let range = inputs.starts[node] as usize..inputs.starts[node + 1] as usize;
			ordered_inputs[range].sort_unstable_by_key(|wire| {
				let input = graph.wires[*wire as usize].input;
				if is_layer && input == 0 { u32::MAX } else { input }
			});
		}

		// Placed nodes start their own trees, walked from the top of the graph down
		let is_root = |node: usize| readers.of(node).is_empty() || matches!(graph.nodes[node].placement, Placement::Absolute(_));
		let mut roots: Vec<usize> = (0..node_count).filter(|node| is_root(*node)).collect();
		roots.sort_by_key(|node| {
			let position = root_position(&graph.nodes[*node]);
			(position.y, position.x, *node)
		});

		// Each entry is a node and the index in `ordered_inputs` of the next of its inputs to visit
		let mut stack: Vec<(usize, usize)> = Vec::new();
		let mut walk_from = |root: usize, reached: &mut [bool], preorder: &mut Vec<u32>| {
			if reached[root] {
				return;
			}
			reached[root] = true;
			preorder.push(root as u32);

			stack.push((root, inputs.starts[root] as usize));
			while let Some((node, next)) = stack.last_mut() {
				let node = *node;
				if *next == inputs.starts[node + 1] as usize {
					stack.pop();
					continue;
				}
				let wire = graph.wires[ordered_inputs[*next] as usize];
				*next += 1;

				let source = wire.source as usize;
				if reached[source] || is_root(source) {
					continue;
				}
				reached[source] = true;
				owners[source] = Some(node as u32);
				owner_inputs[source] = wire.input;
				preorder.push(source as u32);
				stack.push((source, inputs.starts[source] as usize));
			}
		};

		for root in roots {
			walk_from(root, &mut reached, &mut preorder);
		}
		for node in 0..node_count {
			if !reached[node] {
				cyclic.push(node as u32);
				walk_from(node, &mut reached, &mut preorder);
			}
		}

		// Grouping the owned nodes in preorder keeps each owner's in the order the walk reached them
		let owned_in_preorder: Vec<u32> = preorder.iter().copied().filter(|node| owners[*node as usize].is_some()).collect();
		let children = GroupedByNode::grouped_items(node_count, &owned_in_preorder, |node| owners[*node as usize].unwrap_or_default());

		Self {
			owners,
			owner_inputs,
			preorder,
			children,
			cyclic,
		}
	}
}

fn root_position(node: &Node) -> IVec2 {
	match node.placement {
		Placement::Absolute(position) | Placement::Relative(position) => position,
		Placement::Chain | Placement::Stack(_) => IVec2::ZERO,
	}
}

/// Where a node sits from its owner as stored, before anything is fitted.
fn stored_offset(graph: &Graph, node: usize, owner: Option<u32>) -> IVec2 {
	let Some(owner) = owner else { return IVec2::ZERO };
	match graph.nodes[node].placement {
		Placement::Relative(offset) => offset,
		Placement::Chain => IVec2::new(-CHAIN_WIDTH, 0),
		Placement::Stack(gap) => IVec2::new(0, graph.nodes[owner as usize].height() + 1 + gap as i32),
		Placement::Absolute(_) => IVec2::ZERO,
	}
}

/// Fits each node's upstream together, from the deepest nodes up. The nodes it owns are taken from the top as stored:
/// each is moved left of it if it would cover it, or below it when it feeds a layer's stack input, then down past the
/// outlines of those taken before it. A link in a chain is never moved, so a chain stays whole and gives way only at
/// its end, and a waypoint stays where it is put.
fn elongations(graph: &Graph, forest: &Forest, offsets: &[IVec2]) -> Vec<IVec2> {
	let node_count = graph.nodes.len();
	let mut elongations = vec![IVec2::ZERO; node_count];
	let mut contours: Vec<Contour> = vec![Contour::default(); node_count];
	let mut pool = SpanPool::default();
	let chain_links = chain_links(graph, forest, offsets);

	let mut children_from_the_top = forest.children.items.clone();
	for node in 0..node_count {
		let range = forest.children.starts[node] as usize..forest.children.starts[node + 1] as usize;
		children_from_the_top[range].sort_by_key(|child| (offsets[*child as usize].y, offsets[*child as usize].x));
	}

	for &node in forest.preorder.iter().rev() {
		let node = node as usize;
		let own = own_bounds(&graph.nodes[node]);
		let mut placed = Contour::default();

		let children = &children_from_the_top[forest.children.starts[node] as usize..forest.children.starts[node + 1] as usize];
		for &child in children {
			let child = child as usize;
			let mut contour = std::mem::take(&mut contours[child]).moved(offsets[child]);
			let mut elongation = IVec2::ZERO;

			if !graph.nodes[child].waypoint && !chain_links[child] {
				let feeds_stack = graph.nodes[node].is_layer && forest.owner_inputs[child] == 0;
				let own_clearance = match own {
					None => IVec2::ZERO,
					Some(own) if feeds_stack => IVec2::new(0, contour.clearance_below_bounds(own, VERTICAL_GAP, HORIZONTAL_GAP)),
					Some(own) => IVec2::new(-contour.clearance_left_of_bounds(own, VERTICAL_GAP, HORIZONTAL_GAP), 0),
				};
				contour = contour.moved(own_clearance);

				let sibling_clearance = IVec2::new(0, contour.clearance_below(&placed, VERTICAL_GAP, HORIZONTAL_GAP));
				contour = contour.moved(sibling_clearance);
				elongation = own_clearance + sibling_clearance;
			}

			elongations[child] = elongation;
			placed = placed.union(contour, &mut pool);
		}

		if let Some(own) = own {
			placed.cover_bounds(own, &mut pool);
		}
		contours[node] = placed;
	}

	elongations
}

/// The cells a node takes up, with the empty row a layer keeps below it, or none for a waypoint.
fn own_bounds(node: &Node) -> Option<Bounds> {
	let height = if node.is_layer { node.height() + LAYER_BOTTOM_GAP } else { node.height() };
	(!node.waypoint).then_some(Bounds {
		min: IVec2::ZERO,
		max: IVec2::new(node.width as i32, height),
	})
}

/// Whether each node sits as a link in a chain, one chain width left of the layer or chain link it feeds through its chain input.
fn chain_links(graph: &Graph, forest: &Forest, offsets: &[IVec2]) -> Vec<bool> {
	let mut chain_links = vec![false; graph.nodes.len()];

	// Each owner comes before what it owns, so whether the owner is in a chain is already known
	for &node in &forest.preorder {
		let node = node as usize;
		let Some(owner) = forest.owners[node] else { continue };
		let owner = owner as usize;

		let feeds_chain_input = forest.owner_inputs[node] == graph.nodes[owner].chain_input();
		let beside = offsets[node] == IVec2::new(-CHAIN_WIDTH, 0);
		let owner_in_chain = graph.nodes[owner].is_layer || chain_links[owner];
		chain_links[node] = graph.nodes[node].chainable && !graph.nodes[node].is_layer && feeds_chain_input && beside && owner_in_chain;
	}

	chain_links
}

/// Walks back from each layer through the chain input of each node in turn, while the node it reaches is chainable,
/// is read only by that node through its primary output, and sits one chain width to its left.
fn chain_layers(graph: &Graph, inputs: &GroupedByNode, readers: &GroupedByNode, positions: &[IVec2]) -> Vec<Option<u32>> {
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
			waypoint: false,
			name_width: 60.,
			locked: false,
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

	/// A layer at the top with a chain of two nodes, the far one fed from below by a node three rows tall, and a stacked layer with a chain of its own.
	fn stack_with_tall_content(tall_node_reader: u32, tall_node_column: i32) -> Graph {
		Graph {
			nodes: vec![
				node(Placement::Absolute(IVec2::new(30, 10)), true),
				node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 0)), false),
				node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 0)), false),
				Node {
					rows: 3,
					..node(Placement::Relative(IVec2::new(tall_node_column, 3)), false)
				},
				node(Placement::Relative(IVec2::new(0, LAYER_HEIGHT + 1)), true),
				node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 0)), false),
			],
			wires: vec![wire(1, 0, 1), wire(2, 1, 0), wire(3, tall_node_reader, 1), wire(4, 0, 0), wire(5, 4, 1)],
		}
	}

	#[test]
	fn a_stacked_layer_is_drawn_below_the_content_of_the_layer_above() {
		let graph = stack_with_tall_content(1, 0);
		let layout = layout(&graph);

		// The tall node covers rows 13 to 16 under the stacked layer's chain, so the stacked layer moves below it
		assert_eq!(layout.positions[3], IVec2::new(30 - CHAIN_WIDTH, 13));
		assert_eq!(layout.positions[4], IVec2::new(30, 13 + 4));
		assert_eq!(layout.elongations[4], IVec2::new(0, 4));
		assert_eq!(layout.positions[5], layout.positions[4] - IVec2::new(CHAIN_WIDTH, 0));
	}

	#[test]
	fn a_stacked_layer_fits_beside_content_further_left() {
		let graph = stack_with_tall_content(2, 0);
		let layout = layout(&graph);

		// The tall node hangs below the far end of the chain, which leaves the columns of the stacked layer's chain free
		assert_eq!(layout.positions[3], IVec2::new(30 - 2 * CHAIN_WIDTH, 13));
		assert_eq!(layout.elongations[4], IVec2::ZERO);
	}

	#[test]
	fn a_chain_is_never_drawn_apart() {
		let graph = stack_with_tall_content(1, 0);
		let layout = layout(&graph);

		assert_eq!(layout.elongations[1], IVec2::ZERO);
		assert_eq!(layout.elongations[2], IVec2::ZERO);
		assert_eq!(layout.chain_layers, vec![None, Some(0), Some(0), None, None, Some(4)]);
	}

	#[test]
	fn a_node_covering_its_reader_is_drawn_left_of_it() {
		let graph = Graph {
			nodes: vec![node(Placement::Absolute(IVec2::new(30, 10)), false), node(Placement::Relative(IVec2::new(-2, 1)), false)],
			wires: vec![wire(1, 0, 1)],
		};
		let layout = layout(&graph);

		assert_eq!(layout.positions[1], IVec2::new(30 - 5 - HORIZONTAL_GAP, 11));
	}

	#[test]
	fn a_waypoint_takes_no_room_and_stays_where_it_is_put() {
		let waypoint = Node {
			waypoint: true,
			chainable: false,
			..node(Placement::Relative(IVec2::new(-1, 1)), false)
		};
		let graph = Graph {
			nodes: vec![
				node(Placement::Absolute(IVec2::new(30, 10)), false),
				waypoint,
				node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 0)), false),
			],
			wires: vec![wire(1, 0, 0), wire(2, 1, 0)],
		};
		let layout = layout(&graph);

		assert_eq!(layout.positions[1], IVec2::new(29, 11));
		assert_eq!(layout.positions[2], IVec2::new(29 - CHAIN_WIDTH, 11));
	}

	#[test]
	fn a_shared_node_belongs_to_the_first_reader_reaching_it() {
		let graph = Graph {
			nodes: vec![
				node(Placement::Absolute(IVec2::new(30, 20)), false),
				node(Placement::Absolute(IVec2::new(30, 10)), false),
				node(Placement::Relative(IVec2::new(-CHAIN_WIDTH, 0)), false),
			],
			wires: vec![wire(2, 0, 0), wire(2, 1, 0)],
		};
		let layout = layout(&graph);

		// The upper reader is the first root walked from
		assert_eq!(layout.owners[2], Some(1));
		assert_eq!(layout.positions[2], IVec2::new(30 - CHAIN_WIDTH, 10));
	}

	#[test]
	fn a_fitted_layout_stored_as_offsets_needs_no_further_fitting() {
		let graph = stack_with_tall_content(1, 0);
		let first = layout(&graph);

		let mut refitted = graph.clone();
		for (node, data) in refitted.nodes.iter_mut().enumerate() {
			if let Some(owner) = first.owners[node] {
				data.placement = Placement::Relative(first.positions[node] - first.positions[owner as usize]);
			}
		}
		let second = layout(&refitted);

		assert_eq!(second.positions, first.positions);
		assert!(second.elongations.iter().all(|elongation| *elongation == IVec2::ZERO));
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

	#[test]
	fn a_port_takes_precedence_over_its_node_body() {
		let graph = Graph {
			nodes: vec![node(Placement::Absolute(IVec2::ZERO), false)],
			wires: Vec::new(),
		};

		let layout = layout(&graph);
		let at = |x: f64, y: f64| target_at(&graph, &layout, glam::DVec2::new(x, y)).map(|target| target.part);

		assert_eq!(at(2., GRID_SIZE), Some(Part::Input(0)));
		assert_eq!(at(60., GRID_SIZE), Some(Part::Body));
		assert_eq!(at(60., 200.), None);
	}

	#[test]
	fn a_rounded_corner_is_not_hit() {
		let graph = Graph {
			nodes: vec![Node {
				rows: 3,
				..node(Placement::Absolute(IVec2::ZERO), false)
			}],
			wires: Vec::new(),
		};
		let layout = layout(&graph);
		let body_bottom = GRID_SIZE / 2. + 3. * GRID_SIZE;
		let at = |x: f64, y: f64| target_at(&graph, &layout, glam::DVec2::new(x, y)).map(|target| target.part);

		assert_eq!(at(0.1, body_bottom - 0.1), None);
		assert_eq!(at(3., body_bottom - 3.), Some(Part::Body));
	}

	#[test]
	fn every_part_lies_within_its_nodes_reach() {
		let locked_layer = Node {
			locked: true,
			inputs: 2,
			width: 9,
			..node(Placement::Absolute(IVec2::new(30, 10)), true)
		};
		let tall_node = Node {
			rows: 1,
			inputs: 6,
			outputs: 3,
			..node(Placement::Chain, false)
		};
		let graph = Graph {
			nodes: vec![locked_layer, tall_node, node(Placement::Stack(0), true)],
			wires: vec![wire(1, 0, 1), wire(2, 0, 0)],
		};
		let layout = layout(&graph);

		for index in 0..graph.nodes.len() {
			let [min, max] = reach(&graph, &layout, index);
			node_parts(&graph, &layout, index, |target| {
				assert!(target.min.cmpge(min).all() && target.max.cmple(max).all(), "{target:?} leaves the reach {min} to {max}");
			});
		}
	}
}

#[cfg(test)]
mod phase_timing {
	use super::*;
	use std::time::Instant;

	fn node(placement: Placement, is_layer: bool) -> Node {
		Node {
			placement,
			is_layer,
			rows: 2,
			width: 5,
			inputs: 3,
			outputs: 1,
			chainable: true,
			waypoint: false,
			name_width: 60.,
			locked: false,
		}
	}

	fn wire(source: usize, target: usize, input: u32) -> Wire {
		Wire {
			source: source as u32,
			output: 0,
			target: target as u32,
			input,
		}
	}

	fn stacked_layers(node_count: usize) -> Graph {
		let mut graph = Graph::default();
		let mut previous_layer = None;
		while graph.nodes.len() + 4 <= node_count {
			let layer = graph.nodes.len();
			let placement = if previous_layer.is_some() { Placement::Stack(0) } else { Placement::Absolute(IVec2::ZERO) };
			graph.nodes.push(node(placement, true));
			if let Some(previous_layer) = previous_layer {
				graph.wires.push(wire(layer, previous_layer, 0));
			}
			let mut reader = (layer, 1);
			for _ in 0..3 {
				let chained = graph.nodes.len();
				graph.nodes.push(node(Placement::Chain, false));
				graph.wires.push(wire(chained, reader.0, reader.1));
				reader = (chained, 0);
			}
			previous_layer = Some(layer);
		}
		graph
	}

	fn random_graph(node_count: usize) -> Graph {
		let mut state = 0x9E37_79B9_7F4A_7C15_u64;
		let mut below = |bound: usize| {
			state ^= state << 13;
			state ^= state >> 7;
			state ^= state << 17;
			(state % bound as u64) as usize
		};
		let mut graph = Graph::default();
		for index in 0..node_count {
			if index == 0 || below(100) == 0 {
				graph.nodes.push(node(Placement::Absolute(IVec2::new(below(1_000) as i32, below(1_000) as i32)), false));
				continue;
			}
			graph.nodes.push(node(Placement::Relative(IVec2::new(-7, below(7) as i32)), false));
			for input in 0..1 + below(3) {
				graph.wires.push(wire(index, below(index), input as u32));
			}
		}
		graph
	}

	#[test]
	#[ignore]
	fn time_each_phase() {
		for (name, graph) in [("stacked_layers", stacked_layers(100_000)), ("random", random_graph(100_000))] {
			let repetitions = 10;
			let mut times = [std::time::Duration::ZERO; 7];
			for _ in 0..repetitions {
				let node_count = graph.nodes.len();
				let start = Instant::now();
				let readers = GroupedByNode::grouped(node_count, &graph.wires, |wire| wire.source);
				let inputs = GroupedByNode::grouped(node_count, &graph.wires, |wire| wire.target);
				times[0] += start.elapsed();

				let start = Instant::now();
				let forest = Forest::walk(&graph, &readers, &inputs);
				times[1] += start.elapsed();

				let start = Instant::now();
				let offsets: Vec<IVec2> = (0..node_count).map(|node| stored_offset(&graph, node, forest.owners[node])).collect();
				let elongations = elongations(&graph, &forest, &offsets);
				times[2] += start.elapsed();

				let start = Instant::now();
				let mut positions = vec![IVec2::ZERO; node_count];
				for &node in &forest.preorder {
					let node = node as usize;
					positions[node] = match forest.owners[node] {
						Some(owner) => positions[owner as usize] + offsets[node] + elongations[node],
						None => root_position(&graph.nodes[node]),
					};
				}
				times[3] += start.elapsed();

				let start = Instant::now();
				let wire_ends: Vec<[IVec2; 2]> = graph
					.wires
					.iter()
					.map(|wire| {
						[
							output_port(&graph.nodes[wire.source as usize], positions[wire.source as usize], wire.output),
							input_port(&graph.nodes[wire.target as usize], positions[wire.target as usize], wire.input),
						]
					})
					.collect();
				std::hint::black_box(wire_ends);
				times[4] += start.elapsed();

				let start = Instant::now();
				std::hint::black_box(chain_layers(&graph, &inputs, &readers, &positions));
				times[5] += start.elapsed();

				let start = Instant::now();
				std::hint::black_box(layout(&graph));
				times[6] += start.elapsed();
			}
			let [grouping, walk, fitting, placing, wires, chains, total] = times.map(|time| time / repetitions);
			println!("{name:15} grouping {grouping:>8.1?} | walk {walk:>8.1?} | fitting {fitting:>8.1?} | placing {placing:>8.1?} | wire ends {wires:>8.1?} | chains {chains:>8.1?} | whole layout {total:>8.1?}");
		}
	}
}
