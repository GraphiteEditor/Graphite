//! Collapse all nodes with all edges < distance

use glam::DVec2;
use petgraph::prelude::UnGraphMap;
use rustc_hash::FxHashSet;

use crate::vector::{PointId, VectorData, VectorDataIndex};

impl VectorData {
	/// Collapse all nodes with all edges < distance
	pub(crate) fn merge_by_distance(&mut self, distance: f64) {
		// treat self as an undirected graph with point = node, and segment = edge
		let indices = VectorDataIndex::build_from(self);

		// Graph that will contain only short edges. References data graph
		let mut short_edges = UnGraphMap::new();
		for seg_id in self.segment_ids().iter().copied() {
			let length = indices.segment_chord_length(seg_id);
			if length < distance {
				let [start, end] = indices.segment_ends(seg_id);
				let start = indices.point_graph.node_weight(start).unwrap().id;
				let end = indices.point_graph.node_weight(end).unwrap().id;

				short_edges.add_node(start);
				short_edges.add_node(end);
				short_edges.add_edge(start, end, seg_id);
			}
		}

		// Now group connected segments - all will be collapsed to a single point.
		// Note: there are a few algorithms for this - perhaps test empirically to find fastest
		let collapse: Vec<FxHashSet<PointId>> = petgraph::algo::tarjan_scc(&short_edges).into_iter().map(|connected| connected.into_iter().collect()).collect();
		let average_position = collapse
			.iter()
			.map(|collapse_set| {
				let sum: DVec2 = collapse_set.iter().map(|&id| indices.point_position(id, self)).sum();
				sum / collapse_set.len() as f64
			})
			.collect::<Vec<_>>();

		// we collect all points up and delete them at the end, so that our indices aren't invalidated
		let mut points_to_delete = FxHashSet::default();
		let mut segments_to_delete = FxHashSet::default();
		for (mut collapse_set, average_pos) in collapse.into_iter().zip(average_position.into_iter()) {
			// remove any segments where both endpoints are in the collapse set
			segments_to_delete.extend(self.segment_domain.iter().filter_map(|(id, start_offset, end_offset, _)| {
				let start = self.point_domain.ids()[start_offset];
				let end = self.point_domain.ids()[end_offset];
				if collapse_set.contains(&start) && collapse_set.contains(&end) { Some(id) } else { None }
			}));

			// Delete all points but the first (arbitrary). Set that point's position to the
			// average of the points, update segments to use replace all points with collapsed
			// point.

			// Unwrap: set created from connected algo will not be empty
			let first_id = collapse_set.iter().copied().next().unwrap();
			// `first_id` the point we will collapse to.
			collapse_set.remove(&first_id);
			let first_offset = indices.point_to_offset[&first_id];

			// look for segments with ends in collapse_set and replace them with the point we are collapsing to
			for (_, start_offset, end_offset, handles) in self.segment_domain.iter_mut() {
				let start_id = self.point_domain.ids()[*start_offset];
				let end_id = self.point_domain.ids()[*end_offset];

				// moved points (only need to update Bezier handles)
				if start_id == first_id {
					let point_position = self.point_domain.position[*start_offset];
					handles.move_start(average_pos - point_position);
				}
				if end_id == first_id {
					let point_position = self.point_domain.position[*end_offset];
					handles.move_end(average_pos - point_position);
				}

				// removed points
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
			// This must come after iterating segments, so segments involving the point at
			// `first_offset` have their handles updated correctly.
			self.point_domain.position[first_offset] = average_pos;

			points_to_delete.extend(collapse_set)
		}
		// For now, remove any faces whose start or end segments are removed.
		// TODO: In future, adjust faces and only delete if all (or all but 1 segments) are removed.
		self.region_domain
			.retain_with_region(|_, segment_range| segments_to_delete.contains(segment_range.start()) || segments_to_delete.contains(segment_range.end()));
		self.segment_domain.retain(|id| !segments_to_delete.contains(id), usize::MAX);
		self.point_domain.retain(&mut self.segment_domain, |id| !points_to_delete.contains(id));
	}
}
