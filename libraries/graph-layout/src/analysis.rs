use crate::{Drag, Graph, LAYER_BOTTOM_GAP, Layout, layout_as_stored, layout_with_elongations};
use glam::IVec2;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

/// Where every node is drawn for each drag offset in a rectangle, to find where dragging one more cell makes the layout jump.
pub struct DragField {
	/// The offset at the top left corner of the field.
	pub min: IVec2,
	/// How many offsets the field spans along each axis.
	pub size: IVec2,
	/// The nodes being dragged.
	pub dragged: Vec<u32>,
	/// The position of every node for each offset, row by row.
	positions: Vec<Vec<IVec2>>,
	/// How far the connection to each node's owner is drawn longer than stored, for each offset, row by row.
	elongations: Vec<Vec<IVec2>>,
}

impl DragField {
	/// Lays out `start` with `dragged` moved by every offset from `min` to `max`, both included.
	pub fn new(start: &Graph, dragged: Vec<u32>, min: IVec2, max: IVec2, lay_out: fn(&Graph) -> Layout) -> Self {
		let mut drag = Drag::new(start, dragged.clone(), false, lay_out);
		Self::from_layouts(dragged, min, max, |offset| drag.moved(offset).1)
	}

	/// Takes the layout for every offset from `min` to `max`, both included, from `layout_at`, row by row.
	pub fn from_layouts(dragged: Vec<u32>, min: IVec2, max: IVec2, mut layout_at: impl FnMut(IVec2) -> Layout) -> Self {
		let size = (max - min + IVec2::ONE).max(IVec2::ZERO);
		let mut positions = Vec::with_capacity((size.x * size.y) as usize);
		let mut elongations = Vec::with_capacity((size.x * size.y) as usize);
		for y in min.y..=max.y {
			for x in min.x..=max.x {
				let layout = layout_at(IVec2::new(x, y));
				positions.push(layout.positions);
				elongations.push(layout.elongations);
			}
		}

		Self {
			min,
			size,
			dragged,
			positions,
			elongations,
		}
	}

	fn index(&self, offset: IVec2) -> Option<usize> {
		let local = offset - self.min;
		let outside = local.cmplt(IVec2::ZERO).any() || local.cmpge(self.size).any();
		(!outside).then_some((local.y * self.size.x + local.x) as usize)
	}

	/// The position of every node with the dragged nodes moved by `offset`, if the field covers it.
	pub fn at(&self, offset: IVec2) -> Option<&[IVec2]> {
		self.positions.get(self.index(offset)?).map(Vec::as_slice)
	}

	/// How far the connection to each node's owner is drawn longer than stored with the dragged nodes moved by `offset`, if the field covers it.
	pub fn elongations_at(&self, offset: IVec2) -> Option<&[IVec2]> {
		self.elongations.get(self.index(offset)?).map(Vec::as_slice)
	}

	/// How many grid cells the nodes apart from the dragged ones are drawn away from where they are drawn at `reference`, summed.
	pub fn displacement(&self, offset: IVec2, reference: IVec2) -> Option<u32> {
		let (positions, reference) = (self.at(offset)?, self.at(reference)?);
		let undragged = (0..positions.len()).filter(|node| !self.dragged.contains(&(*node as u32)));
		Some(undragged.map(|node| (positions[node] - reference[node]).abs().element_sum() as u32).sum())
	}

	/// How far the layout is bent from where it is drawn at `reference`, with each node scored by how far it misses either staying or following the drag between them.
	pub fn deformation(&self, offset: IVec2, reference: IVec2) -> Option<Discontinuity> {
		Some(discontinuity(self.at(reference)?, self.at(offset)?, offset - reference, &self.dragged))
	}

	/// How many grid cells the dragged nodes are drawn away from where the drag moves them, their place at the zero offset moved by `offset`, summed.
	pub fn cursor_miss(&self, offset: IVec2) -> Option<u32> {
		let (positions, start) = (self.at(offset)?, self.at(IVec2::ZERO)?);
		Some(
			self.dragged
				.iter()
				.map(|node| (positions[*node as usize] - start[*node as usize] - offset).abs().element_sum() as u32)
				.sum(),
		)
	}

	/// How much the layout jumps when the drag goes one `step` further from `offset`, if the field covers both.
	pub fn step_discontinuity(&self, offset: IVec2, step: IVec2) -> Option<Discontinuity> {
		Some(discontinuity(self.at(offset)?, self.at(offset + step)?, step, &self.dragged))
	}
}

/// How much the layout jumps between two adjacent drag offsets, measured by how far each node misses either staying or following the step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Discontinuity {
	/// How many nodes neither stay nor follow the step.
	pub nodes: u32,
	/// How many grid cells those nodes miss by, summed.
	pub cells: u32,
	/// The most grid cells any one node misses by.
	pub largest: u32,
}

impl Discontinuity {
	pub fn is_jump(&self) -> bool {
		self.nodes > 0
	}
}

/// How much the layout changes between two drag offsets `step` apart beyond staying or following, leaving out the dragged nodes, which follow every step.
pub fn discontinuity(before: &[IVec2], after: &[IVec2], step: IVec2, dragged: &[u32]) -> Discontinuity {
	misses(before, after, step, dragged).fold(Discontinuity::default(), |total, (_, miss)| Discontinuity {
		nodes: total.nodes + 1,
		cells: total.cells + miss,
		largest: total.largest.max(miss),
	})
}

/// Each node apart from the dragged ones that neither stays nor follows `step` between `before` and `after`, with how many grid cells it misses by.
pub fn misses<'a>(before: &'a [IVec2], after: &'a [IVec2], step: IVec2, dragged: &'a [u32]) -> impl Iterator<Item = (u32, u32)> + 'a {
	before.iter().zip(after).enumerate().filter_map(move |(node, (before, after))| {
		let node = node as u32;
		let moved = *after - *before;
		let miss = moved.abs().element_sum().min((moved - step).abs().element_sum()) as u32;
		(miss > 0 && !dragged.contains(&node)).then_some((node, miss))
	})
}

/// The elongations clearing the overlaps between node boxes while moving the nodes the fewest grid cells in total from where they are stored.
#[derive(Clone, Debug)]
pub struct Optimum {
	pub elongations: Vec<IVec2>,
	/// How many grid cells the nodes are drawn away from where they are stored, summed.
	pub displacement: u32,
	/// How many arrangements of elongations the search laid out to find it.
	pub searched: usize,
}

/// Searches for the elongations that clear every overlap between node boxes the fewest grid cells of displacement allow, or `None` past `search_limit` arrangements.
///
/// Each arrangement takes the first overlap it has and branches on every connection separating the two nodes, drawn longer down or
/// left just enough to clear them. Elongations only add up, so the first arrangement without an overlap is the optimum. Like fitting,
/// it never elongates a chain link, a waypoint, or a connection carrying or touching a held node. An overlap no such connection can clear is left.
pub fn optimal_elongations(graph: &Graph, search_limit: usize) -> Option<Optimum> {
	let stored = layout_as_stored(graph);
	let elongatable = elongatable_connections(graph, &stored);
	let displacement = |layout: &Layout| -> u32 {
		layout
			.positions
			.iter()
			.zip(&stored.positions)
			.map(|(drawn, stored)| (*drawn - *stored).abs().element_sum() as u32)
			.sum()
	};

	let start = vec![IVec2::ZERO; graph.nodes.len()];
	let mut arrangements = vec![start.clone()];
	let mut seen = HashSet::from([start]);
	let mut queue = BinaryHeap::from([(Reverse(0), Reverse(0))]);

	let mut searched = 0;
	while let Some((Reverse(cost), Reverse(index))) = queue.pop() {
		searched += 1;
		if searched > search_limit {
			return None;
		}

		let elongations = arrangements[index].clone();
		let layout = layout_with_elongations(graph, &elongations);
		let Some((first, second)) = first_clearable_overlap(graph, &layout, &elongatable) else {
			return Some(Optimum {
				elongations,
				displacement: cost,
				searched,
			});
		};

		for (moved, other) in [(first, second), (second, first)] {
			let ([moved_min, moved_max], [other_min, other_max]) = (node_box(graph, &layout, moved), node_box(graph, &layout, other));
			let clearances = [IVec2::new(0, other_max.y - moved_min.y), IVec2::new(other_min.x - moved_max.x, 0)];

			for connection in separating_connections(&layout.owners, moved, other).filter(|connection| elongatable[*connection as usize]) {
				for clearance in clearances {
					let mut next = elongations.clone();
					next[connection as usize] += clearance;
					if seen.insert(next.clone()) {
						let next_cost = displacement(&layout_with_elongations(graph, &next));
						queue.push((Reverse(next_cost), Reverse(arrangements.len())));
						arrangements.push(next);
					}
				}
			}
		}
	}
	None
}

/// Whether the connection to each node's owner may be drawn longer: not for a root, a waypoint, a chain link, or a connection carrying or touching a held node.
fn elongatable_connections(graph: &Graph, layout: &Layout) -> Vec<bool> {
	let mut elongatable: Vec<bool> = (0..graph.nodes.len())
		.map(|node| layout.owners[node].is_some() && !graph.nodes[node].waypoint && layout.chain_layers[node].is_none())
		.collect();

	for held in (0..graph.nodes.len()).filter(|node| graph.nodes[*node].held) {
		elongatable[held] = false;
		let mut carrier = layout.owners[held];
		while let Some(node) = carrier {
			elongatable[node as usize] = false;
			carrier = layout.owners[node as usize];
		}
	}
	for (node, elongatable) in elongatable.iter_mut().enumerate() {
		if layout.owners[node].is_some_and(|owner| graph.nodes[owner as usize].held) {
			*elongatable = false;
		}
	}
	elongatable
}

/// The node and each owner above it up to, but not including, the first that also carries `other`, whose connections each move `node` without `other`.
fn separating_connections(owners: &[Option<u32>], node: u32, other: u32) -> impl Iterator<Item = u32> + '_ {
	let mut carrying_other = HashSet::from([other]);
	let mut carrier = owners[other as usize];
	while let Some(owner) = carrier {
		carrying_other.insert(owner);
		carrier = owners[owner as usize];
	}

	std::iter::successors(Some(node), |node| owners[*node as usize]).take_while(move |node| !carrying_other.contains(node))
}

/// The cells a node takes up, with the empty row a layer keeps below it.
fn node_box(graph: &Graph, layout: &Layout, node: u32) -> [IVec2; 2] {
	let [min, max] = layout.bounds[node as usize];
	let bottom_gap = if graph.nodes[node as usize].is_layer { LAYER_BOTTOM_GAP } else { 0 };
	[min, max + IVec2::new(0, bottom_gap)]
}

/// The first two nodes whose boxes overlap and that a connection allowed to be drawn longer can separate.
fn first_clearable_overlap(graph: &Graph, layout: &Layout, elongatable: &[bool]) -> Option<(u32, u32)> {
	let nodes: Vec<u32> = (0..graph.nodes.len() as u32).filter(|node| !graph.nodes[*node as usize].waypoint).collect();
	let boxes: Vec<[IVec2; 2]> = nodes.iter().map(|node| node_box(graph, layout, *node)).collect();

	for (i, first) in nodes.iter().enumerate() {
		for (j, second) in nodes.iter().enumerate().skip(i + 1) {
			let ([first_min, first_max], [second_min, second_max]) = (boxes[i], boxes[j]);
			let overlapping = first_min.cmplt(second_max).all() && second_min.cmplt(first_max).all();
			let clearable = || {
				separating_connections(&layout.owners, *first, *second)
					.chain(separating_connections(&layout.owners, *second, *first))
					.any(|connection| elongatable[connection as usize])
			};
			if overlapping && clearable() {
				return Some((*first, *second));
			}
		}
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{LAYER_HEIGHT, Node, Placement, Wire, layout};

	fn layer(placement: Placement) -> Node {
		Node {
			placement,
			is_layer: true,
			rows: 1,
			width: 5,
			inputs: 2,
			outputs: 1,
			chainable: true,
			waypoint: false,
			held: false,
			name_width: 60.,
			locked: false,
		}
	}

	/// A layer with another stacked closed up under it.
	fn stack() -> Graph {
		Graph {
			nodes: vec![layer(Placement::Absolute(IVec2::new(30, 10))), layer(Placement::Stack(0))],
			wires: vec![Wire {
				source: 1,
				output: 0,
				target: 0,
				input: 0,
			}],
		}
	}

	#[test]
	fn a_field_covers_every_offset_from_its_min_to_its_max() {
		let field = DragField::new(&stack(), vec![1], IVec2::new(-2, -2), IVec2::new(2, 2), layout);

		assert_eq!(field.size, IVec2::new(5, 5));
		assert!(field.at(IVec2::new(2, -2)).is_some());
		assert!(field.at(IVec2::new(3, 0)).is_none());
	}

	#[test]
	fn the_layer_above_is_displaced_once_the_dragged_layer_pushes_it() {
		let field = DragField::new(&stack(), vec![1], IVec2::new(0, -2), IVec2::new(0, 2), layout);

		assert_eq!(field.displacement(IVec2::new(0, 2), IVec2::ZERO), Some(0));
		assert_eq!(field.displacement(IVec2::new(0, -2), IVec2::ZERO), Some(2));
		assert_eq!(field.at(IVec2::ZERO).map(|positions| positions[1].y - positions[0].y), Some(LAYER_HEIGHT + 1));
	}

	#[test]
	fn staying_or_following_the_step_is_no_jump_but_any_other_move_is() {
		let before = [IVec2::new(0, 0), IVec2::new(10, 0), IVec2::new(20, 0), IVec2::new(30, 0)];
		let after = [IVec2::new(0, 0), IVec2::new(10, -1), IVec2::new(20, -4), IVec2::new(31, 0)];

		let jump = discontinuity(&before, &after, IVec2::NEG_Y, &[1]);

		assert_eq!(jump, Discontinuity { nodes: 2, cells: 4, largest: 3 });
	}

	#[test]
	fn the_optimum_clears_an_overlap_by_moving_the_fewest_cells() {
		let node = |placement| Node { is_layer: false, ..layer(placement) };
		let beside = Placement::Relative(IVec2::new(-7, 0));
		let graph = Graph {
			nodes: vec![layer(Placement::Absolute(IVec2::new(30, 10))), node(beside), node(beside)],
			wires: vec![
				Wire {
					source: 1,
					output: 0,
					target: 0,
					input: 1,
				},
				Wire {
					source: 2,
					output: 0,
					target: 0,
					input: 0,
				},
			],
		};

		let Some(optimum) = optimal_elongations(&graph, 1_000) else {
			panic!("the overlap can be cleared")
		};

		// Each node is two rows tall and five columns wide, so moving one down beats moving one left
		assert_eq!(optimum.displacement, 2);
		assert_eq!(optimum.elongations.iter().filter(|elongation| **elongation == IVec2::new(0, 2)).count(), 1);
	}
}
