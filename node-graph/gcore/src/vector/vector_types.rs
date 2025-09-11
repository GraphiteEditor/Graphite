use super::misc::dvec2_to_point;
use super::style::{PathStyle, Stroke};
pub use super::vector_attributes::*;
pub use super::vector_modification::*;
use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::math::quad::Quad;
use crate::subpath::{BezierHandles, ManipulatorGroup, Subpath};
use crate::table::{Table, TableRow};
use crate::transform::Transform;
use crate::vector::click_target::{ClickTargetType, FreePoint};
use crate::vector::misc::{HandleId, ManipulatorPointId};
use crate::{AlphaBlending, Color, Graphic};
use core::borrow::Borrow;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, Rect, Shape};
use std::collections::HashMap;

/// Represents vector graphics data, composed of Bézier curves in a path or mesh arrangement.
#[derive(Clone, Debug, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct Vector {
	pub style: PathStyle,

	/// A list of all manipulator groups (referenced in `subpaths`) that have colinear handles (where they're locked at 180° angles from one another).
	/// This gets read in `graph_operation_message_handler.rs` by calling `inputs.as_mut_slice()` (search for the string `"Shape does not have both `subpath` and `colinear_manipulators` inputs"` to find it).
	pub colinear_manipulators: Vec<[HandleId; 2]>,

	pub point_domain: PointDomain,
	pub segment_domain: SegmentDomain,
	pub region_domain: RegionDomain,

	/// Used to store the upstream group/folder of nested layers during destructive Boolean Operations (and other nodes with a similar effect) so that click targets can be preserved for the child layers.
	/// Without this, the tools would be working with a collapsed version of the data which has no reference to the original child layers that were booleaned together, resulting in the inner layers not being editable.
	#[serde(alias = "upstream_group")]
	pub upstream_nested_layers: Option<Table<Graphic>>,
}

impl Default for Vector {
	fn default() -> Self {
		Self {
			style: PathStyle::new(Some(Stroke::new(Some(Color::BLACK), 0.)), super::style::Fill::None),
			colinear_manipulators: Vec::new(),
			point_domain: PointDomain::new(),
			segment_domain: SegmentDomain::new(),
			region_domain: RegionDomain::new(),
			upstream_nested_layers: None,
		}
	}
}

impl std::hash::Hash for Vector {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.point_domain.hash(state);
		self.segment_domain.hash(state);
		self.region_domain.hash(state);
		self.style.hash(state);
		self.colinear_manipulators.hash(state);
	}
}

impl Vector {
	/// Add a subpath to this vector path.
	pub fn append_subpath(&mut self, subpath: impl Borrow<Subpath<PointId>>, preserve_id: bool) {
		let subpath: &Subpath<PointId> = subpath.borrow();
		let stroke_id = StrokeId::ZERO;
		let mut point_id = self.point_domain.next_id();

		let handles = |a: &ManipulatorGroup<_>, b: &ManipulatorGroup<_>| match (a.out_handle, b.in_handle) {
			(None, None) => BezierHandles::Linear,
			(Some(handle), None) | (None, Some(handle)) => BezierHandles::Quadratic { handle },
			(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic { handle_start, handle_end },
		};
		let [mut first_seg, mut last_seg] = [None, None];
		let mut segment_id = self.segment_domain.next_id();
		let mut last_point = None;
		let mut first_point = None;

		// Construct a bezier segment from the two manipulators on the subpath.
		for pair in subpath.manipulator_groups().windows(2) {
			let start = last_point.unwrap_or_else(|| {
				let id = if preserve_id && !self.point_domain.ids().contains(&pair[0].id) {
					pair[0].id
				} else {
					point_id.next_id()
				};
				self.point_domain.push(id, pair[0].anchor);
				self.point_domain.ids().len() - 1
			});
			first_point = Some(first_point.unwrap_or(start));
			let end = if preserve_id && !self.point_domain.ids().contains(&pair[1].id) {
				pair[1].id
			} else {
				point_id.next_id()
			};
			let end_index = self.point_domain.ids().len();
			self.point_domain.push(end, pair[1].anchor);

			let id = segment_id.next_id();
			first_seg = Some(first_seg.unwrap_or(id));
			last_seg = Some(id);
			self.segment_domain.push(id, start, end_index, handles(&pair[0], &pair[1]), stroke_id);

			last_point = Some(end_index);
		}

		let fill_id = FillId::ZERO;

		if subpath.closed() {
			if let (Some(last), Some(first), Some(first_id), Some(last_id)) = (subpath.manipulator_groups().last(), subpath.manipulator_groups().first(), first_point, last_point) {
				let id = segment_id.next_id();
				first_seg = Some(first_seg.unwrap_or(id));
				last_seg = Some(id);
				self.segment_domain.push(id, last_id, first_id, handles(last, first), stroke_id);
			}

			if let [Some(first_seg), Some(last_seg)] = [first_seg, last_seg] {
				self.region_domain.push(self.region_domain.next_id(), first_seg..=last_seg, fill_id);
			}
		}
	}

	pub fn append_free_point(&mut self, point: &FreePoint, preserve_id: bool) {
		let mut point_id = self.point_domain.next_id();

		// Use the current point ID if it's not already in the domain, otherwise generate a new one
		let id = if preserve_id && !self.point_domain.ids().contains(&point.id) {
			point.id
		} else {
			point_id.next_id()
		};
		self.point_domain.push(id, point.position);
	}

	/// Construct some new vector path from a single subpath with an identity transform and black fill.
	pub fn from_subpath(subpath: impl Borrow<Subpath<PointId>>) -> Self {
		Self::from_subpaths([subpath], false)
	}

	/// Construct some new vector path from a single [`BezPath`] with an identity transform and black fill.
	pub fn from_bezpath(bezpath: BezPath) -> Self {
		let mut vector = Self::default();
		vector.append_bezpath(bezpath);
		vector
	}

	/// Construct some new vector path from subpaths with an identity transform and black fill.
	pub fn from_subpaths(subpaths: impl IntoIterator<Item = impl Borrow<Subpath<PointId>>>, preserve_id: bool) -> Self {
		let mut vector = Self::default();

		for subpath in subpaths.into_iter() {
			vector.append_subpath(subpath, preserve_id);
		}

		vector
	}

	pub fn from_target_types(target_types: impl IntoIterator<Item = impl Borrow<ClickTargetType>>, preserve_id: bool) -> Self {
		let mut vector = Self::default();

		for target_type in target_types.into_iter() {
			match target_type.borrow() {
				ClickTargetType::Subpath(subpath) => vector.append_subpath(subpath, preserve_id),
				ClickTargetType::FreePoint(point) => vector.append_free_point(point, preserve_id),
			}
		}

		vector
	}

	/// Compute the bounding boxes of the bezpaths without any transform
	pub fn bounding_box_rect(&self) -> Option<Rect> {
		self.bounding_box_with_transform_rect(DAffine2::IDENTITY)
	}

	pub fn close_subpaths(&mut self) {
		let segments_to_add: Vec<_> = self
			.build_stroke_path_iter()
			.filter(|(_, closed)| !closed)
			.filter_map(|(manipulator_groups, _)| {
				let (first, last) = manipulator_groups.first().zip(manipulator_groups.last())?;
				let (start, end) = self.point_domain.resolve_id(first.id).zip(self.point_domain.resolve_id(last.id))?;
				Some((start, end))
			})
			.collect();

		for (start, end) in segments_to_add {
			let segment_id = self.segment_domain.next_id().next_id();
			self.segment_domain.push(segment_id, start, end, BezierHandles::Linear, StrokeId::ZERO);
		}
	}

	/// Compute the bounding boxes of the subpaths without any transform
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform_rect(DAffine2::IDENTITY)
			.map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
	}

	/// Compute the bounding boxes of the subpaths with the specified transform
	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform_rect(transform)
			.map(|rect| [DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
	}

	/// Compute the bounding boxes of the bezpaths with the specified transform
	pub fn bounding_box_with_transform_rect(&self, transform: DAffine2) -> Option<Rect> {
		let combine = |r1: Rect, r2: Rect| r1.union(r2);
		self.stroke_bezpath_iter()
			.map(|mut bezpath| {
				bezpath.apply_affine(Affine::new(transform.to_cols_array()));
				bezpath.bounding_box()
			})
			.reduce(combine)
	}

	/// Calculate the corners of the bounding box but with a nonzero size.
	///
	/// If the layer bounds are `0` in either axis then they are changed to be `1`.
	pub fn nonzero_bounding_box(&self) -> [DVec2; 2] {
		let [bounds_min, mut bounds_max] = self.bounding_box().unwrap_or_default();

		let bounds_size = bounds_max - bounds_min;
		if bounds_size.x < 1e-10 {
			bounds_max.x = bounds_min.x + 1.;
		}
		if bounds_size.y < 1e-10 {
			bounds_max.y = bounds_min.y + 1.;
		}

		[bounds_min, bounds_max]
	}

	/// Compute the pivot of the layer in layerspace (the coordinates of the subpaths)
	pub fn layerspace_pivot(&self, normalized_pivot: DVec2) -> DVec2 {
		let [bounds_min, bounds_max] = self.nonzero_bounding_box();
		let bounds_size = bounds_max - bounds_min;
		bounds_min + bounds_size * normalized_pivot
	}

	pub fn start_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.segment_domain.start_point().iter().map(|&index| self.point_domain.ids()[index])
	}

	pub fn end_point(&self) -> impl Iterator<Item = PointId> + '_ {
		self.segment_domain.end_point().iter().map(|&index| self.point_domain.ids()[index])
	}

	pub fn push(&mut self, id: SegmentId, start: PointId, end: PointId, handles: (Option<DVec2>, Option<DVec2>), stroke: StrokeId) {
		let [Some(start), Some(end)] = [start, end].map(|id| self.point_domain.resolve_id(id)) else {
			return;
		};
		let handles = match handles {
			(None, None) => BezierHandles::Linear,
			(None, Some(handle)) | (Some(handle), None) => BezierHandles::Quadratic { handle },
			(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic { handle_start, handle_end },
		};
		self.segment_domain.push(id, start, end, handles, stroke)
	}

	pub fn handles_mut(&mut self) -> impl Iterator<Item = (SegmentId, &mut BezierHandles, PointId, PointId)> {
		self.segment_domain
			.handles_mut()
			.map(|(id, handles, start, end)| (id, handles, self.point_domain.ids()[start], self.point_domain.ids()[end]))
	}

	pub fn segment_start_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.segment_domain.segment_start_from_id(segment).map(|index| self.point_domain.ids()[index])
	}

	pub fn segment_end_from_id(&self, segment: SegmentId) -> Option<PointId> {
		self.segment_domain.segment_end_from_id(segment).map(|index| self.point_domain.ids()[index])
	}

	/// Returns an array for the start and end points of a segment.
	pub fn points_from_id(&self, segment: SegmentId) -> Option<[PointId; 2]> {
		self.segment_domain.points_from_id(segment).map(|val| val.map(|index| self.point_domain.ids()[index]))
	}

	/// Attempts to find another point in the segment that is not the one passed in.
	pub fn other_point(&self, segment: SegmentId, current: PointId) -> Option<PointId> {
		let index = self.point_domain.resolve_id(current);
		index.and_then(|index| self.segment_domain.other_point(segment, index)).map(|index| self.point_domain.ids()[index])
	}

	/// Gets all points connected to the current one but not including the current one.
	pub fn connected_points(&self, current: PointId) -> impl Iterator<Item = PointId> + '_ {
		let index = [self.point_domain.resolve_id(current)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.connected_points(index).map(|index| self.point_domain.ids()[index]))
	}

	/// Returns the number of linear segments connected to the given point.
	pub fn connected_linear_segments(&self, point_id: PointId) -> usize {
		self.segment_bezier_iter()
			.filter(|(_, bez, start, end)| (*start == point_id || *end == point_id) && matches!(bez.handles, BezierHandles::Linear))
			.count()
	}

	/// Get an array slice of all segment IDs.
	pub fn segment_ids(&self) -> &[SegmentId] {
		self.segment_domain.ids()
	}

	/// Enumerate all segments that start at the point.
	pub fn start_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.start_connected(index))
	}

	/// Enumerate all segments that end at the point.
	pub fn end_connected(&self, point: PointId) -> impl Iterator<Item = SegmentId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.end_connected(index))
	}

	/// Enumerate all segments that start or end at a point, converting them to [`HandleId`s]. Note that the handles may not exist e.g. for a linear segment.
	pub fn all_connected(&self, point: PointId) -> impl Iterator<Item = HandleId> + '_ {
		let index = [self.point_domain.resolve_id(point)].into_iter().flatten();
		index.flat_map(|index| self.segment_domain.all_connected(index))
	}

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub fn connected_count(&self, point: PointId) -> usize {
		self.point_domain.resolve_id(point).map_or(0, |point| self.segment_domain.connected_count(point))
	}

	/// Enumerate the number of segments connected to a point. If a segment starts and ends at a point then it is counted twice.
	pub fn any_connected(&self, point: PointId) -> bool {
		self.point_domain.resolve_id(point).is_some_and(|point| self.segment_domain.any_connected(point))
	}

	pub fn check_point_inside_shape(&self, transform: DAffine2, point: DVec2) -> bool {
		let number = self
			.stroke_bezpath_iter()
			.map(|mut bezpath| {
				// TODO: apply transform to points instead of modifying the paths
				bezpath.apply_affine(Affine::new(transform.to_cols_array()));
				bezpath.close_path();
				let bbox = bezpath.bounding_box();
				(bezpath, bbox)
			})
			.filter(|(_, bbox)| bbox.contains(dvec2_to_point(point)))
			.map(|(bezpath, _)| bezpath.winding(dvec2_to_point(point)))
			.sum::<i32>();

		// Non-zero fill rule
		number != 0
	}

	/// Points that can be extended from.
	///
	/// This is usually only points with exactly one connection unless vector meshes are enabled.
	pub fn extendable_points(&self, vector_meshes: bool) -> impl Iterator<Item = PointId> + '_ {
		let point_ids = self.point_domain.ids().iter().enumerate();
		point_ids.filter(move |(index, _)| vector_meshes || self.segment_domain.connected_count(*index) == 1).map(|(_, &id)| id)
	}

	/// Computes if all the connected handles are colinear for an anchor, or if that handle is colinear for a handle.
	pub fn colinear(&self, point: ManipulatorPointId) -> bool {
		let has_handle = |target| self.colinear_manipulators.iter().flatten().any(|&handle| handle == target);
		match point {
			ManipulatorPointId::Anchor(id) => {
				self.start_connected(id).all(|segment| has_handle(HandleId::primary(segment))) && self.end_connected(id).all(|segment| has_handle(HandleId::end(segment)))
			}
			ManipulatorPointId::PrimaryHandle(segment) => has_handle(HandleId::primary(segment)),
			ManipulatorPointId::EndHandle(segment) => has_handle(HandleId::end(segment)),
		}
	}

	pub fn other_colinear_handle(&self, handle: HandleId) -> Option<HandleId> {
		let pair = self.colinear_manipulators.iter().find(|pair| pair.contains(&handle))?;
		let other = pair.iter().copied().find(|&val| val != handle)?;
		if handle.to_manipulator_point().get_anchor(self) == other.to_manipulator_point().get_anchor(self) {
			Some(other)
		} else {
			None
		}
	}

	pub fn adjacent_segment(&self, manipulator_id: &ManipulatorPointId) -> Option<(PointId, SegmentId)> {
		match manipulator_id {
			ManipulatorPointId::PrimaryHandle(segment_id) => {
				// For start handle, find segments ending at our start point
				let (start_point_id, _, _) = self.segment_points_from_id(*segment_id)?;
				let start_index = self.point_domain.resolve_id(start_point_id)?;

				self.segment_domain.end_connected(start_index).find(|&id| id != *segment_id).map(|id| (start_point_id, id)).or(self
					.segment_domain
					.start_connected(start_index)
					.find(|&id| id != *segment_id)
					.map(|id| (start_point_id, id)))
			}
			ManipulatorPointId::EndHandle(segment_id) => {
				// For end handle, find segments starting at our end point
				let (_, end_point_id, _) = self.segment_points_from_id(*segment_id)?;
				let end_index = self.point_domain.resolve_id(end_point_id)?;

				self.segment_domain.start_connected(end_index).find(|&id| id != *segment_id).map(|id| (end_point_id, id)).or(self
					.segment_domain
					.end_connected(end_index)
					.find(|&id| id != *segment_id)
					.map(|id| (end_point_id, id)))
			}
			ManipulatorPointId::Anchor(_) => None,
		}
	}

	pub fn concat(&mut self, additional: &Self, transform_of_additional: DAffine2, collision_hash_seed: u64) {
		let point_map = additional
			.point_domain
			.ids()
			.iter()
			.filter(|id| self.point_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let segment_map = additional
			.segment_domain
			.ids()
			.iter()
			.filter(|id| self.segment_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let region_map = additional
			.region_domain
			.ids()
			.iter()
			.filter(|id| self.region_domain.ids().contains(id))
			.map(|&old| (old, old.generate_from_hash(collision_hash_seed)))
			.collect::<HashMap<_, _>>();

		let id_map = IdMap {
			point_offset: self.point_domain.ids().len(),
			point_map,
			segment_map,
			region_map,
		};

		self.point_domain.concat(&additional.point_domain, transform_of_additional, &id_map);
		self.segment_domain.concat(&additional.segment_domain, transform_of_additional, &id_map);
		self.region_domain.concat(&additional.region_domain, transform_of_additional, &id_map);

		// TODO: properly deal with fills such as gradients
		self.style = additional.style.clone();

		self.colinear_manipulators.extend(additional.colinear_manipulators.iter().copied());
	}
}

impl BoundingBox for Table<Vector> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let bounds = self
			.iter()
			.flat_map(|row| {
				if !include_stroke {
					return row.element.bounding_box_with_transform(transform * *row.transform);
				}

				let stroke_width = row.element.style.stroke().map(|s| s.weight()).unwrap_or_default();

				let miter_limit = row.element.style.stroke().map(|s| s.join_miter_limit).unwrap_or(1.);

				let scale = transform.decompose_scale();

				// We use the full line width here to account for different styles of stroke caps
				let offset = DVec2::splat(stroke_width * scale.x.max(scale.y) * miter_limit);

				row.element.bounding_box_with_transform(transform * *row.transform).map(|[a, b]| [a - offset, b + offset])
			})
			.reduce(Quad::combine_bounds);

		match bounds {
			Some(bounds) => RenderBoundingBox::Rectangle(bounds),
			None => RenderBoundingBox::None,
		}
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_vector<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Vector>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Debug, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
	pub struct OldVectorData {
		pub transform: DAffine2,
		pub alpha_blending: AlphaBlending,

		pub style: PathStyle,

		pub colinear_manipulators: Vec<[HandleId; 2]>,

		pub point_domain: PointDomain,
		pub segment_domain: SegmentDomain,
		pub region_domain: RegionDomain,

		pub upstream_graphic_group: Option<Table<Graphic>>,
	}

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	pub struct OldTable<T> {
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<T>,
		transform: Vec<DAffine2>,
		alpha_blending: Vec<AlphaBlending>,
	}

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	pub struct OlderTable<T> {
		id: Vec<u64>,
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<T>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	#[allow(clippy::large_enum_variant)]
	enum VectorFormat {
		Vector(Vector),
		OldVectorData(OldVectorData),
		OldVectorTable(OldTable<Vector>),
		OlderVectorTable(OlderTable<Vector>),
		VectorTable(Table<Vector>),
	}

	Ok(match VectorFormat::deserialize(deserializer)? {
		VectorFormat::Vector(vector) => Table::new_from_element(vector),
		VectorFormat::OldVectorData(old) => {
			let mut vector_table = Table::new_from_element(Vector {
				style: old.style,
				colinear_manipulators: old.colinear_manipulators,
				point_domain: old.point_domain,
				segment_domain: old.segment_domain,
				region_domain: old.region_domain,
				upstream_nested_layers: old.upstream_graphic_group,
			});
			*vector_table.iter_mut().next().unwrap().transform = old.transform;
			*vector_table.iter_mut().next().unwrap().alpha_blending = old.alpha_blending;
			vector_table
		}
		VectorFormat::OlderVectorTable(older_table) => older_table.element.into_iter().map(|element| TableRow { element, ..Default::default() }).collect(),
		VectorFormat::OldVectorTable(old_table) => old_table
			.element
			.into_iter()
			.zip(old_table.transform.into_iter().zip(old_table.alpha_blending))
			.map(|(element, (transform, alpha_blending))| TableRow {
				element,
				transform,
				alpha_blending,
				source_node_id: None,
			})
			.collect(),
		VectorFormat::VectorTable(vector_table) => vector_table,
	})
}

#[cfg(test)]
mod tests {
	use kurbo::{CubicBez, PathSeg, Point};

	use super::*;

	fn assert_subpath_eq(generated: &[Subpath<PointId>], expected: &[Subpath<PointId>]) {
		assert_eq!(generated.len(), expected.len());
		for (generated, expected) in generated.iter().zip(expected) {
			assert_eq!(generated.manipulator_groups().len(), expected.manipulator_groups().len());
			assert_eq!(generated.closed(), expected.closed());
			for (generated, expected) in generated.manipulator_groups().iter().zip(expected.manipulator_groups()) {
				assert_eq!(generated.in_handle, expected.in_handle);
				assert_eq!(generated.out_handle, expected.out_handle);
				assert_eq!(generated.anchor, expected.anchor);
			}
		}
	}

	#[test]
	fn construct_closed_subpath() {
		let circle = Subpath::new_ellipse(DVec2::NEG_ONE, DVec2::ONE);
		let vector = Vector::from_subpath(&circle);
		assert_eq!(vector.point_domain.ids().len(), 4);
		let bezier_paths = vector.segment_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
		assert_eq!(bezier_paths.len(), 4);
		assert!(bezier_paths.iter().all(|&bezier| circle.iter().any(|original_bezier| original_bezier == bezier)));

		let generated = vector.stroke_bezier_paths().collect::<Vec<_>>();
		assert_subpath_eq(&generated, &[circle]);
	}

	#[test]
	fn construct_open_subpath() {
		let bezier = PathSeg::Cubic(CubicBez::new(Point::ZERO, Point::new(-1., -1.), Point::new(1., 1.), Point::new(1., 0.)));
		let subpath = Subpath::from_bezier(bezier);
		let vector = Vector::from_subpath(&subpath);
		assert_eq!(vector.point_domain.ids().len(), 2);
		let bezier_paths = vector.segment_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
		assert_eq!(bezier_paths, vec![bezier]);

		let generated = vector.stroke_bezier_paths().collect::<Vec<_>>();
		assert_subpath_eq(&generated, &[subpath]);
	}

	#[test]
	fn construct_many_subpath() {
		let curve = PathSeg::Cubic(CubicBez::new(Point::ZERO, Point::new(-1., -1.), Point::new(1., 1.), Point::new(1., 0.)));
		let curve = Subpath::from_bezier(curve);
		let circle = Subpath::new_ellipse(DVec2::NEG_ONE, DVec2::ONE);

		let vector = Vector::from_subpaths([&curve, &circle], false);
		assert_eq!(vector.point_domain.ids().len(), 6);

		let bezier_paths = vector.segment_iter().map(|(_, bezier, _, _)| bezier).collect::<Vec<_>>();
		assert_eq!(bezier_paths.len(), 5);
		assert!(bezier_paths.iter().all(|&bezier| circle.iter().chain(curve.iter()).any(|original_bezier| original_bezier == bezier)));

		let generated = vector.stroke_bezier_paths().collect::<Vec<_>>();
		assert_subpath_eq(&generated, &[curve, circle]);
	}
}
