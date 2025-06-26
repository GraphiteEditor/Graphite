use super::{PointId, SegmentId, VectorData};
use glam::DVec2;
use petgraph::graph::{EdgeIndex, NodeIndex, UnGraph};
use rustc_hash::FxHashMap;

/// All the fixed fields of a point from the point domain.
pub struct Point {
	pub id: PointId,
	pub position: DVec2,
}

/// Useful indexes to speed up various operations on `VectorData`.
///
/// Important: It is the user's responsibility to ensure the indexes remain valid after mutations to the data.
pub struct VectorDataIndex {
	/// Points and segments form a graph. Store it here in a form amenable to graph algorithms.
	///
	/// Currently, segment data is not stored as it is not used, but it could easily be added.
	pub point_graph: UnGraph<Point, ()>,
	pub segment_to_edge: FxHashMap<SegmentId, EdgeIndex>,
	/// Get the offset from the point ID.
	pub point_to_offset: FxHashMap<PointId, usize>,
	// TODO: faces
}

impl VectorDataIndex {
	/// Construct a [`VectorDataIndex`] by building indexes from the given [`VectorData`]. Takes `O(n)` time.
	pub fn build_from(data: &VectorData) -> Self {
		let point_to_offset = data.point_domain.ids().iter().copied().enumerate().map(|(a, b)| (b, a)).collect::<FxHashMap<_, _>>();

		let mut point_to_node = FxHashMap::default();
		let mut segment_to_edge = FxHashMap::default();

		let mut graph = UnGraph::new_undirected();

		for (point_id, position) in data.point_domain.iter() {
			let idx = graph.add_node(Point { id: point_id, position });
			point_to_node.insert(point_id, idx);
		}

		for (segment_id, start_offset, end_offset, ..) in data.segment_domain.iter() {
			let start_id = data.point_domain.ids()[start_offset];
			let end_id = data.point_domain.ids()[end_offset];
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
	pub fn segment_chord_length(&self, id: SegmentId) -> f64 {
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
	pub fn segment_ends(&self, id: SegmentId) -> [NodeIndex; 2] {
		let (start, end) = self.point_graph.edge_endpoints(self.segment_to_edge[&id]).unwrap();
		if start < end { [start, end] } else { [end, start] }
	}

	/// Get the physical location of a point. Takes `O(1)` time.
	///
	/// # Panics
	///
	/// Will panic if `id` isn't in the data.
	pub fn point_position(&self, id: PointId, data: &VectorData) -> DVec2 {
		let offset = self.point_to_offset[&id];
		data.point_domain.positions()[offset]
	}
}
