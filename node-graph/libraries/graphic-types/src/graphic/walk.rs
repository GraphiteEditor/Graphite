use super::Graphic;
use super::paint::{LanePaint, PaintColumns, PaintReach, is_paint_present, paint_graphics, set_paint_attribute_at};
use crate::markers::{ATTR_FILL, ATTR_STROKE, Fill};
use core_types::attribute::{Attribute, ClippingMask, EditorLayerPath, Opacity, OpacityFill, Transform};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::lane::LaneSource;
use core_types::list::{AttributeValueDyn, Item, ItemAttributeValues, List};
use core_types::record::FieldOffset;
use core_types::render_complexity::RenderComplexity;
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DVec2};
use raster_types::{CPU, GPU, Raster};
use vector_types::{GradientStops, Vector};

/// One run's attribute tokens, minted once so the lane loops read at an offset.
struct RunAttrs {
	transform: Option<FieldOffset<Transform>>,
	opacity: Option<FieldOffset<Opacity>>,
	opacity_fill: Option<FieldOffset<OpacityFill>>,
	clipping_mask: Option<FieldOffset<ClippingMask>>,
}

impl RunAttrs {
	fn of(item: &core_types::record::GroupItem) -> Self {
		let layout = item.layout();
		Self {
			transform: FieldOffset::of(layout, 0),
			opacity: FieldOffset::of(layout, 0),
			opacity_fill: FieldOffset::of(layout, 0),
			clipping_mask: FieldOffset::of(layout, 0),
		}
	}

	fn read_or<'i, A: Attribute>(item: &'i core_types::record::GroupItem, field: Option<FieldOffset<A>>, lane: usize, default: A::Value<'i>) -> A::Value<'i> {
		match field.and_then(|field| item.lanes().get(lane).try_attr_at(field)) {
			Some(value) => value,
			None => default,
		}
	}
}

pub fn group_is_empty(group: &core_types::record::Group) -> bool {
	group.content.is_empty()
}

pub(in crate::graphic) fn group_all_clipped(group: &core_types::record::Group) -> bool {
	let item = &group.content;
	let attrs = RunAttrs::of(item);
	(0..item.len()).all(|lane| RunAttrs::read_or(item, attrs.clipping_mask, lane, false))
}

pub(in crate::graphic) fn group_is_opaque(group: &core_types::record::Group) -> bool {
	let item = &group.content;
	let attrs = RunAttrs::of(item);
	let lanes = item.typed_lanes::<Graphic>();
	!item.is_empty()
		&& (0..item.len()).all(|lane| {
			RunAttrs::read_or(item, attrs.opacity, lane, 1.) >= 1.
				&& RunAttrs::read_or(item, attrs.opacity_fill, lane, 1.) >= 1.
				&& lanes.as_ref().is_some_and(|lanes| lanes.element_ref(lane).is_opaque())
		})
}

pub(in crate::graphic) fn group_is_fully_transparent(group: &core_types::record::Group) -> bool {
	let item = &group.content;
	let attrs = RunAttrs::of(item);
	let lanes = item.typed_lanes::<Graphic>();
	(0..item.len()).all(|lane| RunAttrs::read_or(item, attrs.opacity, lane, 1.) <= 0. || lanes.as_ref().is_some_and(|lanes| lanes.element_ref(lane).is_fully_transparent()))
}

pub(in crate::graphic) fn group_bounding_box(group: &core_types::record::Group, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> RenderBoundingBox {
	fn combine(combined: &mut Option<[DVec2; 2]>, any_infinite: &mut bool, bounds: RenderBoundingBox, thumbnail: bool) -> Option<RenderBoundingBox> {
		match bounds {
			RenderBoundingBox::None => None,
			RenderBoundingBox::Infinite if thumbnail => {
				*any_infinite = true;
				None
			}
			RenderBoundingBox::Infinite => Some(RenderBoundingBox::Infinite),
			RenderBoundingBox::Rectangle(bounds) => {
				*combined = Some(match *combined {
					Some(existing) => core_types::math::quad::Quad::combine_bounds(existing, bounds),
					None => bounds,
				});
				None
			}
		}
	}
	fn typed_run<T: dyn_any::StaticTypeSized + BoundingBox>(item: &core_types::record::GroupItem, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> Option<RenderBoundingBox> {
		let lanes = item.typed_lanes::<T>()?;
		let transform_offset = RunAttrs::of(item).transform;
		let mut combined = None;
		let mut any_infinite = false;
		for lane in 0..lanes.len() {
			let lane_transform = transform * RunAttrs::read_or(item, transform_offset, lane, DAffine2::IDENTITY);
			let element = lanes.element_ref(lane);
			let bounds = match thumbnail {
				true => element.thumbnail_bounding_box(lane_transform, include_stroke),
				false => element.bounding_box(lane_transform, include_stroke),
			};
			if let Some(short_circuit) = combine(&mut combined, &mut any_infinite, bounds, thumbnail) {
				return Some(short_circuit);
			}
		}
		Some(match (combined, any_infinite) {
			(Some(bounds), _) => RenderBoundingBox::Rectangle(bounds),
			(None, true) => RenderBoundingBox::Infinite,
			(None, false) => RenderBoundingBox::None,
		})
	}
	fn run_bounding_box(item: &core_types::record::GroupItem, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> RenderBoundingBox {
		None.or_else(|| typed_run::<Graphic>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Vector>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Raster<CPU>>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Raster<GPU>>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Color>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<GradientStops>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<String>(item, transform, include_stroke, thumbnail))
			.unwrap_or(RenderBoundingBox::Infinite)
	}
	run_bounding_box(&group.content, transform, include_stroke, thumbnail)
}

/// One step of the vector-row walk: continue to the next row or stop early.
pub enum RowStep {
	Continue,
	Stop,
}

/// The ancestor composition a flattened row inherits: transform, opacity and
/// fill opacity multiply down, each composing only where some ancestor
/// carries the attribute, matching the legacy flatten.
#[derive(Clone, Copy)]
struct FlattenScale {
	has_transform: bool,
	transform: DAffine2,
	has_opacity: bool,
	opacity: f64,
	has_fill_opacity: bool,
	fill_opacity: f64,
}

impl FlattenScale {
	const ROOT: Self = Self {
		has_transform: false,
		transform: DAffine2::IDENTITY,
		has_opacity: false,
		opacity: 1.,
		has_fill_opacity: false,
		fill_opacity: 1.,
	};

	fn composed<S: LaneSource>(self, source: &S, lane: usize) -> Self {
		let transform = source.try_attr::<Transform>(lane);
		let opacity = source.try_attr::<Opacity>(lane);
		let fill_opacity = source.try_attr::<OpacityFill>(lane);
		Self {
			has_transform: self.has_transform || transform.is_some(),
			transform: self.transform * transform.unwrap_or(DAffine2::IDENTITY),
			has_opacity: self.has_opacity || opacity.is_some(),
			opacity: self.opacity * opacity.unwrap_or(1.),
			has_fill_opacity: self.has_fill_opacity || fill_opacity.is_some(),
			fill_opacity: self.fill_opacity * fill_opacity.unwrap_or(1.),
		}
	}
}

/// A graphic level in either of its two storages, as one lane source.
#[derive(Clone, Copy)]
pub enum GraphicLevel<'a> {
	Legacy(&'a List<Graphic<'a>>),
	Run(&'a core_types::record::GroupItem<'a>),
}

pub enum GraphicLevelColumn<'a, A: Attribute> {
	Legacy(core_types::list::ListColumn<'a, A>),
	Run(core_types::record::RunColumn<'a, A>),
}

impl<'a, A: Attribute> core_types::lane::LaneColumn<'a, A> for GraphicLevelColumn<'a, A> {
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>> {
		match self {
			GraphicLevelColumn::Legacy(column) => column.try_get(lane),
			GraphicLevelColumn::Run(column) => column.try_get(lane),
		}
	}
}

impl<'a> LaneSource for GraphicLevel<'a> {
	type Element = Graphic<'a>;
	type Column<'b, A: Attribute>
		= GraphicLevelColumn<'b, A>
	where
		Self: 'b;

	fn lane_count(&self) -> usize {
		match self {
			GraphicLevel::Legacy(list) => list.len(),
			GraphicLevel::Run(item) => item.len(),
		}
	}

	fn element(&self, lane: usize) -> Option<&Graphic<'a>> {
		match self {
			GraphicLevel::Legacy(list) => list.element(lane),
			GraphicLevel::Run(item) => {
				let lanes = item.typed_lanes::<Graphic>()?;
				if lane >= lanes.len() {
					return None;
				}
				// SAFETY: the layout records the element type, and a parked
				// element stores its reference at offset 0.
				Some(unsafe { core_types::record::borrow_element::<Graphic>(item.lanes().get(lane).rec()) })
			}
		}
	}

	fn column<A: Attribute>(&self) -> GraphicLevelColumn<'_, A> {
		match self {
			GraphicLevel::Legacy(list) => GraphicLevelColumn::Legacy(list.column::<A>()),
			GraphicLevel::Run(item) => GraphicLevelColumn::Run(core_types::record::RunColumn::of(item)),
		}
	}
}

/// The lane's attributes as an owned set, read through the erased glue.
pub fn run_lane_attributes(item: &core_types::record::GroupItem, lane: usize) -> ItemAttributeValues {
	let mut scratch: List<Vector> = List::new_from_element(Vector::default());
	for field in &item.layout().fields {
		// SAFETY: the offset comes from the item's own layout.
		let value = unsafe { (field.read_erased)(item.lanes().get(lane).rec().ptr().add(field.offset)) };
		scratch.set_attribute_value_dyn(field.name, 0, AttributeValueDyn(value));
	}
	scratch.clone_item_attributes(0)
}

/// The lane's attributes as an owned set, from either level storage.
pub fn lane_attributes(level: GraphicLevel<'_>, lane: usize) -> ItemAttributeValues {
	match level {
		GraphicLevel::Legacy(list) => list.clone_item_attributes(lane),
		GraphicLevel::Run(item) => run_lane_attributes(item, lane),
	}
}

/// One flattened vector row served by [`walk_vector_rows`]: cheap probes
/// first, the full row built on demand.
pub struct VectorRow<'w> {
	source: RowSourceRef<'w>,
	scale: FlattenScale,
	layer_path: Option<&'w [NodeId]>,
	paint: LanePaint<'w>,
}

enum RowSourceRef<'w> {
	/// A de-tabled vector leaf on a graphic lane: the lane is the row.
	Lane(GraphicLevel<'w>, usize),
	/// A lane of a vector run.
	Run(&'w core_types::record::RunView<'w, Vector>, &'w core_types::record::GroupItem<'w>, usize),
}

impl VectorRow<'_> {
	/// The row's vector, borrowed.
	pub fn element(&self) -> &Vector {
		match &self.source {
			RowSourceRef::Lane(level, index) => level.element(*index).and_then(Graphic::as_vector).expect("the walk visits vector lanes"),
			RowSourceRef::Run(run, _, index) => LaneSource::element(*run, *index).expect("the walk visits held lanes"),
		}
	}

	/// Whether the built row will carry fill paint: the reaching lane paint,
	/// else the row's own.
	pub fn has_fill(&self) -> bool {
		if self.paint.fill.is_some() {
			return true;
		}
		match &self.source {
			RowSourceRef::Lane(level, index) => paint_graphics::<Fill, _>(level, *index).is_some(),
			RowSourceRef::Run(run, _, index) => paint_graphics::<Fill, _>(*run, *index).is_some(),
		}
	}

	/// Builds the row at the end of `out`, applying the reach paint and the
	/// inherited composition.
	pub fn build_into(&self, out: &mut List<Vector>) {
		let index = out.len();
		match &self.source {
			RowSourceRef::Lane(level, lane) => {
				let vector = self.element().clone();
				out.push(Item::from_parts(vector, lane_attributes(*level, *lane)));
			}
			RowSourceRef::Run(run, item, lane) => {
				let vector = LaneSource::element(*run, *lane).expect("the walk visits held lanes").clone();
				out.push(Item::from_parts(vector, run_lane_attributes(item, *lane)));
			}
		}
		for (key, slot) in [(ATTR_FILL, self.paint.fill), (ATTR_STROKE, self.paint.stroke)] {
			if let Some(paint) = slot {
				set_paint_attribute_at(out, index, key, paint.clone());
			}
		}
		if self.scale.has_transform || out.attribute::<DAffine2>(ATTR_TRANSFORM, index).is_some() {
			let row_transform: DAffine2 = out.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			out.set_attribute(ATTR_TRANSFORM, index, self.scale.transform * row_transform);
		}
		if self.scale.has_opacity || out.attribute::<f64>(ATTR_OPACITY, index).is_some() {
			let row_opacity: f64 = out.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			out.set_attribute(ATTR_OPACITY, index, self.scale.opacity * row_opacity);
		}
		if self.scale.has_fill_opacity || out.attribute::<f64>(ATTR_OPACITY_FILL, index).is_some() {
			let row_fill: f64 = out.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			out.set_attribute(ATTR_OPACITY_FILL, index, self.scale.fill_opacity * row_fill);
		}
		if let Some(layer_path) = self.layer_path {
			out.set_attribute(ATTR_EDITOR_LAYER_PATH, index, layer_path.to_vec());
		}
	}
}

fn walk_rows_of_run(item: &core_types::record::GroupItem, scale: FlattenScale, layer_path: Option<&[NodeId]>, paint: LanePaint<'_>, visit: &mut dyn FnMut(VectorRow<'_>) -> RowStep) -> RowStep {
	let Some(run) = core_types::record::RunView::<Vector>::new(item) else {
		return RowStep::Continue;
	};
	for lane in 0..item.len() {
		if let RowStep::Stop = visit(VectorRow {
			source: RowSourceRef::Run(&run, item, lane),
			scale,
			layer_path,
			paint,
		}) {
			return RowStep::Stop;
		}
	}
	RowStep::Continue
}

/// Walks a graphic level into its flattened vector rows, matching the legacy
/// push-then-flatten lowering: lane paint threads with [`PaintReach`],
/// ancestor transform, opacity and fill opacity compose down, the containing
/// level's parent layer path overwrites its rows, and non-vector content is
/// discarded. A de-tabled leaf's row is its lane, attributes included.
pub fn walk_vector_rows(level: GraphicLevel<'_>, visit: &mut dyn FnMut(VectorRow<'_>) -> RowStep) {
	walk_vector_rows_impl(level, FlattenScale::ROOT, None, PaintReach::NONE, visit);
}

fn walk_vector_rows_impl<'a>(
	level: GraphicLevel<'a>,
	scale: FlattenScale,
	parent_layer_path: Option<&'a [NodeId]>,
	inherited: PaintReach<'a>,
	visit: &mut dyn FnMut(VectorRow<'_>) -> RowStep,
) -> RowStep {
	if let GraphicLevel::Run(item) = level {
		// A vector-typed run is already its rows.
		if item.typed_lanes::<Vector>().is_some() {
			let paint = match inherited.applies() {
				true => inherited.paint,
				false => LanePaint::NONE,
			};
			return walk_rows_of_run(item, scale, parent_layer_path, paint, visit);
		}
	}
	let columns = PaintColumns::new(&level);
	for index in 0..level.lane_count() {
		let Some(element) = level.element(index) else { continue };
		let reach = inherited.for_lane(&columns, index);
		let row_paint = match reach.applies() {
			true => reach.paint,
			false => LanePaint::NONE,
		};
		let step = match element {
			Graphic::Vector(_) => visit(VectorRow {
				source: RowSourceRef::Lane(level, index),
				scale,
				layer_path: parent_layer_path,
				paint: row_paint,
			}),
			Graphic::Graphic(children) => walk_vector_rows_impl(
				GraphicLevel::Legacy(children),
				scale.composed(&level, index),
				level.try_attr::<EditorLayerPath>(index),
				reach.nested(),
				visit,
			),
			Graphic::Group(group) => {
				let item = &group.content;
				if item.typed_lanes::<Vector>().is_some() {
					walk_rows_of_run(item, scale.composed(&level, index), level.try_attr::<EditorLayerPath>(index), row_paint, visit)
				} else if item.typed_lanes::<Graphic>().is_some() {
					walk_vector_rows_impl(
						GraphicLevel::Run(item),
						scale.composed(&level, index),
						level.try_attr::<EditorLayerPath>(index),
						reach.into_group_graphics(),
						visit,
					)
				} else {
					RowStep::Continue
				}
			}
			_ => RowStep::Continue,
		};
		if let RowStep::Stop = step {
			return RowStep::Stop;
		}
	}
	RowStep::Continue
}

/// The level's flattened vector rows as one owned list, the walk's collect
/// form.
pub fn flatten_vector_rows(level: GraphicLevel<'_>) -> List<Vector> {
	let mut out = List::new();
	walk_vector_rows(level, &mut |row| {
		row.build_into(&mut out);
		RowStep::Continue
	});
	out
}

/// The transitional paint placement: a lane-level fill or stroke paint
/// attribute moves onto the vector interiors the legacy paint readers
/// inspect, reaching as far as the pre-flip broadcast did.
pub(in crate::graphic) fn push_lane_paint_into_interiors(list: &mut List<Graphic>) {
	for index in 0..list.len() {
		for key in [ATTR_FILL, ATTR_STROKE] {
			let stored = list.attribute::<Option<List<Graphic>>>(key, index).and_then(|optional| optional.as_ref());
			let Some(paint) = stored.filter(|paint| is_paint_present(paint)).cloned() else {
				continue;
			};
			let Some(Graphic::Graphic(children)) = list.element_mut(index) else { continue };
			for child in 0..children.len() {
				if matches!(children.element(child), Some(Graphic::Vector(_))) {
					set_paint_attribute_at(children, child, key, paint.clone());
				}
			}
		}
	}
}

/// The count [`map_groups_to_legacy`] would expose through [`Graphic::as_vector`],
/// read from the run's lanes instead of materializing the legacy list. Mirrors
/// [`group_to_legacy_graphic`]'s typed-run path, where `Vector` is tried first.
pub fn direct_vector_len(graphic: &Graphic) -> usize {
	match graphic {
		Graphic::Vector(_) => 1,
		Graphic::Group(group) => match &group.row {
			None => group.content.typed_lanes::<Vector>().map_or(0, |lanes| lanes.len()),
			_ => 0,
		},
		_ => 0,
	}
}

pub(in crate::graphic) fn group_render_complexity(group: &core_types::record::Group) -> usize {
	fn typed_run<T: dyn_any::StaticTypeSized + RenderComplexity>(item: &core_types::record::GroupItem) -> Option<usize> {
		let lanes = item.typed_lanes::<T>()?;
		Some((0..lanes.len()).map(|lane| lanes.element_ref(lane).render_complexity()).sum())
	}
	let item = &group.content;
	None.or_else(|| typed_run::<Graphic>(item))
		.or_else(|| typed_run::<Vector>(item))
		.or_else(|| typed_run::<Raster<CPU>>(item))
		.or_else(|| typed_run::<Raster<GPU>>(item))
		.or_else(|| typed_run::<Color>(item))
		.or_else(|| typed_run::<GradientStops>(item))
		.or_else(|| typed_run::<String>(item))
		.unwrap_or(item.len())
}

#[cfg(test)]
mod run_tests {
	use super::*;
	use crate::graphic::test_support::unit_square_at;
	use crate::graphic::{IntoGraphicList, map_groups_to_legacy, run_to_legacy_list};
	use core_types::record::{FieldWrite, RunBuilder, RunView, element_write_hashed};

	#[test]
	fn the_vector_row_walk_matches_the_legacy_flatten() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let mut builder = RunBuilder::new(&arena, element_write_hashed::<Vector>(), &[], 1).unwrap();
		builder.push(inner_vector.clone()).unwrap();
		let inner_item = builder.finish();

		let mut painted = List::new();
		painted.push(Item::new_from_element(Graphic::Vector(unit_square_at(DVec2::ZERO))));
		painted.push(Item::new_from_element(Graphic::Vector(unit_square_at(DVec2::ONE))));
		painted.set_attribute(core_types::ATTR_TRANSFORM, 0, DAffine2::from_translation(DVec2::new(1., 0.)));
		painted.set_attribute(core_types::ATTR_TRANSFORM, 1, DAffine2::from_translation(DVec2::new(0., 1.)));
		set_paint_attribute_at(&mut painted, 1, ATTR_FILL, List::new_from_element(Graphic::Color(Color::WHITE)));

		let mut nested = List::new_from_element(Graphic::Vector(unit_square_at(DVec2::new(2., 2.))));
		nested.set_attribute(core_types::ATTR_TRANSFORM, 0, DAffine2::from_scale(DVec2::splat(2.)));

		let mut top = List::new();
		top.push(Item::new_from_element(Graphic::Graphic(painted)));
		top.push(Item::new_from_element(Graphic::Graphic(nested)));
		top.push(Item::new_from_element(Graphic::Group(core_types::record::Group { row: None, content: inner_item })));
		top.push(Item::new_from_element(Graphic::Color(Color::BLACK)));
		top.push(Item::new_from_element(Graphic::Vector(unit_square_at(DVec2::new(6., 0.)))));
		top.set_attribute(core_types::ATTR_TRANSFORM, 0, DAffine2::from_translation(DVec2::new(5., 5.)));
		top.set_attribute(core_types::ATTR_EDITOR_LAYER_PATH, 0, vec![core_types::uuid::NodeId(7)]);
		set_paint_attribute_at(&mut top, 0, ATTR_FILL, List::new_from_element(Graphic::Color(Color::BLACK)));
		top.set_attribute(core_types::ATTR_OPACITY, 1, 0.5);
		top.set_attribute(core_types::ATTR_TRANSFORM, 2, DAffine2::from_scale(DVec2::splat(3.)));
		top.set_attribute(core_types::ATTR_TRANSFORM, 4, DAffine2::from_translation(DVec2::new(0., 7.)));
		top.set_attribute(core_types::ATTR_EDITOR_LAYER_PATH, 4, vec![core_types::uuid::NodeId(9)]);
		set_paint_attribute_at(&mut top, 4, ATTR_FILL, List::new_from_element(Graphic::Color(Color::WHITE)));

		let legacy = {
			let mut list = List::new();
			for item in top.clone().into_iter() {
				let (element, attributes) = item.into_parts();
				list.push(Item::from_parts(map_groups_to_legacy(&element), attributes));
			}
			push_lane_paint_into_interiors(&mut list);
			list.into_flattened_list::<Vector>()
		};
		let native = flatten_vector_rows(GraphicLevel::Legacy(&top));
		assert_eq!(native.len(), legacy.len());
		for row in 0..native.len() {
			assert_eq!(
				native.attribute::<DAffine2>(core_types::ATTR_TRANSFORM, row),
				legacy.attribute::<DAffine2>(core_types::ATTR_TRANSFORM, row),
				"transform, row {row}"
			);
			assert_eq!(
				native.attribute::<f64>(core_types::ATTR_OPACITY, row),
				legacy.attribute::<f64>(core_types::ATTR_OPACITY, row),
				"opacity, row {row}"
			);
			assert_eq!(
				native.attribute::<Vec<core_types::uuid::NodeId>>(core_types::ATTR_EDITOR_LAYER_PATH, row),
				legacy.attribute::<Vec<core_types::uuid::NodeId>>(core_types::ATTR_EDITOR_LAYER_PATH, row),
				"layer path, row {row}"
			);
			assert_eq!(
				native.attribute::<Option<List<Graphic>>>(ATTR_FILL, row),
				legacy.attribute::<Option<List<Graphic>>>(ATTR_FILL, row),
				"fill, row {row}"
			);
		}
		assert_eq!(native, legacy);
	}

	#[test]
	fn a_run_and_its_legacy_list_agree_on_bounding_boxes() {
		let vectors = [unit_square_at(DVec2::ZERO), unit_square_at(DVec2::new(4., 4.))];
		let transforms = [DAffine2::from_translation(DVec2::new(1., 2.)), DAffine2::from_scale(DVec2::splat(3.))];

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let mut builder = RunBuilder::new(&arena, element_write_hashed::<Vector>(), &[FieldWrite::of::<core_types::attribute::Transform>(0)], 2).unwrap();
		for lane in 0..2 {
			let lane = builder.push(vectors[lane].clone()).unwrap();
			builder.attr::<core_types::attribute::Transform>(lane, transforms[lane]);
		}
		let item = builder.finish();
		let run = RunView::<Vector>::new(&item).expect("the run holds vector elements");
		let legacy = run_to_legacy_list::<Vector>(&item).expect("the run lowers to a legacy vector list");

		let outer = DAffine2::from_angle(0.3);
		for include_stroke in [false, true] {
			let bounds = run.bounding_box(outer, include_stroke);
			assert_eq!(bounds, legacy.bounding_box(outer, include_stroke));
			assert!(matches!(bounds, RenderBoundingBox::Rectangle(_)));
			assert_eq!(run.thumbnail_bounding_box(outer, include_stroke), legacy.thumbnail_bounding_box(outer, include_stroke));
		}
	}
}
