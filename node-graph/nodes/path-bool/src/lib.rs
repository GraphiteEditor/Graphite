use core_types::attribute::{Attr, Opacity, OpacityFill, Transform as TransformAttr};
use core_types::list::{Item, List};
use core_types::node::Lane;
use core_types::{ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Ctx};
use glam::DAffine2;
use graphic_types::appearance::Appearance;
use graphic_types::graphic::{GraphicLevel, PaintColumns, PaintReach, bake_paint_transforms};
use graphic_types::markers::{Appearance as AppearanceMarker, EditorMergedLayers};
use graphic_types::vector_types::vector::VectorExt;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphic_types::{Graphic, IntoGraphicList, Vector};
use linesweeper::topology::Topology;
use linesweeper::{BinaryOp, FillRule, binary_op};
use smallvec::SmallVec;
use vector_types::kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, PathEl, PathSeg, Point, QuadBez};
pub use vector_types::vector::misc::BooleanOperation;

// TODO: Fix boolean ops to work by removing .transform() and .one_instance_*() calls,
// TODO: since before we used a Vec of single-item `List`s and now we use a single `List`
// TODO: with multiple items while still assuming a single item for the boolean operations.

#[allow(clippy::type_complexity)]
fn boolean_core<'e>(
	arena: &'e core_types::arena::Arena,
	flattened: List<Vector>,
	snapshot: List<Graphic<'static>>,
	operation: BooleanOperation,
) -> Result<(Vector, Attr<'e, TransformAttr>, Attr<'e, AppearanceMarker>, Attr<'e, EditorMergedLayers>), core_types::gpoll::Interrupt> {
	// The first index is the bottom of the stack
	let mut result_vector_list = boolean_operation_on_vector_list(&flattened, operation);

	// Replace the transformation matrix with a mutation of the vector points themselves
	if result_vector_list.element_mut(0).is_some() {
		let transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		result_vector_list.set_attribute(ATTR_TRANSFORM, 0, DAffine2::IDENTITY);

		let result_vector = result_vector_list.element_mut(0).unwrap();
		Vector::transform(result_vector, transform);

		// Clean up the boolean operation result by merging duplicated points
		let merge_transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		result_vector_list.element_mut(0).unwrap().merge_by_distance_spatial(merge_transform, 0.0001);
	}

	let exhausted = || {
		core_types::gpoll::Interrupt::from(core_types::gpoll::GraphError {
			kind: core_types::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	let element = result_vector_list.element(0).cloned().unwrap_or_default();
	use core_types::lane::LaneSource;
	let appearance = match result_vector_list.attr::<AppearanceMarker>(0).cloned() {
		Some(appearance) => Some(arena.alloc_sized_keyed(appearance, 0).ok_or_else(exhausted)?.0),
		None => None,
	};
	// Snapshot the input layers so the renderer can recurse into them for
	// editor click-target preservation.
	let merged_layers = arena.alloc_sized_keyed(snapshot, 0).ok_or_else(exhausted)?.0;

	// Blending, clipping and the layer path are the merge's carried columns: the
	// caller gathers them from the lane it names, so they are not re-read here.
	Ok((
		element,
		Attr(result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0)),
		Attr(appearance),
		Attr(Some(merged_layers)),
	))
}

/// Combines the geometric forms of one or more closed paths into a new vector path that results from cutting or joining the paths by the chosen method.
#[node_macro::node(category("Vector: Modifier"), memoize)]
fn boolean_operation<'e>(
	ctx: impl Ctx + ExtractArena<'e> + core_types::InjectIndex + Copy,
	/// The input of vector paths to perform the boolean operation on. Nested groups are automatically flattened.
	content: IList<Graphic<'static>>,
	/// Which boolean operation to perform on the paths.
	///
	/// Union combines all paths while cutting out overlapping areas (even the interiors of a single path).
	/// Subtraction cuts overlapping areas out from the last (Subtract Front) or first (Subtract Back) path.
	/// Intersection cuts away all but the overlapping areas shared by every path.
	/// Difference cuts away the overlapping areas shared by every path, leaving only the non-overlapping areas.
	operation: BooleanOperation,
) -> Result<(Lane<Vector>, Attr<'e, TransformAttr>, Attr<'e, AppearanceMarker>, Attr<'e, EditorMergedLayers>), core_types::gpoll::Interrupt> {
	if content.is_empty() {
		return Err(core_types::gpoll::GraphError::past_end().into());
	}
	let item = content.as_group_item();
	let flattened = flatten_vector_run(GraphicLevel::Run(&item), Ancestors::NONE, PaintReach::NONE);
	let snapshot = graphic_types::graphic::run_to_list::<Graphic>(&item).expect("the run holds the row's element type").into_graphic_list();
	let (element, transform, appearance, merged) = boolean_core(ctx.arena(), flattened, snapshot, operation)?;
	// The merge presents the bottom-of-stack lane's blending, clipping and layer
	// path, carried rather than re-read: one named lane instead of four copies.
	Ok((content.lane(0).map_element(element), transform, appearance, merged))
}

/// The boolean operation over a plain vector level, as [`boolean_operation`].
#[node_macro::node(category(""))]
fn boolean_operation_vector<'e>(
	ctx: impl Ctx + ExtractArena<'e> + core_types::InjectIndex + Copy,
	content: IList<Vector>,
	operation: BooleanOperation,
) -> Result<(Lane<Vector>, Attr<'e, TransformAttr>, Attr<'e, AppearanceMarker>, Attr<'e, EditorMergedLayers>), core_types::gpoll::Interrupt> {
	if content.is_empty() {
		return Err(core_types::gpoll::GraphError::past_end().into());
	}
	let item = content.as_group_item();
	let flattened = graphic_types::graphic::run_to_list::<Vector>(&item).expect("the run holds vector lanes");
	let snapshot = graphic_types::graphic::run_to_list::<Vector>(&item).expect("the run holds the row's element type").into_graphic_list();
	let (element, transform, appearance, merged) = boolean_core(ctx.arena(), flattened, snapshot, operation)?;
	Ok((content.lane(0).map_element(element), transform, appearance, merged))
}

pub use _boolean_operation_vector_mod::boolean_operation_vector_entries;

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
		let copy_from_transform: DAffine2 = vector.attribute_cloned_or_default(ATTR_TRANSFORM, index);
		// The boolean op bakes input transforms into the output geometry, so the result item carries no transform of its own
		attributes.insert(ATTR_TRANSFORM, DAffine2::IDENTITY);

		bake_paint_transforms(&mut attributes, copy_from_transform);

		Item::from_parts(Vector::default(), attributes)
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
	for contour in contours.contours() {
		row.element_mut().append_bezpath(closed(contour.path.clone()));
	}

	list.push(row);
	list
}

/// A text lane's rows: the shaped glyph vectors under the composed transform.
fn text_rows(text: &List<String>, parent_transform: DAffine2) -> Vec<Item<Vector>> {
	text_nodes::shape_text_list(text, false)
		.into_iter()
		.map(|mut sub_vector| {
			let current_transform: DAffine2 = sub_vector.attribute_cloned_or_default(ATTR_TRANSFORM);
			*sub_vector.attribute_mut_or_insert_default(ATTR_TRANSFORM) = parent_transform * current_transform;
			sub_vector
		})
		.collect()
}

fn push_rows(out: &mut List<Vector>, rows: Vec<Item<Vector>>) {
	for row in rows {
		out.push(row);
	}
}

/// The ancestors' composable attributes for a lane's contents. Whether an ancestor
/// carries an attribute is structural, so an absent one never invents a column below.
#[derive(Clone, Copy)]
struct Ancestors {
	transform: DAffine2,
	has_transform: bool,
	opacity: f64,
	has_opacity: bool,
	opacity_fill: f64,
	has_opacity_fill: bool,
}

impl Ancestors {
	const NONE: Self = Self {
		transform: DAffine2::IDENTITY,
		has_transform: false,
		opacity: 1.,
		has_opacity: false,
		opacity_fill: 1.,
		has_opacity_fill: false,
	};

	/// The composition a lane's contents inherit: this one with the lane's own attributes folded in.
	fn through<S: core_types::lane::LaneSource>(self, level: &S, index: usize) -> Self {
		let transform = level.try_attr::<TransformAttr>(index);
		let opacity = level.try_attr::<Opacity>(index);
		let opacity_fill = level.try_attr::<OpacityFill>(index);
		Self {
			transform: self.transform * transform.unwrap_or(DAffine2::IDENTITY),
			has_transform: self.has_transform || transform.is_some(),
			opacity: self.opacity * opacity.unwrap_or(1.),
			has_opacity: self.has_opacity || opacity.is_some(),
			opacity_fill: self.opacity_fill * opacity_fill.unwrap_or(1.),
			has_opacity_fill: self.has_opacity_fill || opacity_fill.is_some(),
		}
	}

	/// Composes onto one flattened row, as the legacy `compose_parent` did per item.
	fn compose(self, out: &mut List<Vector>, index: usize) {
		if self.has_transform || out.attribute::<DAffine2>(ATTR_TRANSFORM, index).is_some() {
			let own: DAffine2 = out.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			out.set_attribute(ATTR_TRANSFORM, index, self.transform * own);
		}
		if self.has_opacity || out.attribute::<f64>(ATTR_OPACITY, index).is_some() {
			let own: f64 = out.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			out.set_attribute(ATTR_OPACITY, index, self.opacity * own);
		}
		if self.has_opacity_fill || out.attribute::<f64>(ATTR_OPACITY_FILL, index).is_some() {
			let own: f64 = out.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			out.set_attribute(ATTR_OPACITY_FILL, index, self.opacity_fill * own);
		}
	}
}

/// A de-tabled vector leaf as one row: the lane's attributes with the reach
/// paint and the ancestor composition applied.
fn push_leaf_vector_row(out: &mut List<Vector>, level: GraphicLevel<'_>, index: usize, vector: &Vector, ancestors: Ancestors, reach: PaintReach<'_>) {
	let out_index = out.len();
	out.push(Item::from_parts(vector.clone(), graphic_types::graphic::lane_attributes(level, index)));
	stamp_inherited_appearance(out, out_index, reach.appearance);
	ancestors.compose(out, out_index);
}

fn push_vector_rows(out: &mut List<Vector>, rows: &List<Vector>, composed: Ancestors, reach: PaintReach<'_>) {
	for row in 0..rows.len() {
		let Some(item) = rows.clone_item(row) else { continue };
		let index = out.len();
		out.push(item);
		stamp_inherited_appearance(out, index, reach.appearance);
		composed.compose(out, index);
	}
}

/// The shaped glyph rows under the composition, which the shaping itself does not apply.
fn push_text_rows(out: &mut List<Vector>, text: &List<String>, composed: Ancestors) {
	let start = out.len();
	push_rows(out, text_rows(text, DAffine2::IDENTITY));
	for row in start..out.len() {
		composed.compose(out, row);
	}
}

/// The cascade's resolved appearance lands on a row whose own is undeclared, since a declared row wins wholesale.
fn stamp_inherited_appearance(out: &mut List<Vector>, index: usize, inherited: Option<&Appearance>) {
	if let Some(appearance) = inherited
		&& out.attribute::<Appearance>(graphic_types::ATTR_APPEARANCE, index).and_then(Appearance::declared).is_none()
	{
		out.set_attribute(graphic_types::ATTR_APPEARANCE, index, appearance.clone());
	}
}

fn push_union(out: &mut List<Vector>, flattened: List<Vector>) {
	// The union emits one blank operand even from an empty list, which would fabricate a region out of nothing
	if flattened.is_empty() {
		return;
	}
	for row in boolean_operation_on_vector_list(&flattened, BooleanOperation::Union).into_iter() {
		out.push(row);
	}
}

/// The native flatten over a graphic level: the legacy flatten's arms over
/// either level storage, with lane paint threaded by [`PaintReach`], leaf
/// attributes read from their lanes, and native group runs walked directly.
fn flatten_vector_run(level: GraphicLevel<'_>, ancestors: Ancestors, inherited: PaintReach<'_>) -> List<Vector> {
	let mut out = List::new();
	flatten_vector_run_into(&mut out, level, ancestors, inherited);
	out
}

fn flatten_vector_run_into<'a>(out: &mut List<Vector>, level: GraphicLevel<'a>, ancestors: Ancestors, inherited: PaintReach<'a>) {
	use core_types::lane::LaneSource;
	let columns = PaintColumns::new(&level);
	for index in 0..level.lane_count() {
		let Some(element) = level.element(index) else { continue };
		let reach = inherited.for_lane(&columns, index);
		let composed = ancestors.through(&level, index);
		match element {
			Graphic::Vector(vector) => push_leaf_vector_row(out, level, index, vector, ancestors, reach),
			Graphic::GraphicList(children) => push_union(out, flatten_vector_run(GraphicLevel::Legacy(children), composed, reach)),
			Graphic::Group(group) => flatten_group(out, group, composed, reach),
			Graphic::Text(text) => {
				let one = List::new_from_item(Item::from_parts(text.clone(), graphic_types::graphic::lane_attributes(level, index)));
				push_text_rows(out, &one, composed);
			}
			// Rasters, colors, and gradients bound no region, so they contribute no operand
			Graphic::None | Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Color(_) | Graphic::Gradient(_) => continue,
		}
	}
}

/// A group flattens as its legacy lowering did: a vector run serves its rows and
/// a graphic run unions like a nested list. Runs of non-path content bound no
/// region, so they serve no rows at all.
fn flatten_group(out: &mut List<Vector>, group: &core_types::record::Group, composed: Ancestors, reach: PaintReach<'_>) {
	let item = &group.content;
	if let Some(rows) = graphic_types::graphic::run_to_list::<Vector>(item) {
		push_vector_rows(out, &rows, composed, reach);
	} else if core_types::record::RunView::<Graphic>::new(item).is_some() {
		push_union(out, flatten_vector_run(GraphicLevel::Run(item), composed, reach));
	} else if let Some(text) = graphic_types::graphic::run_to_list::<String>(item) {
		push_text_rows(out, &text, composed);
	}
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

fn to_bez_path(vector: &Vector, transform: DAffine2) -> BezPath {
	let transform = Affine::new(transform.to_cols_array());
	let mut path = BezPath::new();

	for subpath in vector.stroke_bezpath_iter() {
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

	path
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
	use core_types::Color;
	use core_types::record::Group;
	use glam::DVec2;
	use graphic_types::vector_types::kurbo::Shape;

	fn square(corner: DVec2) -> Vector {
		Vector::from_bezpath(graphic_types::vector_types::kurbo::Rect::new(corner.x, corner.y, corner.x + 1., corner.y + 1.).to_path(graphic_types::vector_types::kurbo::DEFAULT_ACCURACY))
	}

	fn black_paint() -> List<Graphic<'static>> {
		List::new_from_element(Graphic::Color(Color::BLACK))
	}

	/// The single-fill appearance a built row paints with.
	fn fill_appearance(paint: List<Graphic<'static>>) -> Appearance {
		use graphic_types::appearance::Coverage;
		Appearance::new_single(Coverage::new_fill(), Graphic::GraphicList(paint))
	}

	#[test]
	fn the_native_flatten_reads_lanes_groups_and_reach() {
		let inner_vector = square(DVec2::ZERO);
		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let mut builder = core_types::record::RunBuilder::new(&arena, core_types::record::element_write_hashed::<Vector>(), &[], 1).unwrap();
		builder.push(inner_vector.clone()).unwrap();
		let inner_item = builder.finish();

		let mut top = List::new();
		top.push(Item::new_from_element(Graphic::Vector(square(DVec2::ZERO))));
		top.push(Item::new_from_element(Graphic::Color(Color::BLACK)));
		top.push(Item::new_from_element(Graphic::Group(Group { row: None, content: inner_item })));
		top.set_attribute(ATTR_TRANSFORM, 0, DAffine2::from_translation(DVec2::new(5., 5.)));
		top.set_attribute(graphic_types::ATTR_APPEARANCE, 0, fill_appearance(black_paint()));
		top.set_attribute(ATTR_OPACITY, 1, 0.5);
		top.set_attribute(ATTR_TRANSFORM, 2, DAffine2::from_scale(DVec2::splat(3.)));

		let rows = flatten_vector_run(GraphicLevel::Legacy(&top), Ancestors::NONE, PaintReach::NONE);
		// The color lane bounds no region, so it serves no operand and the group's row lands at 1
		assert_eq!(rows.len(), 2);

		let fill_of = |index: usize| {
			rows.attribute::<Appearance>(graphic_types::ATTR_APPEARANCE, index)
				.and_then(|appearance| appearance.first_paint_of(graphic_types::appearance::Cover::Fill))
				.and_then(graphic_types::graphic::paint_cell_rows)
		};

		// Lane 0: the leaf row keeps its lane attributes, with the lane fill
		// present and the ancestor composition the identity.
		assert_eq!(rows.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, 0), DAffine2::from_translation(DVec2::new(5., 5.)));
		assert!(fill_of(0).is_some());

		// Lane 1: the group's vector run serves its row under the lane
		// transform.
		assert_eq!(rows.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, 1), DAffine2::from_scale(DVec2::splat(3.)));
		assert_eq!(rows.element(1).unwrap(), &inner_vector);
	}
}
