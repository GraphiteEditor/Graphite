use core_types::attribute::{Attr, BlendMode as BlendModeAttr, ClippingMask, EditorLayerPath, Opacity, OpacityFill, Transform as TransformAttr};
use core_types::list::{Item, List};
use core_types::uuid::NodeId;
use core_types::{ATTR_BLEND_MODE, ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, BlendMode, Color, Ctx};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{PaintColumns, PaintReach, bake_paint_transforms, set_paint_attribute, set_paint_attribute_at};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::vector_types::GradientStops;
use graphic_types::markers::{EditorMergedLayers, Fill, Stroke};
use graphic_types::vector_types::gradient::{GradientSpreadMethod, GradientType};
use graphic_types::vector_types::subpath::{ManipulatorGroup, Subpath};
use graphic_types::vector_types::vector::PointId;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphic_types::vector_types::{ATTR_GRADIENT_TYPE, ATTR_SPREAD_METHOD};
use graphic_types::{ATTR_FILL, ATTR_STROKE, Graphic, IntoGraphicList, Vector};
use linesweeper::topology::Topology;
use linesweeper::{BinaryOp, FillRule, binary_op};
use smallvec::SmallVec;
use vector_types::kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, PathSeg, Point, QuadBez};
pub use vector_types::vector::misc::BooleanOperation;

// TODO: Fix boolean ops to work by removing .transform() and .one_instance_*() calls,
// TODO: since before we used a Vec of single-item `List`s and now we use a single `List`
// TODO: with multiple items while still assuming a single item for the boolean operations.

#[allow(clippy::type_complexity)]
fn boolean_core<'e>(
	arena: &'e core_types::arena::Arena,
	flattened: List<Vector>,
	snapshot: List<Graphic>,
	operation: BooleanOperation,
) -> Result<
	(
		Vector,
		Attr<'e, TransformAttr>,
		Attr<'e, Fill>,
		Attr<'e, Stroke>,
		Attr<'e, BlendModeAttr>,
		Attr<'e, Opacity>,
		Attr<'e, OpacityFill>,
		Attr<'e, ClippingMask>,
		Attr<'e, EditorLayerPath>,
		Attr<'e, EditorMergedLayers>,
	),
	core_types::gpoll::Interrupt,
> {
	// The first index is the bottom of the stack
	let mut result_vector_list = boolean_operation_on_vector_list(&flattened, operation);

	// Replace the transformation matrix with a mutation of the vector points themselves
	if result_vector_list.element_mut(0).is_some() {
		let transform: DAffine2 = result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		result_vector_list.set_attribute(ATTR_TRANSFORM, 0, DAffine2::IDENTITY);

		let result_vector = result_vector_list.element_mut(0).unwrap();
		Vector::transform(result_vector, transform);
		result_vector.set_stroke_transform(DAffine2::IDENTITY);

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
	let park_paint = |paint: Option<List<Graphic>>| -> Result<Option<&'e List<Graphic>>, core_types::gpoll::Interrupt> {
		match paint {
			Some(list) => Ok(Some(arena.alloc(list).ok_or_else(exhausted)?.0)),
			None => Ok(None),
		}
	};

	let element = result_vector_list.element(0).cloned().unwrap_or_default();
	let fill = park_paint(graphic_types::graphic::paint_graphics::<Fill, _>(&result_vector_list, 0).cloned())?;
	let stroke = park_paint(graphic_types::graphic::paint_graphics::<Stroke, _>(&result_vector_list, 0).cloned())?;
	let layer_path: Vec<NodeId> = result_vector_list.attribute::<Vec<NodeId>>(ATTR_EDITOR_LAYER_PATH, 0).map(|path| path.clone()).unwrap_or_default();
	let layer_path = arena.alloc(layer_path).ok_or_else(exhausted)?.0;
	// Snapshot the input layers so the renderer can recurse into them for
	// editor click-target preservation.
	let merged_layers = arena.alloc(snapshot).ok_or_else(exhausted)?.0;

	Ok((
		element,
		Attr(result_vector_list.attribute_cloned_or_default(ATTR_TRANSFORM, 0)),
		Attr(fill),
		Attr(stroke),
		Attr(result_vector_list.attribute_cloned_or_default(ATTR_BLEND_MODE, 0)),
		Attr(result_vector_list.attribute_cloned_or(ATTR_OPACITY, 0, 1.)),
		Attr(result_vector_list.attribute_cloned_or(ATTR_OPACITY_FILL, 0, 1.)),
		Attr(result_vector_list.attribute_cloned_or_default(ATTR_CLIPPING_MASK, 0)),
		Attr(layer_path.as_slice()),
		Attr(Some(merged_layers)),
	))
}

/// Combines the geometric forms of one or more closed paths into a new vector path that results from cutting or joining the paths by the chosen method.
#[node_macro::node(category("Vector: Modifier"), memoize)]
fn boolean_operation<'e>(
	ctx: impl Ctx + ExtractArena<'e> + core_types::InjectIndex + Copy,
	/// The wire of vector paths to perform the boolean operation on. Nested groups are automatically flattened.
	content: IList<Graphic>,
	/// Which boolean operation to perform on the paths.
	///
	/// Union combines all paths while cutting out overlapping areas (even the interiors of a single path).
	/// Subtraction cuts overlapping areas out from the last (Subtract Front) or first (Subtract Back) path.
	/// Intersection cuts away all but the overlapping areas shared by every path.
	/// Difference cuts away the overlapping areas shared by every path, leaving only the non-overlapping areas.
	operation: BooleanOperation,
) -> Result<
	(
		Vector,
		Attr<'e, TransformAttr>,
		Attr<'e, Fill>,
		Attr<'e, Stroke>,
		Attr<'e, BlendModeAttr>,
		Attr<'e, Opacity>,
		Attr<'e, OpacityFill>,
		Attr<'e, ClippingMask>,
		Attr<'e, EditorLayerPath>,
		Attr<'e, EditorMergedLayers>,
	),
	core_types::gpoll::Interrupt,
> {
	// SAFETY: a materialized input's frames are arena-resident.
	let item = unsafe { core_types::record::GroupItem::from_resident(content.batch()) };
	let run = core_types::record::RunView::<Graphic>::new(&item).expect("the run holds graphic lanes");
	let flattened = flatten_vector_run(&run, DAffine2::IDENTITY, PaintReach::NONE);
	let snapshot = graphic_types::graphic::run_to_render_list::<Graphic>(&item)
		.expect("the run holds the row's element type")
		.into_graphic_list();
	boolean_core(ctx.arena(), flattened, snapshot, operation)
}

/// The boolean operation over a plain vector level, as [`boolean_operation`].
#[node_macro::node(category(""))]
fn boolean_operation_vector<'e>(
	ctx: impl Ctx + ExtractArena<'e> + core_types::InjectIndex + Copy,
	content: IList<Vector>,
	operation: BooleanOperation,
) -> Result<
	(
		Vector,
		Attr<'e, TransformAttr>,
		Attr<'e, Fill>,
		Attr<'e, Stroke>,
		Attr<'e, BlendModeAttr>,
		Attr<'e, Opacity>,
		Attr<'e, OpacityFill>,
		Attr<'e, ClippingMask>,
		Attr<'e, EditorLayerPath>,
		Attr<'e, EditorMergedLayers>,
	),
	core_types::gpoll::Interrupt,
> {
	// SAFETY: a materialized input's frames are arena-resident.
	let item = unsafe { core_types::record::GroupItem::from_resident(content.batch()) };
	let flattened = graphic_types::graphic::run_to_list::<Vector>(&item).expect("the run holds vector lanes");
	let snapshot = graphic_types::graphic::run_to_render_list::<Vector>(&item)
		.expect("the run holds the row's element type")
		.into_graphic_list();
	boolean_core(ctx.arena(), flattened, snapshot, operation)
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
	for subpath in from_bez_paths(contours.contours().map(|c| &c.path)) {
		row.element_mut().append_subpath(subpath, false);
	}

	list.push(row);
	list
}

/// A raster stand-in row: the image's unit rectangle under its transform,
/// black-filled, keeping the layer routing and blending attributes.
fn raster_stand_in_rows<T>(image: &List<T>, parent_transform: DAffine2) -> Vec<Item<Vector>> {
	let make_item = |transform, layer, blend_mode: BlendMode, opacity: f64, fill: f64, clip: bool| {
		let mut subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
		subpath.apply_transform(transform);

		let element = Vector::from_subpath(subpath);

		let mut item = Item::new_from_element(element)
			.with_attribute(ATTR_BLEND_MODE, blend_mode)
			.with_attribute(ATTR_OPACITY, opacity)
			.with_attribute(ATTR_OPACITY_FILL, fill)
			.with_attribute(ATTR_CLIPPING_MASK, clip)
			.with_attribute(ATTR_EDITOR_LAYER_PATH, layer);
		set_paint_attribute(item.attributes_mut(), ATTR_FILL, List::new_from_element(Color::BLACK));
		item
	};

	(0..image.len())
		.map(|i| {
			let row_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM, i);
			let layer: Vec<NodeId> = image.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, i);
			let blend_mode: BlendMode = image.attribute_cloned_or_default(ATTR_BLEND_MODE, i);
			let opacity: f64 = image.attribute_cloned_or(ATTR_OPACITY, i, 1.);
			let fill: f64 = image.attribute_cloned_or(ATTR_OPACITY_FILL, i, 1.);
			let clip: bool = image.attribute_cloned_or_default(ATTR_CLIPPING_MASK, i);
			make_item(parent_transform * row_transform, layer, blend_mode, opacity, fill, clip)
		})
		.collect()
}

/// A color row: an empty vector carrying the color as its fill paint.
fn color_paint_rows(color: &List<Color>) -> Vec<Item<Vector>> {
	color
		.clone()
		.into_iter()
		.map(|row| {
			let (color, mut attributes) = row.into_parts();
			set_paint_attribute(&mut attributes, ATTR_FILL, List::new_from_element(color));

			let mut element = Vector::default();
			element.set_stroke_transform(DAffine2::IDENTITY);

			Item::from_parts(element, attributes)
		})
		.collect()
}

/// A gradient row: an empty vector carrying the stops as its fill paint, the
/// gradient keys moved onto the paint.
fn gradient_paint_rows(gradient: &List<GradientStops>) -> Vec<Item<Vector>> {
	gradient
		.clone()
		.into_iter()
		.map(|row| {
			let (stops, mut attributes) = row.into_parts();

			let mut gradient_paint = List::new_from_element(stops);
			if let Some(transform) = attributes.remove::<DAffine2>(ATTR_TRANSFORM) {
				gradient_paint.set_attribute(ATTR_TRANSFORM, 0, transform);
			}
			if let Some(gradient_type) = attributes.remove::<GradientType>(ATTR_GRADIENT_TYPE) {
				gradient_paint.set_attribute(ATTR_GRADIENT_TYPE, 0, gradient_type);
			}
			if let Some(spread_method) = attributes.remove::<GradientSpreadMethod>(ATTR_SPREAD_METHOD) {
				gradient_paint.set_attribute(ATTR_SPREAD_METHOD, 0, spread_method);
			}
			set_paint_attribute(&mut attributes, ATTR_FILL, gradient_paint);

			let mut element = Vector::default();
			element.set_stroke_transform(DAffine2::IDENTITY);

			Item::from_parts(element, attributes)
		})
		.collect()
}

/// A text row: the shaped glyph vectors under the composed transform.
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

fn push_vector_rows(out: &mut List<Vector>, inner: &List<Vector>, composed: DAffine2, reach: PaintReach<'_>) {
	for row in 0..inner.len() {
		let Some(item) = inner.clone_item(row) else { continue };
		let index = out.len();
		out.push(item);
		if reach.applies() {
			for (key, slot) in [(ATTR_FILL, reach.paint.fill), (ATTR_STROKE, reach.paint.stroke)] {
				if let Some(paint) = slot {
					set_paint_attribute_at(out, index, key, paint.clone());
				}
			}
		}
		let current: DAffine2 = out.attribute_cloned_or_default(ATTR_TRANSFORM, index);
		out.set_attribute(ATTR_TRANSFORM, index, composed * current);
	}
}

fn push_union(out: &mut List<Vector>, flattened: List<Vector>) {
	for row in boolean_operation_on_vector_list(&flattened, BooleanOperation::Union).into_iter() {
		out.push(row);
	}
}

/// The native flatten over a graphic level: the legacy flatten's arms over a
/// lane source, with lane paint threaded by [`PaintReach`] in place of the
/// legacy pre-push, and native group runs walked directly.
fn flatten_vector_run<'a, S: core_types::lane::LaneSource<Element = Graphic>>(source: &'a S, transform: DAffine2, inherited: PaintReach<'a>) -> List<Vector> {
	let mut out = List::new();
	flatten_vector_run_into(&mut out, source, transform, inherited);
	out
}

fn flatten_vector_run_into<'a, S: core_types::lane::LaneSource<Element = Graphic>>(out: &mut List<Vector>, source: &'a S, transform: DAffine2, inherited: PaintReach<'a>) {
	let columns = PaintColumns::new(source);
	for index in 0..source.lane_count() {
		let Some(element) = source.element(index) else { continue };
		let reach = inherited.for_lane(&columns, index);
		let composed = transform * source.attr::<TransformAttr>(index);
		match element {
			Graphic::Vector(inner) => push_vector_rows(out, inner, composed, reach),
			Graphic::Graphic(children) => push_union(out, flatten_vector_run(children, composed, reach.nested())),
			Graphic::Group(group) => flatten_group(out, group, composed, reach),
			Graphic::RasterCPU(image) => push_rows(out, raster_stand_in_rows(image, composed)),
			Graphic::RasterGPU(image) => push_rows(out, raster_stand_in_rows(image, composed)),
			Graphic::Color(color) => push_rows(out, color_paint_rows(color)),
			Graphic::Gradient(gradient) => push_rows(out, gradient_paint_rows(gradient)),
			Graphic::Text(text) => push_rows(out, text_rows(text, composed)),
		}
	}
}

/// A group flattens as its legacy lowering did: a vector run serves its rows,
/// a graphic run unions like a nested list, and another typed run serves its
/// stand-in rows.
fn flatten_group(out: &mut List<Vector>, group: &core_types::record::Group, composed: DAffine2, reach: PaintReach<'_>) {
	let item = &group.content;
	if let Some(rows) = graphic_types::graphic::run_to_list::<Vector>(item) {
		push_vector_rows(out, &rows, composed, reach);
	} else if let Some(run) = core_types::record::RunView::<Graphic>::new(item) {
		push_union(out, flatten_vector_run(&run, composed, reach.into_group_graphics()));
	} else if let Some(image) = graphic_types::graphic::run_to_list::<Raster<CPU>>(item) {
		push_rows(out, raster_stand_in_rows(&image, composed));
	} else if let Some(image) = graphic_types::graphic::run_to_list::<Raster<GPU>>(item) {
		push_rows(out, raster_stand_in_rows(&image, composed));
	} else if let Some(color) = graphic_types::graphic::run_to_list::<Color>(item) {
		push_rows(out, color_paint_rows(&color));
	} else if let Some(gradient) = graphic_types::graphic::run_to_list::<GradientStops>(item) {
		push_rows(out, gradient_paint_rows(&gradient));
	} else if let Some(text) = graphic_types::graphic::run_to_list::<String>(item) {
		push_rows(out, text_rows(&text, composed));
	}
}

/// The legacy baseline the flatten law compares against.
#[cfg(test)]
fn flatten_vector(graphic_list: &List<Graphic>) -> List<Vector> {
	(0..graphic_list.len())
		.flat_map(|index| {
			let graphic = graphic_list.element(index).unwrap();
			match graphic.clone() {
				Graphic::Group(_) => Vec::new(),
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

						let element = Vector::from_subpath(subpath);

						let mut item = Item::new_from_element(element)
							.with_attribute(ATTR_BLEND_MODE, blend_mode)
							.with_attribute(ATTR_OPACITY, opacity)
							.with_attribute(ATTR_OPACITY_FILL, fill)
							.with_attribute(ATTR_CLIPPING_MASK, clip)
							.with_attribute(ATTR_EDITOR_LAYER_PATH, layer);
						set_paint_attribute(item.attributes_mut(), ATTR_FILL, List::new_from_element(Color::BLACK));
						item
					};

					// Apply the parent graphic's transform to each raster element, preserving each item's layer
					// and alpha_blending so the boolean op downstream can route clicks (and inherit blending state)
					// back to the originating raster layer
					(0..image.len())
						.map(|i| {
							let row_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM, i);
							let layer: Vec<NodeId> = image.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, i);
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

						let element = Vector::from_subpath(subpath);

						let mut item = Item::new_from_element(element)
							.with_attribute(ATTR_BLEND_MODE, blend_mode)
							.with_attribute(ATTR_OPACITY, opacity)
							.with_attribute(ATTR_OPACITY_FILL, fill)
							.with_attribute(ATTR_CLIPPING_MASK, clip)
							.with_attribute(ATTR_EDITOR_LAYER_PATH, layer);
						set_paint_attribute(item.attributes_mut(), ATTR_FILL, List::new_from_element(Color::BLACK));
						item
					};

					// Apply the parent graphic's transform to each raster element, preserving each item's layer
					// and alpha_blending so the boolean op downstream can route clicks (and inherit blending state)
					// back to the originating raster layer
					(0..image.len())
						.map(|i| {
							let row_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM, i);
							let layer: Vec<NodeId> = image.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, i);
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
						let (color, mut attributes) = row.into_parts();
						set_paint_attribute(&mut attributes, ATTR_FILL, List::new_from_element(color));

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
						if let Some(transform) = attributes.remove::<DAffine2>(ATTR_TRANSFORM) {
							gradient_paint.set_attribute(ATTR_TRANSFORM, 0, transform);
						}
						if let Some(gradient_type) = attributes.remove::<GradientType>(ATTR_GRADIENT_TYPE) {
							gradient_paint.set_attribute(ATTR_GRADIENT_TYPE, 0, gradient_type);
						}
						if let Some(spread_method) = attributes.remove::<GradientSpreadMethod>(ATTR_SPREAD_METHOD) {
							gradient_paint.set_attribute(ATTR_SPREAD_METHOD, 0, spread_method);
						}
						set_paint_attribute(&mut attributes, ATTR_FILL, gradient_paint);

						let mut element = Vector::default();
						element.set_stroke_transform(DAffine2::IDENTITY);

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

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::record::Group;

	fn square(corner: DVec2) -> Vector {
		Vector::from_subpath(Subpath::<PointId>::new_rectangle(corner, corner + DVec2::ONE))
	}

	fn black_paint() -> List<Graphic> {
		List::new_from_element(Graphic::Color(List::new_from_element(Color::BLACK)))
	}

	#[test]
	fn the_native_flatten_matches_the_legacy_flatten() {
		let inner_vector = square(DVec2::ZERO);
		let inner_layout = core_types::record::Layout::default().with_writes(0, core_types::record::element_write_hashed::<Vector>(), &[]);
		let mut inner_bytes = vec![0u8; inner_layout.lane_stride()];
		// SAFETY: `inner_bytes` is one lane of `inner_layout`; a parked element
		// stores its reference.
		unsafe { inner_bytes.as_mut_ptr().cast::<&Vector>().write(&inner_vector) };
		// SAFETY: `inner_bytes` holds one lane of `inner_layout` at its stride.
		let inner_item = unsafe { core_types::record::GroupItem::from_resident(core_types::node::RecordBatch::new(inner_bytes.as_ptr(), 1, &inner_layout)) };

		let mut painted = List::new();
		painted.push(Item::new_from_element(square(DVec2::ZERO)));
		painted.push(Item::new_from_element(square(DVec2::ONE)));
		painted.set_attribute(ATTR_TRANSFORM, 0, DAffine2::from_translation(DVec2::new(1., 0.)));
		set_paint_attribute_at(&mut painted, 1, ATTR_FILL, List::new_from_element(Graphic::Color(List::new_from_element(Color::WHITE))));

		let mut nested_child = List::new();
		nested_child.push(Item::new_from_element(square(DVec2::new(2., 2.))));
		let mut nested = List::new_from_element(Graphic::Vector(nested_child));
		nested.set_attribute(ATTR_TRANSFORM, 0, DAffine2::from_scale(DVec2::splat(2.)));

		let mut colors = List::new_from_element(Color::BLACK);
		colors.set_attribute(ATTR_OPACITY, 0, 0.5);

		let mut top = List::new();
		top.push(Item::new_from_element(Graphic::Vector(painted)));
		top.push(Item::new_from_element(Graphic::Graphic(nested)));
		top.push(Item::new_from_element(Graphic::Color(colors)));
		top.push(Item::new_from_element(Graphic::Group(Group { row: None, content: inner_item })));
		top.set_attribute(ATTR_TRANSFORM, 0, DAffine2::from_translation(DVec2::new(5., 5.)));
		set_paint_attribute_at(&mut top, 0, ATTR_FILL, black_paint());
		top.set_attribute(ATTR_TRANSFORM, 3, DAffine2::from_scale(DVec2::splat(3.)));

		let legacy = {
			let mut prepared = top.clone();
			// The legacy pre-push, written by hand: the painted lane's fill
			// lands on every interior item.
			if let Some(Graphic::Vector(inner)) = prepared.element_mut(0) {
				for index in 0..inner.len() {
					set_paint_attribute_at(inner, index, ATTR_FILL, black_paint());
				}
			}
			if let Some(element) = prepared.element_mut(3) {
				*element = graphic_types::graphic::map_groups_to_legacy(element);
			}
			flatten_vector(&prepared)
		};
		assert_eq!(flatten_vector_run(&top, DAffine2::IDENTITY, PaintReach::NONE), legacy);
	}
}
