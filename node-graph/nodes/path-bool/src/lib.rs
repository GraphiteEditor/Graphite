use core_types::list::ATTR_PATHS;
use core_types::list::{ATTR_APPEARANCE, Item, List, NodeIdPath};
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Ctx};
use glam::DAffine2;
use graphic_types::Appearance;
use graphic_types::graphic::bake_paint_transforms;
use graphic_types::vector_types::vector::algorithms::weld::WeldExt;
use graphic_types::vector_types::vector::{Path, SetVectorPaths, no_paths, paths_from_list_row_attributes, set_paths_at};
use graphic_types::{Graphic, Vector};
use linesweeper::topology::Topology;
use linesweeper::{BinaryOp, FillRule, binary_op};
use smallvec::SmallVec;
use vector_types::kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, PathEl, PathSeg, Point, QuadBez, Shape};
pub use vector_types::vector::misc::BooleanOperation;

/// Why a boolean operation gives no result.
#[derive(Debug, thiserror::Error)]
pub enum BooleanOperationError {
	#[error("An operand has coordinates that aren't finite")]
	NonFiniteCoordinates,
	#[error(transparent)]
	Text(#[from] text_nodes::TextToVectorError),
	#[error("The operands can't be combined because {0}")]
	Topology(#[from] linesweeper::Error),
}

// TODO: Fix boolean ops to work by removing .transform() and .one_instance_*() calls,
// TODO: since before we used a Vec of single-item `List`s and now we use a single `List`
// TODO: with multiple items while still assuming a single item for the boolean operations.

/// Combines the geometric forms of one or more closed paths into a new vector path that results from cutting or joining the paths by the chosen method.
#[node_macro::node(category("Vector: Path"), icon("NodeBooleanOperation"), memoize)]
fn boolean_operation(
	_: impl Ctx,
	/// The `List` of vector paths to perform the boolean operation on. Nested `List`s are automatically flattened.
	content: List<Graphic>,
	/// Which boolean operation to perform on the paths.
	///
	/// Union combines all paths while cutting out overlapping areas (even the interiors of a single path).
	/// Subtraction cuts overlapping areas out from the last (Subtract Front) or first (Subtract Back) path.
	/// Intersection cuts away all but the overlapping areas shared by every path.
	/// Difference cuts away the areas where an even number of paths overlap, keeping those where an odd number do.
	operation: Item<BooleanOperation>,
) -> Result<Item<Vector>, BooleanOperationError> {
	let operation = operation.into_element();

	// The first index is the bottom of the stack
	let flattened = flatten_vector(&content)?;
	let mut result_vector_list = boolean_operation_on_vector_list(&flattened, operation)?;

	// Replace the transformation matrix with a mutation of the vector points themselves
	if result_vector_list.element_mut(0).is_some() {
		let transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		result_vector_list.set_attribute(ATTR_TRANSFORM, 0, DAffine2::IDENTITY);

		let result_vector = result_vector_list.element_mut(0).unwrap();
		Vector::transform(result_vector, transform);

		// The geometry is baked into identity space, so a copied stroke authoring space would be stale
		if let Some(appearance) = result_vector_list.attribute::<Appearance>(ATTR_APPEARANCE, 0) {
			let mut appearance = appearance.clone();
			for coverage in appearance.0.iter_element_values_mut() {
				coverage.0.remove_attribute::<DAffine2>(ATTR_TRANSFORM);
			}
			result_vector_list.set_attribute(ATTR_APPEARANCE, 0, appearance);
		}

		// Snapshot the input layers as the `editor:merged_layers` attribute so the renderer can recurse into them
		// for editor click-target preservation.
		result_vector_list.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, content.clone());

		// Weld duplicated points within each contour only, since welding contours that touch would make the result fill as a mesh, holes included
		let merge_transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		let mut result_paths = result_vector_list.attribute::<List<Path>>(ATTR_PATHS, 0).cloned().unwrap_or_default();
		result_vector_list.element_mut(0).unwrap().weld_within_pieces(&mut result_paths, merge_transform, 0.0001);
		set_paths_at(&mut result_vector_list, 0, result_paths);
	}

	Ok(result_vector_list.into_iter().next().unwrap_or_default())
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
				*me = me.wrapping_add(*them);
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
			BooleanOperation::Difference => self.elems.iter().filter(|&winding| is_in(winding)).count() % 2 == 1,
		}
	}
}

fn boolean_operation_on_vector_list(vector: &List<Vector>, boolean_operation: BooleanOperation) -> Result<List<Vector>, BooleanOperationError> {
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
		// The result weaves its own paths rather than inheriting the operand's
		attributes.remove::<List<Path>>(ATTR_PATHS);

		bake_paint_transforms(&mut attributes, copy_from_transform);

		Item::from_parts(Vector::default(), attributes)
	} else {
		Item::<Vector>::default()
	};

	for index in 0..vector.len() {
		let element = vector.element(index).unwrap();
		let element_paths = paths_from_list_row_attributes(vector, index).unwrap_or_else(|| no_paths());
		paths.push(to_bez_path(element, element_paths, vector.attribute_cloned_or_default(ATTR_TRANSFORM, index))?);
	}

	// Far from the origin, the tolerance grows with the coordinates' magnitude like linesweeper's own `binary_op`, since the sweep can't resolve less
	let bounds = paths.iter().map(Shape::bounding_box).reduce(|a, b| a.union(b)).unwrap_or_default();
	let magnitude = bounds.min_x().min(bounds.min_y()).abs().max(bounds.max_x().max(bounds.max_y()).abs());
	let epsilon = EPSILON.max(magnitude * f64::EPSILON * 64.);

	let top = Topology::<WindingNumber>::from_paths(paths.iter().enumerate().map(|(idx, path)| (path, (idx, paths.len()))), epsilon).map_err(linesweeper::Error::from)?;
	let contours = top.contours(|winding| winding.is_inside(boolean_operation));
	let mut result_paths = List::new();
	let closed_contours = contours.contours().map(|contour| closed(contour.path.clone()));
	row.element_mut().append_bezpaths(&mut result_paths, closed_contours);
	row.set_paths(result_paths);

	list.push(row);
	Ok(list)
}

/// Flattens graphic content into the operands of a boolean operation, matching `flatten_graphic_list`'s attribute composition.
/// Non-path content is dropped, since it contributes no region to operate on.
fn flatten_vector(graphic_list: &List<Graphic>) -> Result<List<Vector>, BooleanOperationError> {
	let operands = (0..graphic_list.len())
		.map(|index| {
			let graphic = graphic_list.element(index).unwrap();

			// Whether the parent carries each attribute is a structural fact, so an absent one never invents a column on the children
			let parent_has_transform = graphic_list.attribute::<DAffine2>(ATTR_TRANSFORM, index).is_some();
			let parent_has_opacity = graphic_list.attribute::<f64>(ATTR_OPACITY, index).is_some();
			let parent_has_fill = graphic_list.attribute::<f64>(ATTR_OPACITY_FILL, index).is_some();
			let parent_has_layer_path = graphic_list.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH, index).is_some();

			let parent_transform: DAffine2 = graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let parent_opacity: f64 = graphic_list.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let parent_fill: f64 = graphic_list.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let layer_path: NodeIdPath = graphic_list.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, index);
			let parent_appearance = graphic_list.attribute::<Appearance>(ATTR_APPEARANCE, index).and_then(Appearance::declared).cloned();

			let compose_parent = |mut item: Item<Vector>| {
				if parent_has_transform || item.attribute::<DAffine2>(ATTR_TRANSFORM).is_some() {
					let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
					item.set_attribute(ATTR_TRANSFORM, parent_transform * item_transform);
				}
				if parent_has_opacity || item.attribute::<f64>(ATTR_OPACITY).is_some() {
					let item_opacity: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
					item.set_attribute(ATTR_OPACITY, parent_opacity * item_opacity);
				}
				if parent_has_fill || item.attribute::<f64>(ATTR_OPACITY_FILL).is_some() {
					let item_fill: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
					item.set_attribute(ATTR_OPACITY_FILL, parent_fill * item_fill);
				}
				if parent_has_layer_path {
					item.set_attribute(ATTR_EDITOR_LAYER_PATH, layer_path.clone());
				}
				// Appearance cascades into each child whose own is undeclared, since a declared child wins wholesale
				if let Some(appearance) = &parent_appearance
					&& item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared).is_none()
				{
					item.set_attribute(ATTR_APPEARANCE, appearance.clone());
				}

				item
			};

			// A boxed single graphic is the rank-0 version of the same nesting, so it flattens through the group path
			let graphic = match graphic.clone() {
				Graphic::Graphic(item) => Graphic::GraphicList(List::new_from_item(*item)),
				other => other,
			};

			Ok(match graphic {
				Graphic::Vector(item) => vec![compose_parent(*item)],
				Graphic::VectorList(vector) => vector.into_iter().map(compose_parent).collect::<Vec<_>>(),
				Graphic::Text(item) => text_nodes::shape_text_list(&List::new_from_item(item), false)?.into_iter().map(compose_parent).collect::<Vec<_>>(),
				Graphic::TextList(text) => text_nodes::shape_text_list(&text, false)?.into_iter().map(compose_parent).collect::<Vec<_>>(),
				Graphic::GraphicList(mut graphic) => {
					if parent_has_transform {
						for transform in graphic.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
							*transform = parent_transform * *transform;
						}
					}
					if parent_has_opacity {
						for opacity in graphic.iter_attribute_values_mut_or_default::<f64>(ATTR_OPACITY) {
							*opacity *= parent_opacity;
						}
					}
					if parent_has_fill {
						for fill in graphic.iter_attribute_values_mut_or_default::<f64>(ATTR_OPACITY_FILL) {
							*fill *= parent_fill;
						}
					}
					if let Some(appearance) = &parent_appearance {
						for value in graphic.iter_attribute_values_mut_or_default::<Appearance>(ATTR_APPEARANCE) {
							if value.is_empty() {
								*value = appearance.clone();
							}
						}
					}

					// Unioning each group lets it enter the outer operation as one region, which is why this cannot defer
					// to `flatten_graphic_list`. Splicing children in as sibling operands makes them subtract from each other.
					let flattened = flatten_vector(&graphic)?;
					if flattened.is_empty() {
						// The union call emits one blank operand even from an empty list
						Vec::new()
					} else {
						boolean_operation_on_vector_list(&flattened, BooleanOperation::Union)?.into_iter().collect::<Vec<_>>()
					}
				}
				// Rasters, colors, and gradients bound no region, so they contribute no operand
				Graphic::None(_) | Graphic::NoneList(_) | Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Color(_) | Graphic::Gradient(_) => Vec::new(),
				Graphic::RasterCPUList(_) | Graphic::RasterGPUList(_) | Graphic::ColorList(_) | Graphic::GradientList(_) => Vec::new(),
				// Strokes have no vector outline representation; a brush node renders them to rasters
				Graphic::StrokeList(_) => Vec::new(),
				// Normalized to GraphicList above
				Graphic::Graphic(_) => Vec::new(),
			})
		})
		.collect::<Result<Vec<_>, BooleanOperationError>>()?;

	Ok(operands.into_iter().flatten().collect())
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

/// Every operand and result region is treated as closed, so an open path gets its closing segment here.
fn closed(mut path: BezPath) -> BezPath {
	if !path.elements().is_empty() && path.elements().last() != Some(&PathEl::ClosePath) {
		path.close_path();
	}
	path
}

fn to_bez_path(vector: &Vector, element_paths: &List<Path>, transform: DAffine2) -> Result<BezPath, BooleanOperationError> {
	let transform = Affine::new(transform.to_cols_array());
	let mut path = BezPath::new();

	for subpath in vector.stroke_bezpath_iter(element_paths) {
		let mut first = true;

		for segment in closed(subpath).segments() {
			let quantized = quantize_segment(transform * segment);
			if first {
				first = false;
				path.move_to(quantized.start());
			}
			path.push(quantized.as_path_el());
		}

		if !first {
			path.close_path();
		}
	}

	if !path.is_finite() {
		return Err(BooleanOperationError::NonFiniteCoordinates);
	}
	Ok(path)
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

#[cfg(test)]
mod tests {
	use super::*;
	use vector_types::kurbo::{Circle, Rect, Shape};

	fn operand(shape: &impl Shape) -> Item<Graphic> {
		Item::new_from_element(Graphic::Vector(Box::new(Vector::item_from_bezpath(&shape.to_path(0.01)))))
	}

	#[test]
	fn difference_leaves_the_overlap_of_two_shapes_empty() {
		let content: List<Graphic> = [operand(&Circle::new((0., 0.), 100.)), operand(&Rect::new(50., 50., 200., 200.))].into_iter().collect();
		let result = boolean_operation((), content, Item::new_from_element(BooleanOperation::Difference)).unwrap();

		let paths = result.attribute::<List<Path>>(ATTR_PATHS).cloned().unwrap_or_default();
		let vector = result.element();
		assert!(!vector.is_branching(&paths), "the two contours meeting where the outlines cross shouldn't be welded into a mesh");

		let outline: BezPath = vector.stroke_bezpath_iter(&paths).flat_map(|subpath| subpath.into_iter()).collect();
		assert_eq!(outline.winding(Point::new(60., 60.)), 0, "the overlap should be empty");
		assert_ne!(outline.winding(Point::new(-50., 0.)), 0, "the circle's own area should be filled");
		assert_ne!(outline.winding(Point::new(150., 150.)), 0, "the square's own area should be filled");
	}

	#[test]
	fn difference_keeps_areas_covered_by_an_odd_number_of_shapes() {
		let circles = [(0., 0.), (60., 0.), (30., 52.)].map(|center| operand(&Circle::new(center, 50.)));
		let result = boolean_operation((), circles.into_iter().collect(), Item::new_from_element(BooleanOperation::Difference)).unwrap();

		let paths = result.attribute::<List<Path>>(ATTR_PATHS).cloned().unwrap_or_default();
		let vector = result.element();
		assert!(!vector.is_branching(&paths), "the regions meeting where the outlines cross shouldn't be welded into a mesh");

		let outline: BezPath = vector.stroke_bezpath_iter(&paths).flat_map(|subpath| subpath.into_iter()).collect();
		assert_ne!(outline.winding(Point::new(-30., 0.)), 0, "the area inside one circle should be filled");
		assert_eq!(outline.winding(Point::new(30., -20.)), 0, "the area inside two circles should be empty");
		assert_ne!(outline.winding(Point::new(30., 17.)), 0, "the area inside all three circles should be filled");
	}
}
