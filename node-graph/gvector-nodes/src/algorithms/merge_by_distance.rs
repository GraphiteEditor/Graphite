use glam::{DAffine2, DVec2};
use graphene_vector::{PointDomain, PointId, SegmentDomain, VectorData, VectorDataIndex};
use petgraph::prelude::UnGraphMap;
use rustc_hash::FxHashSet;

pub trait MergeByDistanceExt {
	/// Collapse all points with edges shorter than the specified distance
	fn merge_by_distance_topological(&mut self, distance: f64);
	fn merge_by_distance_spatial(&mut self, transform: DAffine2, distance: f64);
}

impl MergeByDistanceExt for VectorData {
	fn merge_by_distance_topological(&mut self, distance: f64) {
		// Treat self as an undirected graph
		let indices = VectorDataIndex::build_from(self);

		// TODO: We lose information on the winding order by using an undirected graph. Switch to a directed graph and fix the algorithm to handle that.
		// Graph containing only short edges, referencing the data graph
		let mut short_edges = UnGraphMap::new();

		for segment_id in self.segment_ids().iter().copied() {
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
				let sum: DVec2 = collapse_set.iter().map(|&id| indices.point_position(id, self)).sum();
				sum / collapse_set.len() as f64
			})
			.collect::<Vec<_>>();

		// Collect points and segments to delete at the end to avoid invalidating indices
		let mut points_to_delete = FxHashSet::default();
		let mut segments_to_delete = FxHashSet::default();
		for (mut collapse_set, average_pos) in collapse.into_iter().zip(average_position.into_iter()) {
			// Remove any segments where both endpoints are in the collapse set
			segments_to_delete.extend(self.segment_domain.iter().filter_map(|(id, start_offset, end_offset, _)| {
				let start = self.point_domain.ids()[start_offset];
				let end = self.point_domain.ids()[end_offset];
				if collapse_set.contains(&start) && collapse_set.contains(&end) { Some(id) } else { None }
			}));

			// Delete all points but the first, set its position to the average, and update segments
			let first_id = collapse_set.iter().copied().next().unwrap();
			collapse_set.remove(&first_id);
			let first_offset = indices.point_to_offset[&first_id];

			// Look for segments with endpoints in `collapse_set` and replace them with the point we are collapsing to
			for (_, start_offset, end_offset, handles) in self.segment_domain.iter_mut() {
				let start_id = self.point_domain.ids()[*start_offset];
				let end_id = self.point_domain.ids()[*end_offset];

				// Update Bezier handles for moved points
				if start_id == first_id {
					let point_position = self.point_domain.position[*start_offset];
					handles.move_start(average_pos - point_position);
				}
				if end_id == first_id {
					let point_position = self.point_domain.position[*end_offset];
					handles.move_end(average_pos - point_position);
				}

				// Replace removed points with the collapsed point
				if collapse_set.contains(&start_id) {
					let point_position = self.point_domain.position[*start_offset];
					*start_offset = first_offset;
					handles.move_start(average_pos - point_position);
				}
				if collapse_set.contains(&end_id) {
					let point_position = self.point_domain.position[*end_offset];
					*end_offset = first_offset;
					handles.move_end(average_pos - point_position);
				}
			}

			// Update the position of the collapsed point
			self.point_domain.position[first_offset] = average_pos;

			points_to_delete.extend(collapse_set)
		}

		// Remove faces whose start or end segments are removed
		// TODO: Adjust faces and only delete if all (or all but one) segments are removed
		self.region_domain
			.retain_with_region(|_, segment_range| segments_to_delete.contains(segment_range.start()) || segments_to_delete.contains(segment_range.end()));
		self.segment_domain.retain(|id| !segments_to_delete.contains(id), usize::MAX);
		self.point_domain.retain(&mut self.segment_domain, |id| !points_to_delete.contains(id));
	}

	fn merge_by_distance_spatial(&mut self, transform: DAffine2, distance: f64) {
		let point_count = self.point_domain.positions().len();

		// Find min x and y for grid cell normalization
		let mut min_x = f64::MAX;
		let mut min_y = f64::MAX;

		// Calculate mins without collecting all positions
		for &pos in self.point_domain.positions() {
			let transformed_pos = transform.transform_point2(pos);
			min_x = min_x.min(transformed_pos.x);
			min_y = min_y.min(transformed_pos.y);
		}

		// Create a spatial grid with cell size of 'distance'
		use std::collections::HashMap;
		let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

		// Add points to grid cells without collecting all positions first
		for i in 0..point_count {
			let pos = transform.transform_point2(self.point_domain.positions()[i]);
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

			let pos_i = transform.transform_point2(self.point_domain.positions()[i]);
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
								let pos_j = transform.transform_point2(self.point_domain.positions()[j]);
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
				.map(|&idx| transform.transform_point2(self.point_domain.positions()[idx]))
				.fold(DVec2::ZERO, |sum, pos| sum + pos)
				/ group.len() as f64;

			let merged_position = transform.inverse().transform_point2(merged_position);
			let merged_index = merged_positions.len();

			merged_positions.push(merged_position);
			merged_indices.push(self.point_domain.ids()[group[0]]);

			// Update mapping for all points in the group
			for &idx in &group {
				point_index_map[idx] = Some(merged_index);
			}
		}

		// Create new point domain with merged points
		let mut new_point_domain = PointDomain::new();
		for (idx, pos) in merged_indices.into_iter().zip(merged_positions) {
			new_point_domain.push(idx, pos);
		}

		// Update segment domain
		let mut new_segment_domain = SegmentDomain::new();
		for segment_idx in 0..self.segment_domain.ids().len() {
			let id = self.segment_domain.ids()[segment_idx];
			let start = self.segment_domain.start_point()[segment_idx];
			let end = self.segment_domain.end_point()[segment_idx];
			let handles = self.segment_domain.handles()[segment_idx];
			let stroke = self.segment_domain.stroke()[segment_idx];

			// Get new indices for start and end points
			let new_start = point_index_map[start].unwrap();
			let new_end = point_index_map[end].unwrap();

			// Skip segments where start and end points were merged
			if new_start != new_end {
				new_segment_domain.push(id, new_start, new_end, handles, stroke);
			}
		}

		// Create new vector data
		self.point_domain = new_point_domain;
		self.segment_domain = new_segment_domain;
	}
}
