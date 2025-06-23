use crate::vector::{PointId, VectorData, VectorDataIndex};
use glam::DVec2;
use petgraph::prelude::UnGraphMap;
use rustc_hash::FxHashSet;

impl VectorData {
	/// Collapse all points with edges shorter than the specified distance
	pub fn merge_by_distance(&mut self, distance: f64) {
		// Treat self as an undirected graph
		let indices = VectorDataIndex::build_from(self);

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
}
