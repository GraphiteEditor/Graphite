use core_types::list::{Item, List};
use core_types::uuid::NodeId;
use core_types::{
	ATTR_BLEND_MODE, ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_GRADIENT_TYPE, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_SPREAD_METHOD, ATTR_TRANSFORM, BlendMode, Color,
	Ctx,
};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::gradient::{Gradient, GradientSpreadMethod, GradientType};
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
// TODO: since before we used a Vec of single-item `List`s and now we use a single `List`
// TODO: with multiple items while still assuming a single item for the boolean operations.

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
	let mut result_vector_list = boolean_operation_on_vector_list(&flattened, operation);

	// Replace the transformation matrix with a mutation of the vector points themselves
	if result_vector_list.element_mut(0).is_some() {
		let transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		result_vector_list.set_attribute(ATTR_TRANSFORM, 0, DAffine2::IDENTITY);

		let result_vector = result_vector_list.element_mut(0).unwrap();
		Vector::transform(result_vector, transform);
		result_vector.style.set_stroke_transform(DAffine2::IDENTITY);

		// Snapshot the input layers as the `editor:merged_layers` attribute so the renderer can recurse into them
		// for editor click-target preservation.
		result_vector_list.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, content.clone());

		// Clean up the boolean operation result by merging duplicated points
		let merge_transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		result_vector_list.element_mut(0).unwrap().merge_by_distance_spatial(merge_transform, 0.0001);
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
			BooleanOperation::Difference => self.elems.iter().any(is_in) && !self.elems.iter().all(is_in),
		}
	}
}

fn boolean_operation_on_vector_list(vector: &List<Vector>, boolean_operation: BooleanOperation) -> List<Vector> {
	const EPSILON: f64 = 1e-5;
	let mut list = List::new();
	let mut paths = Vec::new();

	let copy_from_index = if matches!(boolean_operation, BooleanOperation::SubtractFront) {
		if !vector.is_empty() { Some(0) } else { None }
	} else {
		if !vector.is_empty() { Some(vector.len() - 1) } else { None }
	};
	let mut row = if let Some(index) = copy_from_index {
		let mut attributes = vector.clone_item_attributes(index);
		let copy_from_transform: DAffine2 = vector.attribute_cloned_or_default(ATTR_TRANSFORM, index);
		// The boolean op bakes input transforms into the output geometry, so the result item carries no transform of its own
		attributes.insert(ATTR_TRANSFORM, DAffine2::IDENTITY);
		let copy_from = vector.element(index).unwrap();
		let mut element = Vector {
			style: copy_from.style.clone(),
			..Default::default()
		};
		// An absolute gradient lives in the geometry's space, so bake the same transform into it to track the baked points
		if let Fill::Gradient(gradient) = element.style.fill_mut()
			&& gradient.absolute
		{
			gradient.transform = copy_from_transform * gradient.transform;
		}
		Item::from_parts(element, attributes)
	} else {
		Item::<Vector>::default()
	};

	for index in 0..vector.len() {
		let element = vector.element(index).unwrap();
		paths.push(to_bez_path(element, vector.attribute_cloned_or_default(ATTR_TRANSFORM, index)));
	}

	let top = match Topology::<WindingNumber>::from_paths(paths.iter().enumerate().map(|(idx, path)| (path, (idx, paths.len()))), EPSILON) {
		Ok(top) => top,
		Err(e) => {
			log::error!("Boolean operation failed while building topology: {e}");
			list.push(row);
			return list;
		}
	};
	let contours = top.contours(|winding| winding.is_inside(boolean_operation));

	// TODO: Linesweeper emits contours in the opposite winding direction from the rest of Kurbo's and Graphite's vector graphics system (clockwise in screen coordinates).
	// TODO: Report this upstream to Linesweeper and remove this `.reverse()` workaround once fixed.
	for subpath in from_bez_paths(contours.contours().map(|c| &c.path)) {
		row.element_mut().append_subpath(subpath.reverse(), false);
	}

	list.push(row);
	list
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
					let unioned = boolean_operation_on_vector_list(&flattened, BooleanOperation::Union);

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
						// Convert the gradient's transform to absolute endpoints, matching `From<List<GradientStops>> for Fill`
						let transform = attributes.get::<DAffine2>(ATTR_TRANSFORM).cloned().unwrap_or_default();
						element.style.set_fill(Fill::Gradient(Gradient {
							stops,
							gradient_type: attributes.get::<GradientType>(ATTR_GRADIENT_TYPE).cloned().unwrap_or_default(),
							spread_method: attributes.get::<GradientSpreadMethod>(ATTR_SPREAD_METHOD).cloned().unwrap_or_default(),
							start: transform.transform_point2(DVec2::ZERO),
							end: transform.transform_point2(DVec2::X),
							absolute: true,
							transform: DAffine2::IDENTITY,
						}));
						element.style.set_stroke_transform(DAffine2::IDENTITY);

						Item::from_parts(element, attributes)
					})
					.collect::<Vec<_>>(),
				Graphic::Text(text) => {
					// Shape the glyphs into vectors (each item's own transform is applied), then compose the parent's transform like the other arms
					let parent_transform: DAffine2 = graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
					text_nodes::shape_text_list(&text, false)
						.into_iter()
						.map(|mut sub_vector| {
							let current_transform: DAffine2 = sub_vector.attribute_cloned_or_default(ATTR_TRANSFORM);
							*sub_vector.attribute_mut_or_insert_default(ATTR_TRANSFORM) = parent_transform * current_transform;
							sub_vector
						})
						.collect::<Vec<_>>()
				}
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
