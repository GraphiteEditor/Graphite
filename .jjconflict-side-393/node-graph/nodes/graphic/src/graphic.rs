use core_types::attribute::{Attr, EditorLayerPath, Transform as TransformAttr};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::extent::{ExtentIn, LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt, Level};
use core_types::list::List;
use core_types::registry::types::{Angle, SignedInteger};
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_TRANSFORM, CacheHash, Color, Ctx, DeriveCtx, ExtractIndex, InjectIndex, ModifyIndex};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{ATTR_EDITOR_MERGED_LAYERS, Artboard, Vector};
use raster_types::{CPU, GPU, Raster};

use vector_types::{GradientStop, GradientStops, ReferencePoint};

/// Resolves a signed index over `total` lanes: negatives count from the end,
/// out of range resolves to nothing.
fn resolve_index(index: f64, total: u64) -> Option<u64> {
	let index = index as i64;
	match index < 0 {
		true => total.checked_sub(index.unsigned_abs()),
		false => ((index as u64) < total).then_some(index as u64),
	}
}

/// Returns a one-lane level holding the item at the specified index with its
/// attributes, or an empty level when the index is out of range.
#[node_macro::node(category("General"), extent(index_elements_extent))]
pub fn index_elements<T>(
	ctx: impl Ctx + ModifyIndex + Copy,
	/// The list of data.
	list: impl Node<Context<'_>, Output = T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> Result<T, Interrupt> {
	let total = match list.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("index elements over a non-exact extent").into()),
	};
	let Some(source) = resolve_index(index, total) else {
		return Err(GraphError::new("index elements addressed its empty selection").into());
	};
	let mut shifted = *ctx;
	shifted.set_index(source);
	list.eval(&shifted)
}

fn index_elements_extent(list: ExtentIn<'_>, index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => index.get().zip(list.at(level)).map(|(index, extent)| match extent {
			Extent::Exactly(count) => Extent::Exactly(resolve_index(index, count as u64).is_some() as usize),
			_ => Extent::Exactly(1),
		}),
		false => list.at(level),
	}
}

/// Returns the list with the element at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"), extent(omit_element_extent))]
pub fn omit_element<T>(
	ctx: impl Ctx + ModifyIndex + Copy,
	/// The list of data.
	list: impl Node<Context<'_>, Output = T>,
	/// The index of the item to remove, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> Result<T, Interrupt> {
	let total = match list.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("omit over a non-exact extent").into()),
	};
	let lane = ctx.index();
	let source = match resolve_index(index, total) {
		Some(omitted) if lane >= omitted => lane + 1,
		_ => lane,
	};
	let mut shifted = *ctx;
	shifted.set_index(source);
	list.eval(&shifted)
}

fn omit_element_extent(list: ExtentIn<'_>, index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => index.get().zip(list.at(level)).map(|(index, extent)| match extent {
			Extent::Exactly(count) if resolve_index(index, count as u64).is_some() => Extent::Exactly(count - 1),
			extent => extent,
		}),
		false => list.at(level),
	}
}

/// Returns the bare element (without the item's attributes) at the specified index in a `List`.
/// Use this when downstream nodes want just the inner value rather than a `List` containing a single item.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"))]
pub fn extract_element<T: Clone + Default + Send + Sync + CacheHash + 'static>(
	_: impl Ctx,
	/// The `List` of data to extract from.
	#[implementations(String, f64, NodeId, Color, GradientStops, Vector, Raster<CPU>, Graphic, Artboard)]
	list: IList<T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	resolve_index(index, list.len() as u64).map(|resolved| list.element_ref(resolved as usize).clone()).unwrap_or_default()
}

/// One subgraph invocation per content row, the row riding as a vararg, with
/// the subgraph's lanes concatenated into one flat level. The level reports a
/// lower bound; consumers drain to the past-end signal.
#[node_macro::node(category("General"))]
fn map<Row: Clone + Send + Sync + CacheHash + 'static, T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)] content: IList<Row>,
	mapped: impl Node<Context<'_>, Output = IList<T>>,
) -> Result<IList<T>, Interrupt> {
	let mut remaining = ctx.index();
	for row in 0..content.len() {
		let item = crate::record::vararg_row(content, row);
		let scoped = ctx.push_vararg(&item);
		let lanes = mapped.inner_extent_at(&scoped.ctx(), row as u64)?;
		if remaining >= lanes {
			remaining -= lanes;
			continue;
		}
		let mut frame = core_types::context::IndexLink { index: 0, outer: None };
		return mapped.eval(&scoped.ctx().push_level(&mut frame, row as u64, remaining));
	}
	Err(GraphError::past_end().into())
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

/// One output lane of the mirror over its legacy-converted level: the source
/// row's element and standard attributes, the reflection composed onto the
/// mirrored half's transforms.
#[allow(clippy::type_complexity)]
fn mirror_lane<'e, T: Clone + Default + Send + Sync + 'static>(
	arena: &'e core_types::arena::Arena,
	legacy: List<T>,
	lane: usize,
	relative_to_bounds: ReferencePoint,
	offset: f64,
	angle: f64,
	keep_original: bool,
) -> Result<
	(
		T,
		Attr<'e, TransformAttr>,
		Attr<'e, graphic_types::markers::Fill>,
		Attr<'e, graphic_types::markers::Stroke>,
		Attr<'e, core_types::attribute::BlendMode>,
		Attr<'e, core_types::attribute::Opacity>,
		Attr<'e, core_types::attribute::OpacityFill>,
		Attr<'e, core_types::attribute::ClippingMask>,
		Attr<'e, EditorLayerPath>,
	),
	Interrupt,
>
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

	let exhausted = || {
		Interrupt::from(GraphError {
			kind: core_types::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	let park_paint = |paint: Option<List<Graphic<'static>>>| -> Result<Option<&'e List<Graphic<'static>>>, Interrupt> {
		match paint {
			Some(paint) => Ok(Some(arena.alloc_sized_keyed(paint, 0).ok_or_else(exhausted)?.0)),
			None => Ok(None),
		}
	};

	let element = legacy.element(source).cloned().unwrap_or_default();
	let mut transform: DAffine2 = legacy.attribute_cloned_or_default(ATTR_TRANSFORM, source);
	if mirrored {
		transform = reflected_transform.expect("a mirrored lane exists only under a reflection") * transform;
	}
	let fill = park_paint(legacy.attribute::<Option<List<Graphic>>>(graphic_types::ATTR_FILL, source).cloned().flatten())?;
	let stroke = park_paint(legacy.attribute::<Option<List<Graphic>>>(graphic_types::ATTR_STROKE, source).cloned().flatten())?;
	let layer_path: Vec<NodeId> = legacy.attribute::<Vec<NodeId>>(ATTR_EDITOR_LAYER_PATH, source).cloned().unwrap_or_default();
	let layer_path = arena.alloc(layer_path).ok_or_else(exhausted)?.0;

	Ok((
		element,
		Attr(transform),
		Attr(fill),
		Attr(stroke),
		Attr(legacy.attribute_cloned_or_default(core_types::ATTR_BLEND_MODE, source)),
		Attr(legacy.attribute_cloned_or(core_types::ATTR_OPACITY, source, 1.)),
		Attr(legacy.attribute_cloned_or(core_types::ATTR_OPACITY_FILL, source, 1.)),
		Attr(legacy.attribute_cloned_or_default(core_types::ATTR_CLIPPING_MASK, source)),
		Attr(layer_path.as_slice()),
	))
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
	ctx: impl Ctx + core_types::context::ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Graphic<'static>>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range]
	#[soft(-90..90)]
	angle: Angle,
	#[default(true)] keep_original: bool,
) -> Result<
	IList<(
		Graphic<'static>,
		Attr<'e, TransformAttr>,
		Attr<'e, graphic_types::markers::Fill>,
		Attr<'e, graphic_types::markers::Stroke>,
		Attr<'e, core_types::attribute::BlendMode>,
		Attr<'e, core_types::attribute::Opacity>,
		Attr<'e, core_types::attribute::OpacityFill>,
		Attr<'e, core_types::attribute::ClippingMask>,
		Attr<'e, EditorLayerPath>,
	)>,
	Interrupt,
> {
	mirror_lane(ctx.arena(), legacy_render_list_of(content), ctx.index() as usize, relative_to_bounds, offset, angle, keep_original)
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
	ctx: impl Ctx + core_types::context::ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	content: IList<Vector>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range]
	#[soft(-90..90)]
	angle: Angle,
	#[default(true)] keep_original: bool,
) -> Result<
	IList<(
		Vector,
		Attr<'e, TransformAttr>,
		Attr<'e, graphic_types::markers::Fill>,
		Attr<'e, graphic_types::markers::Stroke>,
		Attr<'e, core_types::attribute::BlendMode>,
		Attr<'e, core_types::attribute::Opacity>,
		Attr<'e, core_types::attribute::OpacityFill>,
		Attr<'e, core_types::attribute::ClippingMask>,
		Attr<'e, EditorLayerPath>,
	)>,
	Interrupt,
> {
	mirror_lane(ctx.arena(), legacy_render_list_of(content), ctx.index() as usize, relative_to_bounds, offset, angle, keep_original)
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
	let (parked, _) = ctx.arena().alloc(path).ok_or(GraphError {
		kind: core_types::gpoll::ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
	Ok((element, Attr(parked.as_slice())))
}

/// Joins two levels of the same type, the base's lanes followed by the new's.
#[node_macro::node(category("General"), extent(extend_extent))]
pub fn extend<T>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The input whose lanes appear at the start of the extended level.
	base: impl Node<Context<'_>, Output = T>,
	/// The input whose lanes appear at the end of the extended level.
	#[expose]
	new: impl Node<Context<'_>, Output = T>,
) -> Result<T, Interrupt> {
	let split = match base.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		// A scalar side joins the concat as a single lane, per `Extent::sum`.
		GPoll::Final(Extent::Free) => 1,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("extend over a non-exact base extent").into()),
	};
	let lane = ctx.index();
	match lane < split {
		true => base.eval(ctx),
		false => {
			let mut shifted = *ctx;
			shifted.set_index(lane - split);
			new.eval(&shifted)
		}
	}
}

/// The top level sums both sides; inner levels must agree (rectangular), a
/// free side or a side with no top-level lanes defers to the other.
fn extend_extent(base: ExtentIn<'_>, new: ExtentIn<'_>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => Extent::sum(base.at(level), new.at(level)),
		false => base.at(level).zip(new.at(level)).and_then(|extents| match extents {
			(Extent::Free, other) | (other, Extent::Free) => GPoll::Final(other),
			(base_inner, new_inner) if base_inner == new_inner => GPoll::Final(base_inner),
			(base_inner, new_inner) => {
				let top = LevelIn {
					level: level.depth - 1,
					depth: level.depth,
				};
				match (base.at(top), new.at(top)) {
					(GPoll::Final(Extent::Exactly(0)), _) => GPoll::Final(new_inner),
					(_, GPoll::Final(Extent::Exactly(0))) => GPoll::Final(base_inner),
					_ => GPoll::error("extend inner extents differ"),
				}
			}
		}),
	}
}

// TODO: Eventually remove this document upgrade code
/// Performs an obsolete function as part of a migration from an older document format.
/// Users are advised to delete this node and replace it with a new one.
#[node_macro::node(category(""))]
pub fn legacy_layer_extend<T: Send + Clone>(
	_: impl Ctx,
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<GradientStops>)] base: List<T>,
	#[expose]
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<GradientStops>)]
	new: List<T>,
	nested_node_path: List<NodeId>,
) -> List<T> {
	// Get the penultimate element of the node path, or None if the path is too short
	// This is used to get the ID of the user-facing parent layer-style node (which encapsulates this internal node).
	let layer = {
		let index = nested_node_path.len().wrapping_sub(2);
		nested_node_path.element(index).copied()
	};

	let mut base = base;
	for mut row in new.into_iter() {
		row.set_attribute(ATTR_EDITOR_LAYER_PATH, layer);
		base.push(row);
	}

	base
}

/// Nests the input graphical content in a wrapper graphic. This essentially "groups" the input.
/// The wrapped run keeps the level's element type, so the legacy boundary can
/// lower a wrapped vector level to the bare typed graphic the pre-flip wrap made.
/// The inverse of this node is 'Flatten Graphic'.
#[node_macro::node(category("General"), extent(wrap_graphic_extent))]
pub fn wrap_graphic<'e, T: Clone + Send + Sync + core_types::CacheHash + 'static>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, GradientStops, String)] content: IList<T>,
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
/// collapse (`to_graphic_typed` serves those rows). The legacy list rows accept an
/// unconverted producer's list value as one element, built as a native group.
#[node_macro::node(category("General"))]
pub fn to_graphic<'e, T: graphic_types::graphic::IntoGraphicElement>(
	ctx: impl Ctx + core_types::context::ExtractArena<'e>,
	#[implementations(
		Graphic,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<GradientStops>,
		List<String>,
	)]
	content: T,
) -> Result<Graphic<'e>, Interrupt> {
	content.into_graphic_element(ctx.arena()).ok_or_else(|| GraphError::new("the arena is exhausted").into())
}

/// The elementwise `Graphic` coercion the compiler-inserted converts use: each
/// lane's element converts on its own, so a typed source feeds a graphic input
/// without changing the level's shape. Registered under the convert identifier.
#[node_macro::node(category(""))]
pub fn to_graphic_element<'e, T: graphic_types::graphic::IntoGraphicElement>(
	ctx: impl Ctx + core_types::context::ExtractArena<'e>,
	#[implementations(
		Graphic,
		Vector,
		Raster<CPU>,
		Raster<GPU>,
		Color,
		GradientStops,
		String,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<GradientStops>,
		List<String>,
	)]
	content: T,
) -> Result<Graphic<'e>, Interrupt> {
	content.into_graphic_element(ctx.arena()).ok_or_else(|| GraphError::new("the arena is exhausted").into())
}

/// The typed-level conversion: the whole level nests as one graphic lane, as
/// the pre-flip `Into<Graphic>` list collapse did. Registered under the to
/// graphic identifier.
#[node_macro::node(category(""), extent(wrap_graphic_extent))]
pub fn to_graphic_typed<'e, T: Clone + Send + Sync + core_types::CacheHash + 'static>(
	_: impl Ctx,
	#[implementations(Vector, Raster<CPU>, Raster<GPU>, Color, GradientStops, String)] content: IList<T>,
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

/// The transitional level bridge: the input's records as the legacy list an
/// unconverted consumer expects, attributes copied through their erased
/// reads and content kept in its native form. Registered under the legacy
/// convert identifiers.
#[node_macro::node(category(""))]
pub fn level_to_list<T: Clone + Send + Sync + CacheHash + dyn_any::StaticTypeSized>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, GradientStops, String)] value: IList<T>,
	_converter: (),
) -> List<T> {
	let item = value.as_group_item();
	graphic_types::graphic::run_to_list::<T>(&item).expect("the run holds the row's element type")
}

pub use _level_to_list_mod::level_to_list_entries;
pub use _to_graphic_element_mod::to_graphic_element_entries;
pub use _to_graphic_typed_mod::to_graphic_typed_entries;
pub use _to_graphic_unit_mod::to_graphic_unit_entries;

/// Removes a level of nesting from a `Graphic[]`, or all nesting if "Fully Flatten" is enabled.
#[node_macro::node(category("General"), extent(flatten_graphic_extent))]
pub fn flatten_graphic(ctx: impl Ctx + ExtractIndex + InjectIndex + Copy, content: IList<Graphic<'static>>, fully_flatten: bool) -> Result<IList<(Graphic<'static>, Attr<TransformAttr>)>, Interrupt> {
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
			return Ok((leaf, Attr(composed)));
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

/// Converts a `Graphic[]` into a `Vector[]` by deeply flattening any vector content it contains, and discarding any non-vector content.
#[node_macro::node(category("Vector"))]
pub fn flatten_vector<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Vector>)] content: T) -> List<Vector> {
	let graphic_list = content.into_graphic_list();
	let mut output: List<Vector> = graphic_list.clone().into_flattened_list();

	// TODO: Replace this snapshot hack with per-layer metadata driven by each layer's Monitor node.
	// TODO: Flattening here erases the upstream `List<Graphic>` hierarchy that editor metadata collection walks
	// TODO: to populate `upstream_footprints` / `local_transforms` / `click_targets` per child layer. As a workaround
	// TODO: we stash the pre-flattened list on the output so `List<Vector>::collect_metadata` can recurse into it,
	// TODO: which conflates render output with editor metadata and forces the pre-compensation dance below.
	// TODO: The cleaner fix is to drive each layer's metadata from its own Monitor's captured `(Context, List<Graphic>)`,
	// TODO: at which point this attribute (and the equivalents in Boolean Operation, Solidify Stroke, Flatten Path,
	// TODO: Morph, Rasterize) become unnecessary.
	if !output.is_empty() {
		// Item 0 carries a composed transform inherited from the flattened input, but the merged_layers
		// already holds the original transforms; pre-compensate by item 0's inverse so the renderer's
		// `upstream_footprint *= item_0_transform` recursion cancels out and leaves the originals intact.
		let mut graphic_list = graphic_list;
		let item_0_transform: DAffine2 = output.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		if item_0_transform.matrix2.determinant().abs() > f64::EPSILON {
			let inverse = item_0_transform.inverse();
			for transform in graphic_list.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
				*transform = inverse * *transform;
			}
		}

		output.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, Some(graphic_list));
	}

	output
}

/// Converts a `Graphic[]` into a `Raster[]` by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Raster"))]
pub fn flatten_raster<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Raster<CPU>>)] content: T) -> List<Raster<CPU>> {
	content.into_flattened_list()
}

/// Converts a `Graphic[]` into a `Color[]` by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("General"))]
pub fn flatten_color<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Color>)] content: T) -> List<Color> {
	content.into_flattened_list()
}

/// Converts a `Graphic[]` into a `GradientStops[]` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"))]
pub fn flatten_gradient<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<GradientStops>)] content: T) -> List<GradientStops> {
	content.into_flattened_list()
}

/// Constructs a gradient from a `Color[]`, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
#[node_macro::node(category("Color"))]
fn colors_to_gradient(_: impl Ctx, colors: IList<Color>) -> GradientStops {
	let stop = |position: f64, color: Color| GradientStop { position, midpoint: 0.5, color };
	match colors.len() {
		0 => GradientStops::new(vec![stop(0., Color::BLACK), stop(1., Color::BLACK)]),
		1 => GradientStops::new(vec![stop(0., colors.get(0)), stop(1., colors.get(0))]),
		total => GradientStops::new((0..total).map(|index| stop(index as f64 / (total - 1) as f64, colors.get(index)))),
	}
}
