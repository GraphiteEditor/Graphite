use core_types::list::{Item, List};
use core_types::uuid::NodeId;
use core_types::{ATTR_BLEND_MODE, ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, BlendMode, Color, Ctx};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::subpath::{ManipulatorGroup, Subpath};
use graphic_types::vector_types::vector::PointId;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphic_types::vector_types::vector::style::Fill;
use graphic_types::{Graphic, Vector};
use linesweeper::topology::Topology;
use linesweeper::{BinaryOp, FillRule, binary_op};
use smallvec::SmallVec;
use vector_types::kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, PathSeg, Point, QuadBez};
pub use vector_types::vector::misc::BooleanOperation;

// TODO: Fix boolean ops to work by removing .transform() and .one_instance_*() calls,

/// Combines the geometric forms of one or more closed paths into a new vector path that results from cutting or joining the paths by the chosen method.
#[node_macro::node(category("Vector: Modifier"), memoize)]
async fn boolean_operation<I: graphic_types::IntoGraphicList + 'n + Send + Clone>(
	_: impl Ctx,
	/// The `List` of vector paths to perform the boolean operation on. Nested `List`s are automatically flattened.
	#[implementations(List<Graphic>, List<Vector>)]
	content: I,
	/// Which boolean operation to perform on the paths.
	///
	/// Union combines all paths while cutting out overlapping areas (even the interiors of a single path).
	/// Subtraction cuts overlapping areas out from the last (Subtract Front) or first (Subtract Back) path.
	/// Intersection cuts away all but the overlapping areas shared by every path.
	/// Difference cuts away the overlapping areas shared by every path, leaving only the non-overlapping areas.
	operation: BooleanOperation,
) -> List<Vector> {
	let content = content.into_graphic_list();

	// The first index is the bottom of the stack
	let flattened = flatten_vector(&content);

	let mut result_vector_list = match operation {
		BooleanOperation::Union | BooleanOperation::SubtractFront | BooleanOperation::SubtractBack | BooleanOperation::Intersect | BooleanOperation::Exclude => {
			single_pass_boolean_operation(&flattened, operation)
		}
		BooleanOperation::Trim | BooleanOperation::Crop => cascading_subtract(&flattened, operation),
	};

	// Replace the transformation matrix with a mutation of the vector points themselves
	for i in 0..result_vector_list.len() {
		let transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, i);
		result_vector_list.set_attribute(ATTR_TRANSFORM, i, DAffine2::IDENTITY);

		let result_vector = result_vector_list.element_mut(i).unwrap();
		Vector::transform(result_vector, transform);
		result_vector.style.set_stroke_transform(DAffine2::IDENTITY);

		// Snapshot the input layers as the `editor:merged_layers` attribute so the renderer can recurse into them
		// for editor click-target preservation.
		result_vector_list.set_attribute(ATTR_EDITOR_MERGED_LAYERS, i, content.clone());

		// Clean up the boolean operation result by merging duplicated points
		let merge_transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, i);
		result_vector_list.element_mut(i).unwrap().merge_by_distance_spatial(merge_transform, 0.0001);
	}

	result_vector_list
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WindingNumber {
	elems: SmallVec<[i16; 8]>,
}

impl linesweeper::topology::WindingNumber for WindingNumber {
	type Tag = (usize, usize);

	fn single((tag, out_of): (usize, usize), positive: bool) -> Self {
		let mut elems = SmallVec::with_capacity(out_of);
		elems.resize(out_of, 0);
		elems[tag] = if positive { 1 } else { -1 };
		Self { elems }
	}
}

impl std::ops::AddAssign for WindingNumber {
	fn add_assign(&mut self, rhs: Self) {
		if rhs.elems.is_empty() {
			return;
		}
		if self.elems.is_empty() {
			self.elems = rhs.elems;
		} else {
			for (me, them) in self.elems.iter_mut().zip(&rhs.elems) {
				*me += *them;
			}
		}
	}
}

impl std::ops::Add for WindingNumber {
	type Output = WindingNumber;

	fn add(mut self, rhs: Self) -> Self::Output {
		self += rhs;
		self
	}
}

impl WindingNumber {
	fn is_inside(&self, op: BooleanOperation) -> bool {
		let is_in = |w: &i16| *w != 0;
		let is_out = |w: &i16| *w == 0;
		match op {
			BooleanOperation::Union => self.elems.iter().any(is_in),
			BooleanOperation::SubtractFront => self.elems.first().is_some_and(is_in) && self.elems.iter().skip(1).all(is_out),
			BooleanOperation::SubtractBack => self.elems.last().is_some_and(is_in) && self.elems.iter().rev().skip(1).all(is_out),
			BooleanOperation::Intersect => !self.elems.is_empty() && self.elems.iter().all(is_in),
			BooleanOperation::Exclude => self.elems.iter().any(is_in) && !self.elems.iter().all(is_in),
			BooleanOperation::Trim => unreachable!(),
			BooleanOperation::Crop => unreachable!(),
		}
	}

	fn subtract_front_at(&self, i: usize) -> bool {
		let is_in = |v: &i16| *v != 0;

		self.elems.get(i).is_some_and(is_in) && self.elems.iter().skip(i + 1).all(|v| !is_in(v))
	}

	fn crop_visible_at(&self, i: usize) -> bool {
		let is_in = |v: &i16| *v != 0;

		if self.elems.is_empty() {
			return false;
		}

		let top_index = self.elems.len() - 1;

		if i >= top_index {
			return false;
		}

		self.elems.get(i).is_some_and(is_in) && self.elems.get(top_index).is_some_and(is_in) && self.elems[i + 1..top_index].iter().all(|v| !is_in(v))
	}
}

fn single_pass_boolean_operation(vector: &List<Vector>, boolean_operation: BooleanOperation) -> List<Vector> {
	let mut list = List::new();

	let copy_from_index = if matches!(boolean_operation, BooleanOperation::SubtractFront) {
		if !vector.is_empty() { Some(0) } else { None }
	} else {
		if !vector.is_empty() { Some(vector.len() - 1) } else { None }
	};
	let mut item = if let Some(index) = copy_from_index {
		let mut attributes = vector.clone_item_attributes(index);
		// The boolean op bakes input transforms into the output geometry, so the result item carries no transform of its own
		attributes.insert(ATTR_TRANSFORM, DAffine2::IDENTITY);
		let copy_from = vector.element(index).unwrap();
		let element = Vector {
			style: copy_from.style.clone(),
			..Default::default()
		};
		Item::from_parts(element, attributes)
	} else {
		Item::<Vector>::default()
	};

	let top = match try_create_topology(vector) {
		Some(top) => top,
		None => {
			list.push(item);
			return list;
		}
	};

	let contours = top.contours(|winding| winding.is_inside(boolean_operation));

	if contours.contours().next().is_some() {
		append_linesweeper_contours(item.element_mut(), &contours);
		list.push(item);
	}

	list
}

fn cascading_subtract(vector: &List<Vector>, boolean_operation: BooleanOperation) -> List<Vector> {
	let mut list = List::new();

	let top = match try_create_topology(vector) {
		Some(top) => top,
		None => return list,
	};

	for i in 0..vector.len() {
		let contours = match boolean_operation {
			BooleanOperation::Crop if i == vector.len() - 1 => top.contours(|winding| winding.is_inside(BooleanOperation::SubtractBack)),

			BooleanOperation::Crop => top.contours(|winding| winding.crop_visible_at(i)),

			_ => top.contours(|winding| winding.subtract_front_at(i)),
		};

		if contours.contours().next().is_none() {
			continue;
		}

		let source = match vector.element(i) {
			Some(source) => source,
			None => continue,
		};

		let mut attributes = vector.clone_item_attributes(i);
		attributes.insert(ATTR_TRANSFORM, DAffine2::IDENTITY);

		let mut element = Vector {
			style: source.style.clone(),
			..Default::default()
		};

		if boolean_operation == BooleanOperation::Crop && i == vector.len() - 1 {
			element.style.clear_fill();
			element.style.clear_stroke();
		}

		append_linesweeper_contours(&mut element, &contours);

		let item = Item::from_parts(element, attributes);
		list.push(item);
	}

	list
}

fn try_create_topology(vector: &List<Vector>) -> Option<Topology<WindingNumber>> {
	const EPSILON: f64 = 1e-5;

	let mut paths = Vec::new();

	for index in 0..vector.len() {
		let element = vector.element(index).unwrap();
		paths.push(to_bez_path(element, vector.attribute_cloned_or_default(ATTR_TRANSFORM, index)));
	}

	match Topology::<WindingNumber>::from_paths(paths.iter().enumerate().map(|(idx, path)| (path, (idx, paths.len()))), EPSILON) {
		Ok(top) => Some(top),
		Err(e) => {
			log::error!("Boolean operation failed while building topology: {e}");
			None
		}
	}
}

fn append_linesweeper_contours(vector: &mut Vector, contours: &linesweeper::topology::Contours) {
	// TODO: Linesweeper emits contours in the opposite winding direction from the rest of Kurbo's and Graphite's vector graphics system (clockwise in screen coordinates).
	// TODO: Report this upstream to Linesweeper and remove this `.reverse()` workaround once fixed.
	for subpath in from_bez_paths(contours.contours().map(|c| &c.path)) {
		vector.append_subpath(subpath.reverse(), false);
	}
}

fn flatten_vector(graphic_list: &List<Graphic>) -> List<Vector> {
	(0..graphic_list.len())
		.flat_map(|index| {
			let graphic = graphic_list.element(index).unwrap();
			match graphic.clone() {
				Graphic::Vector(vector) => {
					// Apply the parent graphic's transform to each element of the `List<Vector>`
					let parent_transform: DAffine2 = graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
					vector
						.into_iter()
						.map(|mut sub_vector| {
							let current_transform: DAffine2 = sub_vector.attribute_cloned_or_default(ATTR_TRANSFORM);
							*sub_vector.attribute_mut_or_insert_default(ATTR_TRANSFORM) = parent_transform * current_transform;
							sub_vector
						})
						.collect::<Vec<_>>()
				}
				Graphic::RasterCPU(image) => {
					let parent_transform: DAffine2 = graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
					let make_item = |transform, layer, blend_mode: BlendMode, opacity: f64, fill: f64, clip: bool| {
						let mut subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						let mut element = Vector::from_subpath(subpath);
						element.style.set_fill(Fill::Solid(Color::BLACK));

						Item::new_from_element(element)
							.with_attribute(ATTR_BLEND_MODE, blend_mode)
							.with_attribute(ATTR_OPACITY, opacity)
							.with_attribute(ATTR_OPACITY_FILL, fill)
							.with_attribute(ATTR_CLIPPING_MASK, clip)
							.with_attribute(ATTR_EDITOR_LAYER_PATH, layer)
					};

					// Apply the parent graphic's transform to each raster element, preserving each item's layer
					// and alpha_blending so the boolean op downstream can route clicks (and inherit blending state)
					// back to the originating raster layer
					(0..image.len())
						.map(|i| {
							let row_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM, i);
							let layer: List<NodeId> = image.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, i);
							let blend_mode: BlendMode = image.attribute_cloned_or_default(ATTR_BLEND_MODE, i);
							let opacity: f64 = image.attribute_cloned_or(ATTR_OPACITY, i, 1.);
							let fill: f64 = image.attribute_cloned_or(ATTR_OPACITY_FILL, i, 1.);
							let clip: bool = image.attribute_cloned_or_default(ATTR_CLIPPING_MASK, i);
							make_item(parent_transform * row_transform, layer, blend_mode, opacity, fill, clip)
						})
						.collect::<Vec<_>>()
				}
				Graphic::RasterGPU(image) => {
					let parent_transform: DAffine2 = graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
					let make_item = |transform, layer, blend_mode: BlendMode, opacity: f64, fill: f64, clip: bool| {
						let mut subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						let mut element = Vector::from_subpath(subpath);
						element.style.set_fill(Fill::Solid(Color::BLACK));

						Item::new_from_element(element)
							.with_attribute(ATTR_BLEND_MODE, blend_mode)
							.with_attribute(ATTR_OPACITY, opacity)
							.with_attribute(ATTR_OPACITY_FILL, fill)
							.with_attribute(ATTR_CLIPPING_MASK, clip)
							.with_attribute(ATTR_EDITOR_LAYER_PATH, layer)
					};

					// Apply the parent graphic's transform to each raster element, preserving each item's layer
					// and alpha_blending so the boolean op downstream can route clicks (and inherit blending state)
					// back to the originating raster layer
					(0..image.len())
						.map(|i| {
							let row_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM, i);
							let layer: List<NodeId> = image.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, i);
							let blend_mode: BlendMode = image.attribute_cloned_or_default(ATTR_BLEND_MODE, i);
							let opacity: f64 = image.attribute_cloned_or(ATTR_OPACITY, i, 1.);
							let fill: f64 = image.attribute_cloned_or(ATTR_OPACITY_FILL, i, 1.);
							let clip: bool = image.attribute_cloned_or_default(ATTR_CLIPPING_MASK, i);
							make_item(parent_transform * row_transform, layer, blend_mode, opacity, fill, clip)
						})
						.collect::<Vec<_>>()
				}
				Graphic::Graphic(mut graphic) => {
					let parent_transform: DAffine2 = graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
					// Apply the parent graphic's transform to each element of the inner `List`
					for transform in graphic.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
						*transform = parent_transform * *transform;
					}

					// Recursively flatten the inner `List` into the output `List<Vector>`
					let flattened = flatten_vector(&graphic);
					let unioned = single_pass_boolean_operation(&flattened, BooleanOperation::Union);

					unioned.into_iter().collect::<Vec<_>>()
				}
				Graphic::Color(color) => color
					.into_iter()
					.map(|row| {
						let (color, attributes) = row.into_parts();
						let mut element = Vector::default();
						element.style.set_fill(Fill::Solid(color));
						element.style.set_stroke_transform(DAffine2::IDENTITY);

						Item::from_parts(element, attributes)
					})
					.collect::<Vec<_>>(),
				Graphic::Gradient(gradient) => gradient
					.into_iter()
					.map(|row| {
						let (stops, attributes) = row.into_parts();
						let mut element = Vector::default();
						element.style.set_fill(Fill::Gradient(graphic_types::vector_types::gradient::Gradient { stops, ..Default::default() }));
						element.style.set_stroke_transform(DAffine2::IDENTITY);

						Item::from_parts(element, attributes)
					})
					.collect::<Vec<_>>(),
			}
		})
		.collect()
}

// This quantization should potentially be removed since it's not conceptually necessary,
// but without it, the oak leaf in the Changing Seasons demo artwork is funky because
// quantization is needed for the top and bottom points to line up vertically.
fn quantize_segment(seg: PathSeg) -> PathSeg {
	const QUANTIZE_EPS: f64 = 1e-8;
	fn q(p: Point) -> Point {
		Point::new((p.x / QUANTIZE_EPS).round() * QUANTIZE_EPS, (p.y / QUANTIZE_EPS).round() * QUANTIZE_EPS)
	}

	match seg {
		PathSeg::Line(s) => PathSeg::Line(Line::new(q(s.p0), q(s.p1))),
		PathSeg::Quad(s) => PathSeg::Quad(QuadBez::new(q(s.p0), q(s.p1), q(s.p2))),
		PathSeg::Cubic(s) => PathSeg::Cubic(CubicBez::new(q(s.p0), q(s.p1), q(s.p2), q(s.p3))),
	}
}

fn to_bez_path(vector: &Vector, transform: DAffine2) -> BezPath {
	let mut path = BezPath::new();
	for subpath in vector.stroke_bezier_paths() {
		push_subpath(&mut path, &subpath, transform);
	}
	path
}

fn push_subpath(path: &mut BezPath, subpath: &Subpath<PointId>, transform: DAffine2) {
	let transform = Affine::new(transform.to_cols_array());
	let mut first = true;

	for seg in subpath.iter_closed() {
		let quantized = quantize_segment(transform * seg);
		if first {
			first = false;
			path.move_to(quantized.start());
		}
		path.push(quantized.as_path_el());
	}
	path.close_path();
}

fn from_bez_paths<'a>(paths: impl Iterator<Item = &'a BezPath>) -> Vec<Subpath<PointId>> {
	let mut all_subpaths = Vec::new();

	for path in paths {
		let cubics: Vec<CubicBez> = path.segments().map(|segment| segment.to_cubic()).collect();
		let mut manipulators_list = Vec::new();
		let mut current_start = None;

		for (index, cubic) in cubics.iter().enumerate() {
			let d = |p: Point| DVec2::new(p.x, p.y);
			let [start, handle1, handle2, end] = [d(cubic.p0), d(cubic.p1), d(cubic.p2), d(cubic.p3)];

			if current_start.is_none() {
				// Use the correct in-handle (None) and out-handle for the start point
				manipulators_list.push(ManipulatorGroup::new(start, None, Some(handle1)));
			} else {
				// Update the out-handle of the previous point
				if let Some(last) = manipulators_list.last_mut() {
					last.out_handle = Some(handle1);
				}
			}

			// Add the end point with the correct in-handle and out-handle (None)
			manipulators_list.push(ManipulatorGroup::new(end, Some(handle2), None));

			current_start = Some(end);

			// Check if this is the last segment
			if index == cubics.len() - 1 {
				all_subpaths.push(Subpath::new(manipulators_list, true));
				manipulators_list = Vec::new(); // Reset manipulators for the next path
			}
		}
	}

	all_subpaths
}

pub fn boolean_intersect(a: &BezPath, b: &BezPath) -> Vec<BezPath> {
	match binary_op(a, b, FillRule::NonZero, BinaryOp::Intersection) {
		Ok(contours) => contours.contours().map(|c| c.path.clone()).collect(),
		Err(e) => {
			log::error!("Boolean Operation failed (a: {} segments, b: {} segments): {e}", a.segments().count(), b.segments().count());
			Vec::new()
		}
	}
}
//TODO: Add styles for inputs and style asserts for outputs once the requirements are defined
#[cfg(test)]
mod test {
	use super::*;
	use core_types::OwnedContextImpl;
	use core_types::list::{Item, List};
	use kurbo::{DEFAULT_ACCURACY, Rect, Shape};
	use vector_types::Vector;

	fn create_input_shapes(include_third_shape: bool) -> List<Vector> {
		let square = Vector::from_bezpath(Rect::new(-4., -4., 4., 4.).to_path(DEFAULT_ACCURACY));
		let rectangle = Vector::from_bezpath(Rect::new(2., -2., 8., 2.).to_path(DEFAULT_ACCURACY));
		let mut shapes = List::new_from_element(square);
		shapes.push(Item::new_from_element(rectangle));

		if include_third_shape {
			let rectangle = Vector::from_bezpath(Rect::new(-2., -6., 5., 0.).to_path(DEFAULT_ACCURACY));
			shapes.push(Item::new_from_element(rectangle));
		}

		shapes
	}

	fn create_no_overlap_input_shapes() -> List<Vector> {
		let square = Vector::from_bezpath(Rect::new(-4., -4., 4., 4.).to_path(DEFAULT_ACCURACY));
		let rectangle = Vector::from_bezpath(Rect::new(5., -2., 5., 2.).to_path(DEFAULT_ACCURACY));
		let mut shapes = List::new_from_element(square);
		shapes.push(Item::new_from_element(rectangle));

		shapes
	}

	fn assert_anchor_positions(vector: &Vector, expected_anchors: &[DVec2]) {
		const EPSILON: f64 = 1e-5;
		let anchors = vector.point_domain.positions();

		assert_eq!(anchors.len(), expected_anchors.len(), "Anchor count mismatch");

		for (i, expected) in expected_anchors.iter().enumerate() {
			let actual = anchors[i];
			let distance = (actual - *expected).length();

			assert!(distance < EPSILON, "Anchor {i} mismatch: expected {expected:?}, got {actual:?}, distance {distance}");
		}
	}

	fn assert_shapes_geometry(generated: &List<Vector>, expected_anchors: Vec<Vec<DVec2>>) {
		assert_eq!(generated.len(), expected_anchors.len(), "Shape count mismatch");

		for (i, expected) in expected_anchors.iter().enumerate() {
			let result_shape = generated.element(i).unwrap();

			assert_anchor_positions(result_shape, &expected);

			assert_eq!(result_shape.segment_domain.ids().len(), expected.len(), "Segment count mismatch");
			assert_eq!(result_shape.segment_domain.end_point().last(), Some(&0), "The result shape is not closed");
		}
	}

	#[tokio::test]
	async fn union() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_input_shapes(false);
		let generated = boolean_operation(context, shapes, BooleanOperation::Union).await;

		let expected_anchors = vec![vec![
			DVec2::new(-4., -4.),
			DVec2::new(4., -4.),
			DVec2::new(4., -2.),
			DVec2::new(8., -2.),
			DVec2::new(8., 2.),
			DVec2::new(4., 2.),
			DVec2::new(4., 4.),
			DVec2::new(-4., 4.),
		]];

		assert_shapes_geometry(&generated, expected_anchors);
	}

	#[tokio::test]
	async fn subtract_front() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_input_shapes(false);
		let generated = boolean_operation(context, shapes, BooleanOperation::SubtractFront).await;

		let expected_anchors = vec![vec![
			DVec2::new(-4., -4.),
			DVec2::new(4., -4.),
			DVec2::new(4., -2.),
			DVec2::new(2., -2.),
			DVec2::new(2., 2.),
			DVec2::new(4., 2.),
			DVec2::new(4., 4.),
			DVec2::new(-4., 4.),
		]];

		assert_shapes_geometry(&generated, expected_anchors);
	}

	#[tokio::test]
	async fn subtract_back() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_input_shapes(false);
		let generated = boolean_operation(context, shapes, BooleanOperation::SubtractBack).await;

		let expected_anchors = vec![vec![DVec2::new(4., -2.), DVec2::new(8., -2.), DVec2::new(8., 2.), DVec2::new(4., 2.)]];

		assert_shapes_geometry(&generated, expected_anchors);
	}

	#[tokio::test]
	async fn intersect() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_input_shapes(false);
		let generated = boolean_operation(context, shapes, BooleanOperation::Intersect).await;

		let expected_anchors = vec![vec![DVec2::new(2., -2.), DVec2::new(4., -2.), DVec2::new(4., 2.), DVec2::new(2., 2.)]];

		assert_shapes_geometry(&generated, expected_anchors);
	}

	#[tokio::test]
	async fn intersect_no_overlap() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_no_overlap_input_shapes();
		let generated = boolean_operation(context, shapes, BooleanOperation::Intersect).await;

		assert_eq!(generated.len(), 0);
	}

	#[tokio::test]
	async fn exclude() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_input_shapes(false);
		let generated = boolean_operation(context, shapes, BooleanOperation::Exclude).await;

		let expected_anchors = [
			DVec2::new(-4., -4.),
			DVec2::new(4., -4.),
			DVec2::new(4., -2.),
			DVec2::new(2., -2.),
			DVec2::new(2., 2.),
			DVec2::new(4., 2.),
			DVec2::new(4., 4.),
			DVec2::new(-4., 4.),
			DVec2::new(8., -2.),
			DVec2::new(8., 2.),
		];

		assert_eq!(generated.len(), 1);
		let result_shape = generated.element(0).unwrap();

		assert_anchor_positions(result_shape, &expected_anchors);

		assert_eq!(result_shape.segment_domain.ids().len(), 12);
		assert_eq!(result_shape.region_domain.ids().len(), 2);
		assert_eq!(result_shape.region_domain.segment_range().len(), 2);
	}

	#[tokio::test]
	async fn trim() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_input_shapes(true);
		let generated = boolean_operation(context, shapes, BooleanOperation::Trim).await;

		let expected_anchors = vec![
			vec![
				DVec2::new(-4., -4.),
				DVec2::new(-2., -4.),
				DVec2::new(-2., 0.),
				DVec2::new(2., 0.),
				DVec2::new(2., 2.),
				DVec2::new(4., 2.),
				DVec2::new(4., 4.),
				DVec2::new(-4., 4.),
			],
			vec![
				DVec2::new(5., -2.),
				DVec2::new(8., -2.),
				DVec2::new(8., 2.),
				DVec2::new(4., 2.),
				DVec2::new(2., 2.),
				DVec2::new(2., 0.),
				DVec2::new(4., 0.),
				DVec2::new(5., 0.),
			],
			vec![
				DVec2::new(-2., -6.),
				DVec2::new(5., -6.),
				DVec2::new(5., -2.),
				DVec2::new(5., 0.),
				DVec2::new(4., 0.),
				DVec2::new(2., 0.),
				DVec2::new(-2., 0.),
				DVec2::new(-2., -4.),
			],
		];

		assert_shapes_geometry(&generated, expected_anchors);
	}

	#[tokio::test]
	async fn crop() {
		let context = OwnedContextImpl::default().into_context();
		let shapes = create_input_shapes(true);
		let generated = boolean_operation(context, shapes, BooleanOperation::Crop).await;

		let expected_anchors = vec![
			vec![
				DVec2::new(-2., -4.),
				DVec2::new(4., -4.),
				DVec2::new(4., -2.),
				DVec2::new(2., -2.),
				DVec2::new(2., 0.),
				DVec2::new(-2., 0.),
			],
			vec![
				DVec2::new(2., -2.),
				DVec2::new(4., -2.),
				DVec2::new(5., -2.),
				DVec2::new(5., 0.),
				DVec2::new(4., 0.),
				DVec2::new(2., 0.),
			],
			vec![
				DVec2::new(-2., -6.),
				DVec2::new(5., -6.),
				DVec2::new(5., -2.),
				DVec2::new(4., -2.),
				DVec2::new(4., -4.),
				DVec2::new(-2., -4.),
			],
		];

		assert_shapes_geometry(&generated, expected_anchors);
	}
}
