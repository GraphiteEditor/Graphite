/// Which kinds of snap are offered. Turning one off removes it as a target without changing what the moving thing can snap with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
	pub anchors: bool,
	pub midpoints: bool,
	pub curves: bool,
	pub box_corners: bool,
	pub box_edge_midpoints: bool,
	pub box_centers: bool,
	pub box_edges: bool,
	pub artboards: bool,
	pub tangents: bool,
	pub perpendiculars: bool,
	/// Lines along each axis through awake points, and through the point a segment is drawn from.
	pub point_alignment: bool,
	/// Lines along each axis through the corners and centers of awake boxes.
	pub box_alignment: bool,
	pub extensions: bool,
	/// Parallels, right angles, lines through two points, and the points between them.
	pub constructions: bool,
	pub equal_lengths: bool,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			anchors: true,
			midpoints: true,
			curves: true,
			box_corners: true,
			box_edge_midpoints: true,
			box_centers: true,
			box_edges: true,
			artboards: true,
			tangents: true,
			perpendiculars: true,
			point_alignment: true,
			box_alignment: true,
			extensions: true,
			constructions: true,
			equal_lengths: true,
		}
	}
}
