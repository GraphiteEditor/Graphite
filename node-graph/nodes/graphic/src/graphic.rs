use crate::record::Inherited;
use core_types::arena::Arena;
use core_types::attribute::{Attr, EditorLayerPath, Name0, Named, Opacity, OpacityFill, Transform as TransformAttr, WireValue};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::extent::{LevelIn, ListIn, ValueIn};
use core_types::gpoll::{ErrorKind, Extent, GPoll, GraphError, Interrupt};
use core_types::list::List;
use core_types::node::Lane;
use core_types::registry::types::Angle;
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color, Ctx, ExtractIndex, InjectIndex};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, GraphicLevel, RowStep, TryFromGraphic, walk_vector_rows};
use graphic_types::markers::{EditorMergedLayers, Fill, Stroke as StrokeAttr};
use graphic_types::{ATTR_FILL, ATTR_STROKE, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::gradient::{GradientForm as GradientFormValue, GradientHueDirection, GradientSpace, GradientSpread};
use vector_types::{Gradient, ReferencePoint};

fn arena_exhausted() -> Interrupt {
	GraphError {
		kind: ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	}
	.into()
}

/// The reflection transform the mirror applies, or nothing when the content
/// has no rectangular bounds (the legacy passthrough case).
fn mirror_reflection<T>(legacy: &List<T>, relative_to_bounds: ReferencePoint, offset: f64, angle: f64) -> Option<DAffine2>
where
	List<T>: BoundingBox,
{
	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let RenderBoundingBox::Rectangle(bounding_box) = legacy.bounding_box(DAffine2::IDENTITY, false) else {
		return None;
	};

	let reference_point_location = relative_to_bounds.point_in_bounding_box((bounding_box[0], bounding_box[1]).into());
	let mirror_reference_point = reference_point_location.map(|point| point + normal * offset);

	// Create the reflection matrix
	let reflection = DAffine2::from_mat2_translation(
		glam::DMat2::from_cols(
			DVec2::new(1. - 2. * normal.x * normal.x, -2. * normal.y * normal.x),
			DVec2::new(-2. * normal.x * normal.y, 1. - 2. * normal.y * normal.y),
		),
		DVec2::ZERO,
	);

	// Apply reflection around the reference point
	Some(if let Some(mirror_reference_point) = mirror_reference_point {
		DAffine2::from_translation(mirror_reference_point) * reflection * DAffine2::from_translation(-mirror_reference_point)
	} else {
		reflection * DAffine2::from_translation(DVec2::from_angle(angle.to_radians()) * DVec2::splat(-offset))
	})
}

/// One output lane of the mirror over its legacy-converted level: the input
/// lane it reflects, that row's element, and the reflection composed onto the
/// mirrored half's transform. The materialized list holds one item per input
/// lane, so `source` names the lane whose columns the output row carries.
fn mirror_lane<T: Clone + Default + Send + Sync + 'static>(
	legacy: List<T>,
	lane: usize,
	relative_to_bounds: ReferencePoint,
	offset: f64,
	angle: f64,
	keep_original: bool,
) -> Result<(usize, T, DAffine2), Interrupt>
where
	List<T>: BoundingBox,
{
	let count = legacy.len();
	let reflected_transform = mirror_reflection(&legacy, relative_to_bounds, offset, angle);
	// Kept originals always double the level so the count stays structural;
	// without a reflection (no rectangular bounds) the second half duplicates.
	let (source, mirrored) = match (keep_original, lane < count) {
		(true, true) => (lane, false),
		(true, false) => (lane - count, reflected_transform.is_some()),
		(false, _) => (lane, reflected_transform.is_some()),
	};
	if source >= count {
		return Err(GraphError::past_end().into());
	}

	let element = legacy.element(source).cloned().unwrap_or_default();
	let mut transform: DAffine2 = legacy.attribute_cloned_or_default(ATTR_TRANSFORM, source);
	if mirrored {
		transform = reflected_transform.expect("a mirrored lane exists only under a reflection") * transform;
	}

	Ok((source, element, transform))
}

/// The materialized level as its legacy list, content kept native.
fn legacy_render_list_of<T: dyn_any::StaticTypeSized>(content: core_types::node::List<'_, T>) -> List<T::Static>
where
	T::Static: Clone + Send + Sync + dyn_any::StaticTypeSized,
{
	let item = content.as_group_item();
	graphic_types::graphic::run_to_list::<T::Static>(&item).expect("the run holds the row's element type")
}

#[node_macro::node(category("General"), extent(mirror_extent))]
fn mirror<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Graphic<'static>>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range]
	#[soft(-90..90)]
	angle: Angle,
	#[default(true)] keep_original: bool,
) -> Result<IList<(Lane<Graphic<'static>>, Attr<'e, TransformAttr>)>, Interrupt> {
	let (source, element, transform) = mirror_lane(legacy_render_list_of(content), ctx.index() as usize, relative_to_bounds, offset, angle, keep_original)?;
	Ok((content.lane(source).map_element(element), Attr(transform)))
}

/// The kept originals double the level, counted from the subject's extent
/// query alone so nested extents stay materialization-free.
fn mirror_extent(
	content: ListIn<'_, Graphic>,
	_relative_to_bounds: ValueIn<'_, ReferencePoint>,
	_offset: ValueIn<'_, f64>,
	_angle: ValueIn<'_, f64>,
	keep_original: ValueIn<'_, bool>,
	level: LevelIn,
) -> GPoll<Extent> {
	match level.top() {
		true => content.total().zip(keep_original.get()).map(|(total, keep_original)| match (total, keep_original) {
			(total, false) => total,
			(Extent::Exactly(count), true) => Extent::Exactly(count * 2),
			(Extent::AtLeast(bound), true) => Extent::AtLeast(bound * 2),
			(Extent::Free, true) => Extent::Free,
		}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// The mirror over a plain vector level, as [`mirror`]. Registered under the
/// mirror identifier.
#[node_macro::node(category(""), extent(mirror_vector_extent))]
fn mirror_vector<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Vector>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range]
	#[soft(-90..90)]
	angle: Angle,
	#[default(true)] keep_original: bool,
) -> Result<IList<(Lane<Vector>, Attr<'e, TransformAttr>)>, Interrupt> {
	let (source, element, transform) = mirror_lane(legacy_render_list_of(content), ctx.index() as usize, relative_to_bounds, offset, angle, keep_original)?;
	Ok((content.lane(source).map_element(element), Attr(transform)))
}

fn mirror_vector_extent(
	content: ListIn<'_, Vector>,
	_relative_to_bounds: ValueIn<'_, ReferencePoint>,
	_offset: ValueIn<'_, f64>,
	_angle: ValueIn<'_, f64>,
	keep_original: ValueIn<'_, bool>,
	level: LevelIn,
) -> GPoll<Extent> {
	match level.top() {
		true => content.total().zip(keep_original.get()).map(|(total, keep_original)| match (total, keep_original) {
			(total, false) => total,
			(Extent::Exactly(count), true) => Extent::Exactly(count * 2),
			(Extent::AtLeast(bound), true) => Extent::AtLeast(bound * 2),
			(Extent::Free, true) => Extent::Free,
		}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

pub use _mirror_vector_mod::mirror_vector_entries;

/// `node_path` with its trailing entry dropped: the containing network's path, which is also a unique
/// reference to the owning document node at any nesting depth. Stamped onto `editor:layer_path`.
#[node_macro::node(name("Path of Subgraph"), category(""))]
pub fn path_of_subgraph(_: impl Ctx, node_path: Vec<NodeId>) -> Vec<NodeId> {
	let len = node_path.len();
	node_path.into_iter().take(len.saturating_sub(1)).collect()
}

/// The layer-path stamp: writes the owning layer's document node path on
/// each lane, which lets editor tools trace data back to its layer.
#[node_macro::node(category(""))]
pub fn stamp_layer_path<'e, T>(ctx: impl Ctx + ExtractArena<'e>, element: T, path: Vec<NodeId>) -> Result<(T, Attr<'e, EditorLayerPath>), Interrupt> {
	let (parked, _) = ctx.arena().alloc(path).ok_or_else(arena_exhausted)?;
	Ok((element, Attr(parked.as_slice())))
}

/// Writes `value` onto each lane under the attribute `name` names. The name is
/// constant text the compiler folds into the layout when the graph compiles, so
/// the write costs exactly what a marker node's does; a name that is not
/// constant is refused there rather than resolved here.
#[node_macro::node(category("Attributes: Write"))]
pub fn write_attribute<'e, T, V: WireValue>(
	ctx: impl Ctx + ExtractArena<'e>,
	content: T,
	/// The attribute name, folded into the layout when the graph compiles.
	name: Named<Name0>,
	#[implementations(f64, u32, u64, bool, DVec2, DAffine2, Color, Vec<NodeId>, String)] value: V,
) -> Result<(T, Attr<'e, Named<Name0, V::Row>>), Interrupt> {
	let parked = value.park(ctx.arena()).ok_or_else(arena_exhausted)?;
	Ok((content, Attr(parked)))
}

// The attribute reads: one node per value type, since a name means one type
// and there is no coercion between them. Each takes any record wire, never
// looks at its element, and serves the name's own default where the attribute
// is absent, so the value always carries the declared type.
//
// The name is constant text the compiler folds into an offset when the graph
// compiles; a name written at another value type is a graph error rather than
// a conversion.
macro_rules! attribute_reads {
	($($(#[$meta:meta])* $node:ident: $row:ty => $value:ty;)*) => {
		$(
			$(#[$meta])*
			#[node_macro::node(category("Attributes: Read"))]
			pub fn $node<T>(
				_: impl Ctx,
				/// The content whose lanes carry the attribute; its element is never read.
				(content, value): (T, Attr<Named<Name0, $row>>),
				/// The attribute name, folded into an offset when the graph compiles.
				name: Named<Name0>,
			) -> $value {
				let _ = content;
				*value
			}
		)*
	};
}

attribute_reads! {
	/// Reads a named `f64` attribute, such as `opacity` or `font_size`.
	read_number_attribute: f64 => f64;
	/// Reads a named `u64` attribute, such as a regex match's `start` or `end`.
	read_integer_attribute: u64 => u64;
	/// Reads a named `bool` attribute, such as `clipping_mask` or `clip`.
	read_bool_attribute: bool => bool;
	/// Reads a named `DVec2` attribute, such as an artboard's `location` or `dimensions`.
	read_coordinate_attribute: DVec2 => DVec2;
	/// Reads a named `DAffine2` attribute, such as `transform`.
	read_transform_attribute: DAffine2 => DAffine2;
	/// Reads a named `Color` attribute, such as an artboard's `background`.
	read_color_attribute: Color => Color;
	/// Reads a named `BlendMode` attribute, such as `blend_mode`.
	read_blend_mode_attribute: core_types::blending::BlendMode => core_types::blending::BlendMode;
	/// Reads a named gradient-shape attribute, such as `gradient_form`.
	read_gradient_form_attribute: GradientFormValue => GradientFormValue;
	/// Reads a named gradient-spread attribute, such as `gradient_spread`.
	read_gradient_spread_attribute: GradientSpread => GradientSpread;
	/// Reads a named gradient-space attribute, such as `gradient_space`.
	read_gradient_space_attribute: GradientSpace => GradientSpace;
	/// Reads a named gradient-hue-direction attribute, such as `gradient_hue_direction`.
	read_gradient_hue_direction_attribute: GradientHueDirection => GradientHueDirection;
}

/// Nests the input graphical content in a wrapper graphic. This essentially "groups" the input.
/// The wrapped run keeps the level's element type, so the legacy boundary can
/// lower a wrapped vector level to the bare typed graphic the pre-flip wrap made.
/// The inverse of this node is 'Flatten Graphic'.
#[node_macro::node(category("General"), extent(wrap_graphic_extent))]
pub fn wrap_graphic<'e, T: Clone + Send + Sync + core_types::CacheHash + 'static>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)] content: IList<T>,
) -> Result<IList<Graphic<'e>>, Interrupt> {
	let item = content.as_group_item();
	Ok(Graphic::Group(core_types::record::Group { row: None, content: item }))
}

/// The collected group is the level's single lane.
fn wrap_graphic_extent<T>(_content: ListIn<'_, T>, _level: LevelIn) -> GPoll<Extent> {
	GPoll::Final(Extent::Exactly(1))
}

/// Converts graphical content into a `Graphic` level. A `Graphic` level passes through
/// unchanged; a typed level nests as one graphic lane, keeping the pre-flip list
/// collapse (`to_graphic_typed` serves those rows).
#[node_macro::node(category("General"))]
pub fn to_graphic<'e, T: graphic_types::graphic::IntoGraphicElement>(ctx: impl Ctx + ExtractArena<'e>, #[implementations(Graphic)] content: T) -> Result<Graphic<'e>, Interrupt> {
	content.into_graphic_element(ctx.arena()).ok_or_else(|| GraphError::new("the arena is exhausted").into())
}

/// The elementwise `Graphic` coercion the compiler-inserted converts use: each
/// lane's element converts on its own, so a typed source feeds a graphic input
/// without changing the level's shape. Registered under the convert identifier.
#[node_macro::node(category(""))]
pub fn to_graphic_element<'e, T: graphic_types::graphic::IntoGraphicElement>(
	ctx: impl Ctx + ExtractArena<'e>,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)] content: T,
) -> Result<Graphic<'e>, Interrupt> {
	content.into_graphic_element(ctx.arena()).ok_or_else(|| GraphError::new("the arena is exhausted").into())
}

/// The typed-level conversion: the whole level nests as one graphic lane, as
/// the pre-flip `Into<Graphic>` list collapse did. Registered under the to
/// graphic identifier.
#[node_macro::node(category(""), extent(wrap_graphic_extent))]
pub fn to_graphic_typed<'e, T: Clone + Send + Sync + core_types::CacheHash + 'static>(
	_: impl Ctx,
	#[implementations(Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)] content: IList<T>,
) -> Result<IList<Graphic<'e>>, Interrupt> {
	let item = content.as_group_item();
	Ok(Graphic::Group(core_types::record::Group { row: None, content: item }))
}

/// An unconnected content input carries the unit, which renders as nothing like
/// the pre-flip empty list. Registered under the to graphic identifier.
#[node_macro::node(category(""), extent(to_graphic_unit_extent))]
pub fn to_graphic_unit(_: impl Ctx, _content: ()) -> Result<IList<Graphic<'static>>, Interrupt> {
	Err(core_types::gpoll::GraphError::past_end().into())
}

fn to_graphic_unit_extent(_content: core_types::extent::ValueIn<'_, ()>, _level: LevelIn) -> GPoll<Extent> {
	GPoll::Final(Extent::Exactly(0))
}

pub use _to_graphic_element_mod::to_graphic_element_entries;
pub use _to_graphic_typed_mod::to_graphic_typed_entries;
pub use _to_graphic_unit_mod::to_graphic_unit_entries;

/// Removes a level of nesting from a `Graphic[]`, or all nesting if "Fully Flatten" is enabled.
///
/// A hoisted leaf carries the columns of the TOP-LEVEL row it came out of, not
/// of the nested lane it sat in. That is forced rather than chosen: a gather's
/// carry is a byte-copy plan resolved once, at wiring, from a statically known
/// layout, and only the subject's own per-lane layout is known then. A leaf's
/// layout belongs to whatever nested level held it and varies leaf by leaf, so
/// there is no single plan that could copy from it. Columns the top row does not
/// supply read their declared defaults, and any it carries that this output does
/// not declare are truncated.
#[node_macro::node(category("General"), extent(flatten_graphic_extent))]
pub fn flatten_graphic<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Graphic<'static>>,
	fully_flatten: bool,
) -> Result<IList<(Lane<Graphic<'static>>, Attr<'e, TransformAttr>)>, Interrupt> {
	let mut remaining = ctx.index() as usize;
	for row in 0..content.len() {
		let graphic = content.element_ref(row);
		let count = crate::record::leaf_count(graphic, fully_flatten, 0);
		if remaining >= count {
			remaining -= count;
			continue;
		}
		let transform: DAffine2 = content.lane(row).attr::<TransformAttr>();
		if let Some((leaf, composed)) = crate::record::locate(graphic, transform, fully_flatten, 0, &mut remaining) {
			// The composed transform is the one genuine override: it is the path's
			// product, not any single lane's column.
			return Ok((content.lane(row).map_element(leaf), Attr(composed)));
		}
	}
	Err(GraphError::new("flatten addressed past its leaf count").into())
}

/// The level holds one row per leaf of the walk.
fn flatten_graphic_extent(content: ListIn<'_, Graphic>, fully_flatten: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => fully_flatten
			.get()
			.zip(content.get())
			.map(|(fully_flatten, content)| Extent::Exactly((0..content.len()).map(|row| crate::record::leaf_count(content.element_ref(row), fully_flatten, 0)).sum())),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// The `lane`-th flattened vector row of `level` as a one-item list, with the
/// top-level lane it descends from.
fn locate_vector_row(level: GraphicLevel<'_>, lane: usize) -> Option<(List<Vector>, usize)> {
	let mut remaining = lane;
	let mut located = None;
	walk_vector_rows(level, &mut |row| {
		if remaining > 0 {
			remaining -= 1;
			return RowStep::Continue;
		}
		let mut one = List::new();
		row.build_into(&mut one);
		located = Some((one, row.top_lane()));
		RowStep::Stop
	});
	located
}

fn vector_row_count(level: GraphicLevel<'_>) -> usize {
	let mut count = 0;
	walk_vector_rows(level, &mut |_| {
		count += 1;
		RowStep::Continue
	});
	count
}

// TODO: Replace this snapshot hack with per-layer metadata driven by each layer's Monitor node.
// TODO: Flattening erases the upstream `Graphic` hierarchy that editor metadata collection walks to populate
// TODO: `upstream_footprints` / `local_transforms` / `click_targets` per child layer, so the pre-flattened list
// TODO: is stashed on row 0 for `collect_metadata` to recurse into (as Boolean Operation, Solidify Stroke,
// TODO: Combine Paths, Morph and Rasterize do). Driving each layer's metadata from its own Monitor's captured
// TODO: `(Context, List<Graphic>)` would make this attribute unnecessary.
/// The parked merged-layers snapshot for row 0. Row 0 carries a composed
/// transform the snapshot's own transforms already include, so the snapshot is
/// pre-compensated by its inverse to cancel the renderer's
/// `upstream_footprint *= row_0_transform` recursion.
fn merged_layers_snapshot<'e>(arena: &'e Arena, mut snapshot: List<Graphic<'static>>, row_0_transform: DAffine2) -> Result<&'e List<Graphic<'static>>, Interrupt> {
	if row_0_transform.matrix2.determinant().abs() > f64::EPSILON {
		let inverse = row_0_transform.inverse();
		for transform in snapshot.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*transform = inverse * *transform;
		}
	}
	arena.alloc_sized_keyed(snapshot, 0).map(|(parked, _)| parked).ok_or_else(arena_exhausted)
}

type FlattenedVectorRow<'a, 'e> = (
	Lane<'a, Vector>,
	Attr<'e, TransformAttr>,
	Attr<'e, Fill>,
	Attr<'e, StrokeAttr>,
	Attr<'e, Opacity>,
	Attr<'e, OpacityFill>,
	Attr<'e, EditorLayerPath>,
	Attr<'e, EditorMergedLayers>,
);

/// A built vector row as the flatten's output: `carrier`'s columns with the
/// walk's composition, paint and layer path overriding, and `snapshot` parked
/// as the merged layers where given.
fn emit_vector_row<'a, 'e>(arena: &'e Arena, carrier: Lane<'a, Graphic<'static>>, row: List<Vector>, snapshot: Option<List<Graphic<'static>>>) -> Result<FlattenedVectorRow<'a, 'e>, Interrupt> {
	let park_paint = |paint: Option<&Option<List<Graphic<'static>>>>| {
		paint
			.and_then(|paint| paint.as_ref())
			.map(|paint| arena.alloc_sized_keyed(paint.clone(), 0).map(|(parked, _)| parked).ok_or_else(arena_exhausted))
			.transpose()
	};
	let fill = park_paint(row.attribute(ATTR_FILL, 0))?;
	let stroke = park_paint(row.attribute(ATTR_STROKE, 0))?;
	let layer_path: Vec<NodeId> = row.attribute(ATTR_EDITOR_LAYER_PATH, 0).cloned().unwrap_or_default();
	let (layer_path, _) = arena.alloc(layer_path).ok_or_else(arena_exhausted)?;

	let transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
	let merged_layers = snapshot.map(|snapshot| merged_layers_snapshot(arena, snapshot, transform)).transpose()?;
	let element = row.element(0).cloned().unwrap_or_default();

	Ok((
		carrier.map_element(element),
		Attr(transform),
		Attr(fill),
		Attr(stroke),
		Attr(row.attribute_cloned_or(ATTR_OPACITY, 0, 1.)),
		Attr(row.attribute_cloned_or(ATTR_OPACITY_FILL, 0, 1.)),
		Attr(layer_path.as_slice()),
		Attr(merged_layers),
	))
}

/// Converts a `Graphic[]` into a `Vector[]` by deeply flattening any vector content it contains, and discarding any non-vector content.
/// Each row carries the columns of the top-level row it descends from, with the
/// path's composed transform and opacities, the reaching paint and the layer path overriding.
#[node_macro::node(category("Vector"), extent(flatten_vector_extent))]
pub fn flatten_vector<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Graphic<'static>>,
) -> Result<
	IList<(
		Lane<Vector>,
		Attr<'e, TransformAttr>,
		Attr<'e, Fill>,
		Attr<'e, StrokeAttr>,
		Attr<'e, Opacity>,
		Attr<'e, OpacityFill>,
		Attr<'e, EditorLayerPath>,
		Attr<'e, EditorMergedLayers>,
	)>,
	Interrupt,
> {
	let lane = ctx.index() as usize;
	let item = content.as_group_item();
	let Some((row, top)) = locate_vector_row(GraphicLevel::Run(&item), lane) else {
		return Err(GraphError::past_end().into());
	};
	let snapshot = (lane == 0).then(|| legacy_render_list_of(content));
	emit_vector_row(ctx.arena(), content.lane(top), row, snapshot)
}

/// The level holds one row per vector leaf of the walk.
fn flatten_vector_extent(content: ListIn<'_, Graphic>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => content.get().map(|content| Extent::Exactly(vector_row_count(GraphicLevel::Run(&content.as_group_item())))),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// The `lane`-th `T` leaf under the content, carrying the columns of the
/// top-level row it descends from with the path's composition overriding.
type FlattenedLeafRow<'a, 'e, T> = (Lane<'a, T>, Attr<'e, TransformAttr>, Attr<'e, Opacity>, Attr<'e, OpacityFill>);

fn flatten_leaf_lane<'a, 'e, T: TryFromGraphic + dyn_any::StaticTypeSized>(content: core_types::node::List<'a, Graphic<'static>>, lane: usize) -> Result<FlattenedLeafRow<'a, 'e, T>, Interrupt> {
	let mut remaining = lane;
	for row in 0..content.len() {
		let carrier = content.lane(row);
		let mut located = None;
		crate::record::walk_typed_leaves(content.element_ref(row), Inherited::of(&carrier), &mut |leaf: &T, inherited| {
			if remaining > 0 {
				remaining -= 1;
				return RowStep::Continue;
			}
			located = Some((leaf.clone(), inherited));
			RowStep::Stop
		});
		if let Some((leaf, inherited)) = located {
			return Ok((carrier.map_element(leaf), Attr(inherited.transform), Attr(inherited.opacity), Attr(inherited.fill_opacity)));
		}
	}
	Err(GraphError::past_end().into())
}

/// The level holds one row per `T` leaf under the content.
fn flatten_leaves_extent<T: TryFromGraphic + dyn_any::StaticTypeSized>(content: ListIn<'_, Graphic>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => content
			.get()
			.map(|content| Extent::Exactly((0..content.len()).map(|row| crate::record::typed_leaf_count::<T>(content.element_ref(row))).sum())),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Converts a `Graphic[]` into a `Raster[]` by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Raster"), extent(flatten_leaves_extent::<Raster<CPU>>))]
pub fn flatten_raster<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Graphic<'static>>,
) -> Result<IList<(Lane<Raster<CPU>>, Attr<'e, TransformAttr>, Attr<'e, Opacity>, Attr<'e, OpacityFill>)>, Interrupt> {
	flatten_leaf_lane(content, ctx.index() as usize)
}

/// Converts a `Graphic[]` into a `Color[]` by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("General"), extent(flatten_leaves_extent::<Color>))]
pub fn flatten_color<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Graphic<'static>>,
) -> Result<IList<(Lane<Color>, Attr<'e, TransformAttr>, Attr<'e, Opacity>, Attr<'e, OpacityFill>)>, Interrupt> {
	flatten_leaf_lane(content, ctx.index() as usize)
}

/// Converts a `Graphic[]` into a `Gradient[]` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"), extent(flatten_leaves_extent::<Gradient>))]
pub fn flatten_gradient<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Graphic<'static>>,
) -> Result<IList<(Lane<Gradient>, Attr<'e, TransformAttr>, Attr<'e, Opacity>, Attr<'e, OpacityFill>)>, Interrupt> {
	flatten_leaf_lane(content, ctx.index() as usize)
}

/// Constructs a gradient from a `Color[]`, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
#[node_macro::node(category("Gradient"), name("Colors to Gradient"))]
pub fn colors_to_gradient(_: impl Ctx, colors: IList<Color>) -> Gradient {
	Gradient::from(colors.iter().collect::<Vec<_>>())
}

/// The gradient over a graphic level's color leaves, as [`colors_to_gradient`].
/// Registered under the colors to gradient identifier.
#[node_macro::node(category(""))]
pub fn colors_to_gradient_graphic(_: impl Ctx, colors: IList<Graphic<'static>>) -> Gradient {
	let mut leaves = Vec::new();
	for row in 0..colors.len() {
		crate::record::walk_typed_leaves::<Color>(colors.element_ref(row), Inherited::IDENTITY, &mut |color, _| {
			leaves.push(*color);
			RowStep::Continue
		});
	}
	Gradient::from(leaves)
}

pub use _colors_to_gradient_graphic_mod::colors_to_gradient_graphic_entries;
