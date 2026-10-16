use crate::vector::flatten::WorkingVector;
use crate::vector::path::Path;
use crate::vector::{PointId, SegmentId, Vector};
use core_types::list::List;
use glam::{DAffine2, DVec2};
use petgraph::graph::{EdgeIndex, NodeIndex, UnGraph};
use petgraph::prelude::UnGraphMap;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

pub trait MergeByDistanceExt {
	/// Collapse all points with edges shorter than the specified distance
	fn merge_by_distance_topological(&mut self, paths: &mut List<Path>, distance: f64);
	fn merge_by_distance_spatial(&mut self, paths: &mut List<Path>, transform: DAffine2, distance: f64);
}

impl MergeByDistanceExt for Vector {
	fn merge_by_distance_topological(&mut self, paths: &mut List<Path>, distance: f64) {
		let mut working = WorkingVector::flatten(self, paths);

		// Treat the working form as an undirected graph
		let indices = VectorIndex::build_from(&working);

		// TODO: We lose information on the winding order by using an undirected graph. Switch to a directed graph and fix the algorithm to handle that.
		// Graph containing only short edges, referencing the data graph
		let mut short_edges = UnGraphMap::new();

		for segment_id in working.segment_ids.iter().copied() {
			let length = indices.segment_chord_length(segment_id);
			if length < distance {
				let [start, end] = indices.segment_ends(segment_id);
				let start = indices.point_graph.node_weight(start).unwrap().id;
				let end = indices.point_graph.node_weight(end).unwrap().id;

				short_edges.add_node(start);
				short_edges.add_node(end);
				short_edges.add_edge(start, end, segment_id);
			}
		}

		// Group connected segments to collapse them into a single point
		// TODO: there are a few possible algorithms for this - perhaps test empirically to find fastest
		let collapse: Vec<FxHashSet<PointId>> = petgraph::algo::tarjan_scc(&short_edges).into_iter().map(|connected| connected.into_iter().collect()).collect();
		let average_position = collapse
			.iter()
			.map(|collapse_set| {
				let sum: DVec2 = collapse_set.iter().map(|&id| indices.point_position(id, &working)).sum();
				sum / collapse_set.len() as f64
			})
			.collect::<Vec<_>>();

		// Collect points and segments to delete at the end to avoid invalidating indices
		let mut points_to_delete = FxHashSet::default();
		let mut segments_to_delete = FxHashSet::default();
		for (mut collapse_set, average_pos) in collapse.into_iter().zip(average_position) {
			// Remove any segments where both endpoints are in the collapse set
			for segment_index in 0..working.segment_ids.len() {
				let start = working.point_ids[working.segment_start[segment_index]];
				let end = working.point_ids[working.segment_end[segment_index]];
				if collapse_set.contains(&start) && collapse_set.contains(&end) {
					segments_to_delete.insert(working.segment_ids[segment_index]);
				}
			}

			// Delete all points but the first, set its position to the average, and update segments
			let first_id = collapse_set.iter().copied().next().unwrap();
			collapse_set.remove(&first_id);
			let first_offset = indices.point_to_offset[&first_id];

			// Look for segments with endpoints in `collapse_set` and replace them with the point we are collapsing to
			for segment_index in 0..working.segment_ids.len() {
				let start_id = working.point_ids[working.segment_start[segment_index]];
				let end_id = working.point_ids[working.segment_end[segment_index]];

				// Update Bezier handles for moved points
				if start_id == first_id {
					let point_position = working.point_positions[working.segment_start[segment_index]];
					working.segment_handles[segment_index].move_start(average_pos - point_position);
				}
				if end_id == first_id {
					let point_position = working.point_positions[working.segment_end[segment_index]];
					working.segment_handles[segment_index].move_end(average_pos - point_position);
				}

				// Replace removed points with the collapsed point
				if collapse_set.contains(&start_id) {
					let point_position = working.point_positions[working.segment_start[segment_index]];
					working.segment_start[segment_index] = first_offset;
					working.segment_handles[segment_index].move_start(average_pos - point_position);
				}
				if collapse_set.contains(&end_id) {
					let point_position = working.point_positions[working.segment_end[segment_index]];
					working.segment_end[segment_index] = first_offset;
					working.segment_handles[segment_index].move_end(average_pos - point_position);
				}
			}

			// Update the position of the collapsed point
			working.point_positions[first_offset] = average_pos;

			points_to_delete.extend(collapse_set)
		}

		// Drop the collapsed segments, then the merged-away points, remapping segment endpoints
		let keep_segment: Vec<bool> = working.segment_ids.iter().map(|id| !segments_to_delete.contains(id)).collect();
		retain_by(&mut working.segment_ids, &keep_segment);
		retain_by(&mut working.segment_start, &keep_segment);
		retain_by(&mut working.segment_end, &keep_segment);
		retain_by(&mut working.segment_handles, &keep_segment);

		let mut point_map = Vec::with_capacity(working.point_ids.len());
		let mut new_index = 0;
		let keep_point: Vec<bool> = working
			.point_ids
			.iter()
			.map(|id| {
				let keep = !points_to_delete.contains(id);
				point_map.push(if keep { new_index } else { usize::MAX });
				new_index += usize::from(keep);
				keep
			})
			.collect();
		retain_by(&mut working.point_ids, &keep_point);
		retain_by(&mut working.point_positions, &keep_point);
		for index in working.segment_start.iter_mut().chain(working.segment_end.iter_mut()) {
			*index = point_map.get(*index).copied().unwrap_or(usize::MAX);
		}

		working.reweave(self, paths);
	}

	fn merge_by_distance_spatial(&mut self, paths: &mut List<Path>, transform: DAffine2, distance: f64) {
		let working = WorkingVector::flatten(self, paths);
		let point_count = working.point_positions.len();

		// Find min x and y for grid cell normalization
		let mut min_x = f64::MAX;
		let mut min_y = f64::MAX;

		// Calculate mins without collecting all positions
		for &pos in &working.point_positions {
			let transformed_pos = transform.transform_point2(pos);
			min_x = min_x.min(transformed_pos.x);
			min_y = min_y.min(transformed_pos.y);
		}

		// Create a spatial grid with cell size of 'distance'
		use std::collections::HashMap;
		let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

		// Add points to grid cells without collecting all positions first
		for i in 0..point_count {
			let pos = transform.transform_point2(working.point_positions[i]);
			let grid_x = ((pos.x - min_x) / distance).floor() as i32;
			let grid_y = ((pos.y - min_y) / distance).floor() as i32;

			grid.entry((grid_x, grid_y)).or_default().push(i);
		}

		// Create point index mapping for merged points
		let mut point_index_map = vec![None; point_count];
		let mut merged_positions = Vec::new();
		let mut merged_indices = Vec::new();

		// Process each point
		for i in 0..point_count {
			// Skip points that have already been processed
			if point_index_map[i].is_some() {
				continue;
			}

			let pos_i = transform.transform_point2(working.point_positions[i]);
			let grid_x = ((pos_i.x - min_x) / distance).floor() as i32;
			let grid_y = ((pos_i.y - min_y) / distance).floor() as i32;

			let mut group = vec![i];

			// Check only neighboring cells (3x3 grid around current cell)
			for dx in -1..=1 {
				for dy in -1..=1 {
					let neighbor_cell = (grid_x + dx, grid_y + dy);

					if let Some(indices) = grid.get(&neighbor_cell) {
						for &j in indices {
							if j > i && point_index_map[j].is_none() {
								let pos_j = transform.transform_point2(working.point_positions[j]);
								if pos_i.distance(pos_j) <= distance {
									group.push(j);
								}
							}
						}
					}
				}
			}

			// Create merged point - calculate positions as needed
			let merged_position = group
				.iter()
				.map(|&idx| transform.transform_point2(working.point_positions[idx]))
				.fold(DVec2::ZERO, |sum, pos| sum + pos)
				/ group.len() as f64;

			let merged_position = transform.inverse().transform_point2(merged_position);
			let merged_index = merged_positions.len();

			merged_positions.push(merged_position);
			merged_indices.push(working.point_ids[group[0]]);

			// Update mapping for all points in the group
			for &idx in &group {
				point_index_map[idx] = Some(merged_index);
			}
		}

		// Build the merged working form, skipping segments with endpoints that collapsed together
		let mut merged = WorkingVector {
			point_ids: merged_indices,
			point_positions: merged_positions,
			colinear: working.colinear,
			..Default::default()
		};

		for segment_index in 0..working.segment_ids.len() {
			let new_start = point_index_map[working.segment_start[segment_index]].unwrap();
			let new_end = point_index_map[working.segment_end[segment_index]].unwrap();

			if new_start != new_end {
				merged.segment_ids.push(working.segment_ids[segment_index]);
				merged.segment_start.push(new_start);
				merged.segment_end.push(new_end);
				merged.segment_handles.push(working.segment_handles[segment_index]);
			}
		}

		merged.reweave(self, paths);
	}
}

/// Keeps only the elements flagged true, in order.
fn retain_by<T>(values: &mut Vec<T>, keep: &[bool]) {
	let mut flags = keep.iter();
	values.retain(|_| *flags.next().unwrap_or(&false));
}

/// All the fixed fields of a point from the working form.
pub(crate) struct Point {
	pub id: PointId,
	pub position: DVec2,
}

/// Useful indexes to speed up various operations on the flattened working form.
///
/// Important: It is the user's responsibility to ensure the indexes remain valid after mutations to the data.
pub(crate) struct VectorIndex {
	/// Points and segments form a graph. Store it here in a form amenable to graph algorithms.
	///
	/// Currently, segment data is not stored as it is not used, but it could easily be added.
	pub(crate) point_graph: UnGraph<Point, ()>,
	pub(crate) segment_to_edge: FxHashMap<SegmentId, EdgeIndex>,
	/// Get the offset from the point ID.
	pub(crate) point_to_offset: FxHashMap<PointId, usize>,
	// TODO: faces
}

impl VectorIndex {
	/// Construct a [`VectorIndex`] by building indexes from the given working form. Takes `O(n)` time.
	fn build_from(working: &WorkingVector) -> Self {
		let point_to_offset = working.point_ids.iter().copied().enumerate().map(|(a, b)| (b, a)).collect::<FxHashMap<_, _>>();

		let mut point_to_node = FxHashMap::default();
		let mut segment_to_edge = FxHashMap::default();

		let mut graph = UnGraph::new_undirected();

		for (&point_id, &position) in working.point_ids.iter().zip(&working.point_positions) {
			let idx = graph.add_node(Point { id: point_id, position });
			point_to_node.insert(point_id, idx);
		}

		for (segment_index, &segment_id) in working.segment_ids.iter().enumerate() {
			let start_id = working.point_ids[working.segment_start[segment_index]];
			let end_id = working.point_ids[working.segment_end[segment_index]];
			let edge = graph.add_edge(point_to_node[&start_id], point_to_node[&end_id], ());

			segment_to_edge.insert(segment_id, edge);
		}

		Self {
			point_graph: graph,
			segment_to_edge,
			point_to_offset,
		}
	}

	/// Fetch the length of given segment's chord. Takes `O(1)` time.
	///
	/// # Panics
	///
	/// Will panic if no segment with the given ID is found.
	fn segment_chord_length(&self, id: SegmentId) -> f64 {
		let edge_idx = self.segment_to_edge[&id];
		let (start, end) = self.point_graph.edge_endpoints(edge_idx).unwrap();
		let start_position = self.point_graph.node_weight(start).unwrap().position;
		let end_position = self.point_graph.node_weight(end).unwrap().position;
		(start_position - end_position).length()
	}

	/// Get the ends of a segment. Takes `O(1)` time.
	///
	/// The IDs will be ordered [smallest, largest] so they can be used to find other segments with the same endpoints, regardless of direction.
	///
	/// # Panics
	///
	/// This function will panic if the ID is not present.
	fn segment_ends(&self, id: SegmentId) -> [NodeIndex; 2] {
		let (start, end) = self.point_graph.edge_endpoints(self.segment_to_edge[&id]).unwrap();
		if start < end { [start, end] } else { [end, start] }
	}

	/// Get the physical location of a point. Takes `O(1)` time.
	///
	/// # Panics
	///
	/// Will panic if `id` isn't in the data.
	fn point_position(&self, id: PointId, working: &WorkingVector) -> DVec2 {
		let offset = self.point_to_offset[&id];
		working.point_positions[offset]
	}
}
