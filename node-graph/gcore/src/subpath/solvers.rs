use crate::subpath::{Identifier, Subpath};
use crate::vector::algorithms::bezpath_algorithms::bezpath_is_inside_bezpath;
use crate::vector::misc::dvec2_to_point;
use glam::DVec2;
use kurbo::{Affine, BezPath, Shape};

impl<PointId: Identifier> Subpath<PointId> {
	pub fn contains_point(&self, point: DVec2) -> bool {
		self.to_bezpath().contains(dvec2_to_point(point))
	}

	pub fn to_bezpath(&self) -> BezPath {
		let mut bezpath = kurbo::BezPath::new();
		let mut out_handle;

		let Some(first) = self.manipulator_groups.first() else { return bezpath };
		bezpath.move_to(dvec2_to_point(first.anchor));
		out_handle = first.out_handle;

		for manipulator in self.manipulator_groups.iter().skip(1) {
			match (out_handle, manipulator.in_handle) {
				(Some(handle_start), Some(handle_end)) => bezpath.curve_to(dvec2_to_point(handle_start), dvec2_to_point(handle_end), dvec2_to_point(manipulator.anchor)),
				(None, None) => bezpath.line_to(dvec2_to_point(manipulator.anchor)),
				(None, Some(handle)) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(manipulator.anchor)),
				(Some(handle), None) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(manipulator.anchor)),
			}
			out_handle = manipulator.out_handle;
		}

		if self.closed {
			match (out_handle, first.in_handle) {
				(Some(handle_start), Some(handle_end)) => bezpath.curve_to(dvec2_to_point(handle_start), dvec2_to_point(handle_end), dvec2_to_point(first.anchor)),
				(None, None) => bezpath.line_to(dvec2_to_point(first.anchor)),
				(None, Some(handle)) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(first.anchor)),
				(Some(handle), None) => bezpath.quad_to(dvec2_to_point(handle), dvec2_to_point(first.anchor)),
			}
			bezpath.close_path();
		}
		bezpath
	}

	/// Returns `true` if this subpath is completely inside the `other` subpath.
	pub fn is_inside_subpath(&self, other: &Subpath<PointId>, accuracy: Option<f64>, minimum_separation: Option<f64>) -> bool {
		bezpath_is_inside_bezpath(&self.to_bezpath(), &other.to_bezpath(), accuracy, minimum_separation)
	}

	/// Return the min and max corners that represent the bounding box of the subpath. Return `None` if the subpath is empty.
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.iter()
			.map(|bezier| bezier.bounding_box())
			.map(|bbox| [DVec2::new(bbox.min_x(), bbox.min_y()), DVec2::new(bbox.max_x(), bbox.max_y())])
			.reduce(|bbox1, bbox2| [bbox1[0].min(bbox2[0]), bbox1[1].max(bbox2[1])])
	}

	/// Return the min and max corners that represent the bounding box of the subpath, after a given affine transform.
	pub fn bounding_box_with_transform(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		self.iter()
			.map(|bezier| (Affine::new(transform.to_cols_array()) * bezier).bounding_box())
			.map(|bbox| [DVec2::new(bbox.min_x(), bbox.min_y()), DVec2::new(bbox.max_x(), bbox.max_y())])
			.reduce(|bbox1, bbox2| [bbox1[0].min(bbox2[0]), bbox1[1].max(bbox2[1])])
	}

	/// Return the min and max corners that represent the loose bounding box of the subpath (bounding box of all handles and anchors).
	pub fn loose_bounding_box(&self) -> Option<[DVec2; 2]> {
		self.manipulator_groups
			.iter()
			.flat_map(|group| [group.in_handle, group.out_handle, Some(group.anchor)])
			.flatten()
			.map(|pos| [pos, pos])
			.reduce(|bbox1, bbox2| [bbox1[0].min(bbox2[0]), bbox1[1].max(bbox2[1])])
	}

	/// Return the min and max corners that represent the loose bounding box of the subpath, after a given affine transform.
	pub fn loose_bounding_box_with_transform(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		self.manipulator_groups
			.iter()
			.flat_map(|group| [group.in_handle, group.out_handle, Some(group.anchor)])
			.flatten()
			.map(|pos| transform.transform_point2(pos))
			.map(|pos| [pos, pos])
			.reduce(|bbox1, bbox2| [bbox1[0].min(bbox2[0]), bbox1[1].max(bbox2[1])])
	}
}
