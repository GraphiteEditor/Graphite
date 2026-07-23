use core_types::attr::{self, Attr};
use core_types::list::{Item, ItemAttributeValues, List};
use core_types::{Color, Ctx};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{bake_paint_transforms, set_paint_attribute};
use graphic_types::vector_types::subpath::{ManipulatorGroup, Subpath};
use graphic_types::vector_types::vector::PointId;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
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
async fn boolean_operation<I: graphic_types::IntoGraphicList>(
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
	operation: Item<BooleanOperation>,
) -> Item<Vector> {
	let operation = operation.into_element();
	let content = content.into_graphic_list();

	// The first index is the bottom of the stack
	let flattened = flatten_vector(&content);
	let mut result_vector_list = boolean_operation_on_vector_list(&flattened, operation);

	// Replace the transformation matrix with a mutation of the vector points themselves
	if result_vector_list.element_mut(0).is_some() {
		let transform = result_vector_list.attr_cloned_or_default::<attr::Transform>(0);
		result_vector_list.set_attr::<attr::Transform>(0, DAffine2::IDENTITY);

		let result_vector = result_vector_list.element_mut(0).unwrap();
		Vector::transform(result_vector, transform);
		result_vector.set_stroke_transform(DAffine2::IDENTITY);

		// Snapshot the input layers as the `editor:merged_layers` attribute so the renderer can recurse into them
		// for editor click-target preservation.
		result_vector_list.set_attr::<graphic_types::attr::editor::MergedLayers>(0, content.clone());

		// Clean up the boolean operation result by merging duplicated points
		let merge_transform = result_vector_list.attr_cloned_or_default::<attr::Transform>(0);
		result_vector_list.element_mut(0).unwrap().merge_by_distance_spatial(merge_transform, 0.0001);
	}

	result_vector_list.into_iter().next().unwrap_or_default()
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

	fn of_tag(&self, (tag, out_of): Self::Tag) -> Self {
		let mut elems = SmallVec::with_capacity(out_of);
		elems.resize(out_of, 0);
		if let (Some(slot), Some(&value)) = (elems.get_mut(tag), self.elems.get(tag)) {
			*slot = value;
		} else {
			log::warn!("WindingNumber::of_tag: tag {tag} out of bounds (out_of {out_of}, len {})", self.elems.len());
		}
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
		let copy_from_transform = vector.attr_cloned_or_default::<attr::Transform>(index);
		// The boolean op bakes input transforms into the output geometry, so the result item carries no transform of its own
		attributes.set_attr::<attr::Transform>(DAffine2::IDENTITY);

		bake_paint_transforms(&mut attributes, copy_from_transform);

		let copy_from = vector.element(index).unwrap();
		let element = Vector {
			stroke: copy_from.stroke.clone(),
			..Default::default()
		};
		Item::from_parts(element, attributes)
	} else {
		Item::<Vector>::default()
	};

	for index in 0..vector.len() {
		let element = vector.element(index).unwrap();
		paths.push(to_bez_path(element, vector.attr_cloned_or_default::<attr::Transform>(index)));
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
	for subpath in from_bez_paths(contours.contours().map(|c| &c.path)) {
		row.element_mut().append_subpath(subpath, false);
	}

	list.push(row);
	list
}

fn flatten_vector(graphic_list: &List<Graphic>) -> List<Vector> {
	(0..graphic_list.len())
		.flat_map(|index| {
			let graphic = graphic_list.element(index).unwrap();
			match graphic.clone() {
				Graphic::None => Vec::new(),
				Graphic::Vector(vector) => {
					// Apply the parent graphic's transform to each element of the `List<Vector>`
					let parent_transform = graphic_list.attr_cloned_or_default::<attr::Transform>(index);
					vector
						.into_iter()
						.map(|mut sub_vector| {
							let current_transform = sub_vector.attr_cloned_or_default::<attr::Transform>();
							*sub_vector.attr_mut_or_insert_default::<attr::Transform>() = parent_transform * current_transform;
							sub_vector
						})
						.collect::<Vec<_>>()
				}
				Graphic::RasterCPU(image) => {
					let parent_transform = graphic_list.attr_cloned_or_default::<attr::Transform>(index);
					let make_item = |transform: DAffine2, source_attributes: &ItemAttributeValues| {
						let mut subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						let element = Vector::from_subpath(subpath);

						let mut item = Item::new_from_element(element);
						for key in [
							attr::BlendMode::name(),
							attr::Opacity::name(),
							attr::OpacityFill::name(),
							attr::ClippingMask::name(),
							attr::editor::LayerPath::name(),
						] {
							item.attributes_mut().insert_cloned_from(source_attributes, key);
						}
						set_paint_attribute::<graphic_types::attr::Fill>(item.attributes_mut(), List::new_from_element(Color::BLACK));
						item
					};

					// Apply the parent graphic's transform to each raster element, preserving each item's layer
					// and alpha_blending so the boolean op downstream can route clicks (and inherit blending state)
					// back to the originating raster layer
					(0..image.len())
						.map(|i| {
							let row_transform = image.attr_cloned_or_default::<attr::Transform>(i);
							let source_attributes = image.clone_item_attributes(i);
							make_item(parent_transform * row_transform, &source_attributes)
						})
						.collect::<Vec<_>>()
				}
				Graphic::RasterGPU(image) => {
					let parent_transform = graphic_list.attr_cloned_or_default::<attr::Transform>(index);
					let make_item = |transform: DAffine2, source_attributes: &ItemAttributeValues| {
						let mut subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						let element = Vector::from_subpath(subpath);

						let mut item = Item::new_from_element(element);
						for key in [
							attr::BlendMode::name(),
							attr::Opacity::name(),
							attr::OpacityFill::name(),
							attr::ClippingMask::name(),
							attr::editor::LayerPath::name(),
						] {
							item.attributes_mut().insert_cloned_from(source_attributes, key);
						}
						set_paint_attribute::<graphic_types::attr::Fill>(item.attributes_mut(), List::new_from_element(Color::BLACK));
						item
					};

					// Apply the parent graphic's transform to each raster element, preserving each item's layer
					// and alpha_blending so the boolean op downstream can route clicks (and inherit blending state)
					// back to the originating raster layer
					(0..image.len())
						.map(|i| {
							let row_transform = image.attr_cloned_or_default::<attr::Transform>(i);
							let source_attributes = image.clone_item_attributes(i);
							make_item(parent_transform * row_transform, &source_attributes)
						})
						.collect::<Vec<_>>()
				}
				Graphic::Graphic(mut graphic) => {
					let parent_transform = graphic_list.attr_cloned_or_default::<attr::Transform>(index);
					// Apply the parent graphic's transform to each element of the inner `List`
					for transform in graphic.iter_attr_values_mut_or_default::<attr::Transform>() {
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
						let (color, mut attributes) = row.into_parts();
						set_paint_attribute::<graphic_types::attr::Fill>(&mut attributes, List::new_from_element(color));

						let mut element = Vector::default();
						element.set_stroke_transform(DAffine2::IDENTITY);

						Item::from_parts(element, attributes)
					})
					.collect::<Vec<_>>(),
				Graphic::Gradient(gradient) => gradient
					.into_iter()
					.map(|row| {
						let (stops, mut attributes) = row.into_parts();

						let mut gradient_paint = List::new_from_element(stops);
						if let Some(transform) = attributes.remove_attr::<attr::Transform>() {
							gradient_paint.set_attr::<attr::Transform>(0, transform);
						}
						if let Some(gradient_type) = attributes.remove_attr::<vector_types::attr::GradientType>() {
							gradient_paint.set_attr::<vector_types::attr::GradientType>(0, gradient_type);
						}
						if let Some(spread_method) = attributes.remove_attr::<vector_types::attr::SpreadMethod>() {
							gradient_paint.set_attr::<vector_types::attr::SpreadMethod>(0, spread_method);
						}
						set_paint_attribute::<graphic_types::attr::Fill>(&mut attributes, gradient_paint);

						let mut element = Vector::default();
						element.set_stroke_transform(DAffine2::IDENTITY);

						Item::from_parts(element, attributes)
					})
					.collect::<Vec<_>>(),
				Graphic::Text(text) => {
					// Shape the glyphs into vectors (each item's own transform is applied), then compose the parent's transform like the other arms
					let parent_transform = graphic_list.attr_cloned_or_default::<attr::Transform>(index);
					text_nodes::shape_text_list(&text, false)
						.into_iter()
						.map(|mut sub_vector| {
							let current_transform = sub_vector.attr_cloned_or_default::<attr::Transform>();
							*sub_vector.attr_mut_or_insert_default::<attr::Transform>() = parent_transform * current_transform;
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
