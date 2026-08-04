use core_types::attribute::{Attr, EditorLayerPath, Name0, Named, Transform as TransformAttr, WireValue};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::extent::{ExtentIn, LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt, Level};
use core_types::list::{Item, List, ListDyn};
use core_types::node::Lane;
use core_types::registry::types::{Angle, SeedValue, SignedInteger};
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_TRANSFORM, CacheHash, Color, Ctx, DeriveCtx, ExtractIndex, InjectIndex, ModifyIndex};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{ATTR_EDITOR_MERGED_LAYERS, Artboard, Vector};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use raster_types::{CPU, GPU, Raster};
use std::cmp::Ordering;

use vector_types::gradient::{GradientForm as GradientFormValue, GradientSpread};
use vector_types::{Gradient, ReferencePoint};

/// Resolves a signed index over `total` lanes: negatives count from the end,
/// out of range resolves to nothing.
fn resolve_index(index: f64, total: u64) -> Option<u64> {
	let index = index as i64;
	match index < 0 {
		true => total.checked_sub(index.unsigned_abs()),
		false => ((index as u64) < total).then_some(index as u64),
	}
}

/// Returns the list with the item at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"), name("Remove at Index"), extent(omit_element_extent))]
pub fn remove_at_index<T>(
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
#[node_macro::node(category("General"), name("Item at Index"))]
pub fn item_at_index<T: Clone + Default + Send + Sync + CacheHash + 'static>(
	_: impl Ctx,
	/// The `List` of data to extract from.
	#[implementations(String, f64, NodeId, Color, Gradient, Vector, Raster<CPU>, Graphic, Artboard)]
	list: IList<T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	resolve_index(index, list.len() as u64).map(|resolved| list.element_ref(resolved as usize).clone()).unwrap_or_default()
}

/// Keeps chosen items from a list (those corresponding to `true` values) and discards the others (those corresponding to `false` values) based on the *Keep Pattern* bool list. A short pattern is repeated over the remainder of the filtered list, allowing a pattern like `[true, false]` to keep every other item starting from the first. An empty pattern keeps all items.
#[node_macro::node(category("General"))]
fn filter<T: Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to filter.
	#[implementations(
		List<String>,
		List<bool>,
		List<f32>,
		List<f64>,
		List<u32>,
		List<u64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
	)]
	list: List<T>,
	/// The list of true and false values that determines which corresponding items are kept (`true`) and discarded (`false`). The pattern may repeat if it is shorter than the list of data.
	keep_pattern: List<bool>,
) -> List<T> {
	// Tile the keep pattern over the items, so a short pattern repeats from the start
	let pattern = keep_pattern.iter_element_values().as_slice();
	if pattern.is_empty() {
		return list;
	}

	list.into_iter().enumerate().filter_map(|(index, item)| pattern[index % pattern.len()].then_some(item)).collect()
}

/// Reverses the order of the items in a list, so the last item comes first and the first comes last.
#[node_macro::node(category("General"))]
fn reverse<T: Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to reverse.
	#[implementations(
		List<String>,
		List<bool>,
		List<f32>,
		List<f64>,
		List<u32>,
		List<u64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
	)]
	list: List<T>,
) -> List<T> {
	list.into_iter().rev().collect()
}

/// Shifts the items in a list by a number of positions. With wrapping, items pushed off one end reappear at the other. Otherwise they are dropped, shortening the list.
#[node_macro::node(category("General"))]
fn shift<T: Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to shift.
	#[implementations(
		List<String>,
		List<bool>,
		List<f32>,
		List<f64>,
		List<u32>,
		List<u64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
	)]
	list: List<T>,
	/// How many positions to shift each item. Positive values shift items toward the start of the list, negative toward the end.
	amount: SignedInteger,
	/// Whether items shifted off one end wrap around to the other. When off, they are dropped and the list gets shorter.
	#[default(true)]
	wrap: bool,
) -> List<T> {
	let amount = amount as i64;
	let len = list.len() as i64;
	if len == 0 {
		return list;
	}

	let mut items: Vec<Item<T>> = list.into_iter().collect();
	if wrap {
		items.rotate_left((((amount % len) + len) % len) as usize);
		items.into_iter().collect()
	} else if amount >= 0 {
		items.into_iter().skip(amount.min(len) as usize).collect()
	} else {
		items.into_iter().take((len + amount).max(0) as usize).collect()
	}
}

/// Randomly reorders the items in a list. The same seed always produces the same ordering.
#[node_macro::node(category("General"))]
fn shuffle<T: Send + Sync + 'static>(
	_: impl Ctx,
	/// The list to have its items randomly reordered.
	#[implementations(
		List<String>,
		List<bool>,
		List<f32>,
		List<f64>,
		List<u32>,
		List<u64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
	)]
	list: List<T>,
	/// Seed to determine the unique variation of the random shuffle ordering. The same seed always produces the same ordering.
	seed: SeedValue,
) -> List<T> {
	let mut items: Vec<Item<T>> = list.into_iter().collect();

	let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());
	items.shuffle(&mut rng);

	items.into_iter().collect()
}

/// Generates a list of evenly spaced numbers, starting at a value and progressing by a step (which may be positive, negative, or zero) for a given count.
#[node_macro::node(category("General"), name("Number Sequence"))]
fn number_sequence(
	_: impl Ctx,
	_primary: (),
	/// The first number in the sequence.
	start: f64,
	/// The amount added to reach each successive number.
	#[default(1.)]
	step: f64,
	/// How many numbers to generate.
	#[default(10)]
	count: u32,
) -> List<f64> {
	(0..count).map(|i| Item::new_from_element(start + step * i as f64)).collect()
}

/// Counts out the index of each item in a list (0, 1, 2, and so on), producing a list of numbers with one for each item.
#[node_macro::node(category("General"))]
fn list_indices(
	_: impl Ctx,
	/// The list whose items are counted.
	list: ListDyn,
	/// The number that the count begins from for the first item.
	start_index: SignedInteger,
) -> List<f64> {
	(0..list.len()).map(|index| Item::new_from_element(start_index + index as f64)).collect()
}

/// Extracts a portion of a list, starting at "Start" and ending before "End".
///
/// Negative indices count from the end of the list. If the index of "Start" equals or exceeds "End", the result is an empty list.
#[node_macro::node(category("General"))]
fn list_slice<T: Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to take a portion of.
	#[implementations(
		List<String>,
		List<bool>,
		List<f32>,
		List<f64>,
		List<u32>,
		List<u64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
	)]
	list: List<T>,
	/// The index of the first item in the portion. Negative indices count from the end of the list.
	start: SignedInteger,
	/// The index the portion ends before, which is not included. Zero or negative indices count from the end of the list.
	end: SignedInteger,
) -> List<T> {
	let total_items = list.len();

	let start = if start < 0. {
		total_items.saturating_sub(start.abs() as usize)
	} else {
		(start as usize).min(total_items)
	};
	let end = if end <= 0. {
		total_items.saturating_sub(end.abs() as usize)
	} else {
		(end as usize).min(total_items)
	};

	if start >= end {
		return List::new();
	}

	list.into_iter().skip(start).take(end - start).collect()
}

/// Pairwise ordering used by the Sort node for element values. Types without a natural
/// order compare as equal, so the stable sort leaves their items in their original relative positions.
pub trait ElementOrder {
	fn element_order(&self, _other: &Self) -> Ordering {
		Ordering::Equal
	}
}
impl ElementOrder for String {
	fn element_order(&self, other: &Self) -> Ordering {
		self.cmp(other)
	}
}
impl ElementOrder for bool {
	fn element_order(&self, other: &Self) -> Ordering {
		self.cmp(other)
	}
}
impl ElementOrder for f32 {
	fn element_order(&self, other: &Self) -> Ordering {
		self.total_cmp(other)
	}
}
impl ElementOrder for f64 {
	fn element_order(&self, other: &Self) -> Ordering {
		self.total_cmp(other)
	}
}
impl ElementOrder for u32 {
	fn element_order(&self, other: &Self) -> Ordering {
		self.cmp(other)
	}
}
impl ElementOrder for u64 {
	fn element_order(&self, other: &Self) -> Ordering {
		self.cmp(other)
	}
}
impl ElementOrder for DVec2 {}
impl ElementOrder for DAffine2 {}
impl ElementOrder for Vector {}
impl<'e> ElementOrder for Graphic<'e> {}
impl ElementOrder for Raster<CPU> {}
impl ElementOrder for Raster<GPU> {}
impl ElementOrder for Color {}
impl ElementOrder for Gradient {}
impl<'e> ElementOrder for Artboard<'e> {}

/// Reorders a list's items from smallest to largest, either by each item's own value or by a parallel list of sortable values in the *Sort Order* input. The sort is stable, so items with the same sort order retain their relative positions.
#[node_macro::node(category("General"))]
fn sort<T: ElementOrder + Clone + Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to reorder.
	#[implementations(
		List<String>,
		List<bool>,
		List<f32>,
		List<f64>,
		List<u32>,
		List<u64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
	)]
	list: List<T>,
	/// The optional list of orderable values, corresponding item-to-item with the input list, to sort by instead of the items' own values.
	// The two-generic grid master authors here (f64/String/bool key lists) needs multi-generic implementations support our macro does not have; narrowed to f64 keys.
	#[expose]
	sort_order: List<f64>,
	/// Reverses the sorted list order, following descending order instead of ascending (numbers largest-to-smallest, strings Z-to-A, etc.).
	reverse: bool,
) -> List<T> {
	// Order by the parallel keys when provided (repeating the last if there are fewer keys than items), otherwise by the element values themselves
	let keys = sort_order.iter_element_values().as_slice();
	let elements: Vec<&T> = list.iter_element_values().collect();

	let mut order: Vec<usize> = (0..list.len()).collect();
	order.sort_by(|&a, &b| {
		let ordering = match keys {
			[] => elements[a].element_order(elements[b]),
			keys => keys[a.min(keys.len() - 1)].element_order(&keys[b.min(keys.len() - 1)]),
		};
		if reverse { ordering.reverse() } else { ordering }
	});

	let mut result = List::new();
	for index in order {
		if let Some(item) = list.clone_item(index) {
			result.push(item);
		}
	}

	result
}

/// One subgraph invocation per content row, the row riding as a vararg, with
/// the subgraph's lanes concatenated into one flat level. The level reports a
/// lower bound; consumers drain to the past-end signal.
#[node_macro::node(category("General"))]
fn map<Row: Clone + Send + Sync + CacheHash + 'static, T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, Gradient, String)] content: IList<Row>,
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
	ctx: impl Ctx + core_types::context::ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
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
	ctx: impl Ctx + core_types::context::ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
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
	let (parked, _) = ctx.arena().alloc(path).ok_or(GraphError {
		kind: core_types::gpoll::ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
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
	let parked = value.park(ctx.arena()).ok_or(GraphError {
		kind: core_types::gpoll::ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
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
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)] base: List<T>,
	#[expose]
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)]
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
		List<Gradient>,
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
		Gradient,
		String,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
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

/// The transitional level bridge: the input's records as the legacy list an
/// unconverted consumer expects, attributes copied through their erased
/// reads and content kept in its native form. Registered under the legacy
/// convert identifiers.
#[node_macro::node(category(""))]
pub fn level_to_list<T: Clone + Send + Sync + CacheHash + dyn_any::StaticTypeSized>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)] value: IList<T>,
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
	ctx: impl Ctx + core_types::context::ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
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
	// TODO: at which point this attribute (and the equivalents in Boolean Operation, Solidify Stroke, Combine Paths,
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

/// Converts a `Graphic[]` into a `Gradient[]` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"))]
pub fn flatten_gradient<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Gradient>)] content: T) -> List<Gradient> {
	content.into_flattened_list()
}

/// Constructs a gradient from a `Color[]`, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
#[node_macro::node(category("Gradient"), name("Colors to Gradient"))]
fn colors_to_gradient<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Color>)] colors: T) -> Gradient {
	Gradient::from(colors.into_flattened_list::<Color>())
}

#[cfg(test)]
mod test {
	use super::*;

	fn list_of<T>(elements: impl IntoIterator<Item = T>) -> List<T> {
		elements.into_iter().map(Item::new_from_element).collect()
	}

	fn elements<T: Clone>(list: &List<T>) -> Vec<T> {
		list.iter_element_values().cloned().collect()
	}

	#[test]
	fn sorts_elements_by_their_natural_order() {
		let list = list_of(["banana".to_string(), "apple".to_string(), "cherry".to_string()]);
		let sorted = sort(&(), list, List::<f64>::new(), false);
		assert_eq!(elements(&sorted), ["apple", "banana", "cherry"]);
	}

	#[test]
	fn sorts_elements_in_reverse() {
		let list = list_of([3., 1., 2.]);
		let sorted = sort(&(), list, List::<f64>::new(), true);
		assert_eq!(elements(&sorted), [3., 2., 1.]);
	}

	#[test]
	fn sort_order_keys_override_element_order() {
		let list = list_of(["apple".to_string(), "banana".to_string(), "cherry".to_string()]);
		let sorted = sort(&(), list, list_of([2., 0., 1.]), false);
		assert_eq!(elements(&sorted), ["banana", "cherry", "apple"]);
	}

	#[test]
	fn short_sort_order_repeats_its_last_key() {
		let list = list_of(["a".to_string(), "b".to_string(), "c".to_string()]);
		let sorted = sort(&(), list, list_of([2., 1.]), false);
		assert_eq!(elements(&sorted), ["b", "c", "a"]);
	}

	#[test]
	fn long_sort_order_ignores_its_extra_keys() {
		let list = list_of([1., 2.]);
		let sorted = sort(&(), list, list_of([3., 1., 0., 5.]), false);
		assert_eq!(elements(&sorted), [2., 1.]);
	}

	#[test]
	fn unsortable_elements_keep_their_original_order() {
		let list = list_of([DVec2::new(3., 3.), DVec2::new(1., 1.), DVec2::new(2., 2.)]);
		let sorted = sort(&(), list, List::<f64>::new(), false);
		assert_eq!(elements(&sorted), [DVec2::new(3., 3.), DVec2::new(1., 1.), DVec2::new(2., 2.)]);
	}

	#[test]
	fn shift_wraps_items_around() {
		let forward = shift(&(), list_of([1., 2., 3., 4.]), 1., true);
		assert_eq!(elements(&forward), [2., 3., 4., 1.]);

		let backward = shift(&(), list_of([1., 2., 3., 4.]), -1., true);
		assert_eq!(elements(&backward), [4., 1., 2., 3.]);
	}

	#[test]
	fn shift_without_wrapping_drops_items() {
		let dropped_front = shift(&(), list_of([1., 2., 3., 4.]), 1., false);
		assert_eq!(elements(&dropped_front), [2., 3., 4.]);

		let dropped_back = shift(&(), list_of([1., 2., 3., 4.]), -1., false);
		assert_eq!(elements(&dropped_back), [1., 2., 3.]);
	}

	#[test]
	fn shuffle_is_deterministic_and_preserves_elements() {
		let original = [1., 2., 3., 4., 5., 6., 7., 8.];
		let first = shuffle(&(), list_of(original), 42_u32.into());
		let second = shuffle(&(), list_of(original), 42_u32.into());
		assert_eq!(elements(&first), elements(&second), "the same seed should always produce the same ordering");

		let mut recovered = elements(&first);
		recovered.sort_by(|a, b| a.partial_cmp(b).unwrap());
		assert_eq!(recovered, original, "shuffling should preserve all the elements");
	}

	#[test]
	fn number_sequence_generates_evenly_spaced_numbers() {
		let sequence = number_sequence(&(), (), 0., 2., 4_u32);
		assert_eq!(elements(&sequence), [0., 2., 4., 6.]);
	}

	#[test]
	fn list_indices_counts_each_item() {
		let indices = list_indices(&(), ListDyn::from(list_of(["a".to_string(), "b".to_string(), "c".to_string()])), 0.);
		assert_eq!(elements(&indices), [0., 1., 2.]);

		let from_one = list_indices(&(), ListDyn::from(list_of(["a".to_string(), "b".to_string(), "c".to_string()])), 1.);
		assert_eq!(elements(&from_one), [1., 2., 3.]);
	}

	#[test]
	fn list_slice_takes_the_portion_between_start_and_end() {
		let portion = list_slice(&(), list_of([1., 2., 3., 4., 5.]), 1., 3.);
		assert_eq!(elements(&portion), [2., 3.]);
	}

	#[test]
	fn list_slice_resolves_negative_indices_from_the_end() {
		let portion = list_slice(&(), list_of([1., 2., 3., 4., 5.]), -2., 0.);
		assert_eq!(elements(&portion), [4., 5.], "an end of zero reaches through the end of the list");
	}

	#[test]
	fn list_slice_yields_nothing_when_start_reaches_end() {
		let portion = list_slice(&(), list_of([1., 2., 3., 4., 5.]), 3., 3.);
		assert!(elements(&portion).is_empty());
	}
}
