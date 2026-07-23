use core_types::attr;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::list::{AttributeValueDyn, Item, List, ListDyn, NodeIdPath};
use core_types::registry::types::{Angle, SeedValue, SignedInteger};
use core_types::{AnyHash, BlendMode, CacheHash, CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{Artboard, Vector};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use raster_types::{CPU, GPU, Raster};
use std::cmp::Ordering;
use vector_types::gradient::{GradientSpreadMethod, GradientType};
use vector_types::{Gradient, GradientStop, ReferencePoint};

/// Returns the list with the item at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"), name("Remove at Index"))]
pub fn remove_at_index<T: graphic_types::graphic::OmitIndex + Clone + Default>(
	_: impl Ctx,
	/// The list of data.
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
	list: T,
	/// The index of the item to remove, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: Item<SignedInteger>,
) -> T {
	let index = index.into_element() as i32;

	if index < 0 {
		list.omit_index_from_end(index.unsigned_abs() as usize)
	} else {
		list.omit_index(index as usize)
	}
}

/// Returns the item at the specified index in a list, keeping its attributes.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"), name("Item at Index"))]
pub fn item_at_index<T: Clone + Default + Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to take the item from.
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
		List<Color>,
		List<Gradient>,
		List<Artboard>,
	)]
	list: List<T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: Item<SignedInteger>,
) -> Item<T> {
	let len = list.len();
	let index = index.into_element() as i32;
	let resolved = if index < 0 {
		let from_end = index.unsigned_abs() as usize;
		if from_end > len {
			return Item::default();
		}
		len - from_end
	} else {
		index as usize
	};
	list.clone_item(resolved).unwrap_or_default()
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
	amount: Item<SignedInteger>,
	/// Whether items shifted off one end wrap around to the other. When off, they are dropped and the list gets shorter.
	#[default(true)]
	wrap: Item<bool>,
) -> List<T> {
	let amount = amount.into_element() as i64;
	let wrap = wrap.into_element();
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
	seed: Item<SeedValue>,
) -> List<T> {
	let seed = seed.into_element();
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
	start: Item<f64>,
	/// The amount added to reach each successive number.
	#[default(1.)]
	step: Item<f64>,
	/// How many numbers to generate.
	#[default(10)]
	count: Item<u32>,
) -> List<f64> {
	let (start, step, count) = (*start.element(), *step.element(), count.into_element());

	(0..count).map(|i| Item::new_from_element(start + step * i as f64)).collect()
}

/// Counts out the index of each item in a list (0, 1, 2, and so on), producing a list of numbers with one for each item.
#[node_macro::node(category("General"))]
fn list_indices(
	_: impl Ctx,
	/// The list whose items are counted.
	list: ListDyn,
	/// The number that the count begins from for the first item.
	start_index: Item<SignedInteger>,
) -> List<f64> {
	let start_index = start_index.into_element();

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
	start: Item<SignedInteger>,
	/// The index the portion ends before, which is not included. Zero or negative indices count from the end of the list.
	end: Item<SignedInteger>,
) -> List<T> {
	let (start, end) = (start.into_element(), end.into_element());
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
impl ElementOrder for Graphic {}
impl ElementOrder for Raster<CPU> {}
impl ElementOrder for Raster<GPU> {}
impl ElementOrder for Color {}
impl ElementOrder for Gradient {}
impl ElementOrder for Artboard {}

/// Reorders a list's items from smallest to largest, either by each item's own value or by a parallel list of sortable values in the *Sort Order* input. The sort is stable, so items with the same sort order retain their relative positions.
#[node_macro::node(category("General"))]
fn sort<T: ElementOrder + Clone + Send + Sync + 'static, U: ElementOrder + Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to reorder.
	#[implementations(
		List<String>, List<bool>, List<f32>, List<f64>, List<u32>, List<u64>, List<DVec2>, List<DAffine2>, List<Vector>, List<Graphic>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<Artboard>,
		List<String>, List<bool>, List<f32>, List<f64>, List<u32>, List<u64>, List<DVec2>, List<DAffine2>, List<Vector>, List<Graphic>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<Artboard>,
		List<String>, List<bool>, List<f32>, List<f64>, List<u32>, List<u64>, List<DVec2>, List<DAffine2>, List<Vector>, List<Graphic>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<Artboard>,
	)]
	list: List<T>,
	/// The optional list of orderable values, corresponding item-to-item with the input list, to sort by instead of the items' own values.
	#[expose]
	#[implementations(
		List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>,
		List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>,
		List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>,
	)]
	sort_order: List<U>,
	/// Reverses the sorted list order, following descending order instead of ascending (numbers largest-to-smallest, strings Z-to-A, etc.).
	reverse: Item<bool>,
) -> List<T> {
	let reverse = reverse.into_element();

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

#[node_macro::node(category("General"))]
async fn map<Item: AnyHash + Send + Sync + CacheHash>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
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
	content: List<Item>,
	#[implementations(
		Context -> List<String>,
		Context -> List<bool>,
		Context -> List<f32>,
		Context -> List<f64>,
		Context -> List<u32>,
		Context -> List<u64>,
		Context -> List<DVec2>,
		Context -> List<DAffine2>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<Artboard>,
	)]
	mapped: impl Node<Context<'static>, Output = List<Item>>,
) -> List<Item> {
	let mut rows = List::new();

	for (i, row) in content.into_iter().enumerate() {
		let owned_ctx = OwnedContextImpl::from(ctx.clone());
		let owned_ctx = owned_ctx.with_vararg(Box::new(row)).with_index(i);
		let list = mapped.eval(owned_ctx.into_context()).await;

		rows.extend(list);
	}

	rows
}

#[node_macro::node(category("General"))]
async fn mirror<T: BoundingBox + 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(
		Graphic,
		Vector,
		Raster<CPU>,
		Raster<GPU>,
		Color,
		Gradient,
		String,
	)]
	content: Item<T>,
	#[default(ReferencePoint::Center)] relative_to_bounds: Item<ReferencePoint>,
	#[unit(" px")] offset: Item<f64>,
	#[range]
	#[soft(-90..90)]
	angle: Item<Angle>,
	#[default(true)] keep_original: Item<bool>,
) -> List<T> {
	let (relative_to_bounds, offset, angle, keep_original) = (relative_to_bounds.into_element(), offset.into_element(), angle.into_element(), keep_original.into_element());

	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let item_transform = content.attr_cloned_or_default::<attr::Transform>();
	let RenderBoundingBox::Rectangle(bounding_box) = content.element().bounding_box(item_transform, false) else {
		return List::new_from_item(content);
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
	let reflected_transform = if let Some(mirror_reference_point) = mirror_reference_point {
		DAffine2::from_translation(mirror_reference_point) * reflection * DAffine2::from_translation(-mirror_reference_point)
	} else {
		reflection * DAffine2::from_translation(DVec2::from_angle(angle.to_radians()) * DVec2::splat(-offset))
	};

	let mut result_list = List::new();

	if keep_original {
		result_list.push(content.clone());
	}

	// Add the mirrored copy with the reflection composed onto its transform
	let mut mirrored = content;
	mirrored.set_attr::<attr::Transform>(reflected_transform * item_transform);
	result_list.push(mirrored);

	result_list
}

/// Returns the path identifying the subgraph (network) that contains this proto node — i.e. the input `node_path`
/// with its own trailing entry dropped. The terminating element of the returned path is the document node whose
/// encapsulated network we live in, so the path doubles as a unique reference to that node at any nesting depth.
/// Used as the value source for stamping the `editor:layer_path` attribute on each item of a layer's output, which lets
/// editor tools (e.g. selection, click target routing) trace data back to its owning layer regardless of whether
/// the layer is at the root document network or nested inside a custom subgraph.
#[node_macro::node(name("Path of Subgraph"), category(""))]
pub fn path_of_subgraph(_: impl Ctx, node_path: Item<NodeIdPath>) -> Item<NodeIdPath> {
	let node_path = node_path.into_element().0;
	let len = node_path.len();
	Item::new_from_element(NodeIdPath(node_path.into_iter().take(len.saturating_sub(1)).collect()))
}

/// Sets a named attribute on the input list, computing one value per item via the value-producing input. That input
/// is evaluated once per item, with the item's index and the item itself (as a list containing only that item,
/// passed as a vararg) provided via context, so the upstream pipeline can return a different value per item that may
/// be derived from the item's own data. If the attribute already exists, its values are replaced; if not, it's added.
/// The value is type-erased into an `Item<AttributeValueDyn>` by the auto-inserted input adapter, so this node only
/// monomorphizes over `T` instead of the cartesian product `(T, U)`.
#[node_macro::node(category("Attributes: Write"))]
async fn write_attribute<T: AnyHash + Clone + Send + Sync + CacheHash>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	/// The list to set the named attribute on (one value per item).
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
		List<BlendMode>,
		List<GradientType>,
		List<GradientSpreadMethod>,
	)]
	content: List<T>,
	/// The attribute name (key) to write or replace.
	name: Item<String>,
	/// The node that produces the attribute value for each item. Called once per item with the item's index in context.
	#[implementations(Context -> Item<AttributeValueDyn>)]
	value: impl Node<'n, Context<'static>, Output = Item<AttributeValueDyn>>,
) -> List<T> {
	let name = name.into_element();

	let mut content = content;
	for index in 0..content.len() {
		let row = content.clone_item(index).expect("index is within bounds");
		let owned_ctx = OwnedContextImpl::from(ctx.clone()).with_vararg(Box::new(row)).with_index(index);
		let v = value.eval(owned_ctx.into_context()).await.into_element();
		content.set_attribute_value_dyn(&name, index, v);
	}
	content
}

/// Reads a named `Vector` attribute from the input list, outputting each value as an element of a new `Vector[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_vector(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<Vector> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<Vector>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named numeric attribute (`f64`, `u64`, or `u32`) from the input list, outputting each value as an element of a new `f64[]`. Integer values are converted to `f64`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_number(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<f64> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let value = content
			.attribute_dyn::<f64>(&name, index)
			.copied()
			.or_else(|| content.attribute_dyn::<u64>(&name, index).map(|v| *v as f64))
			.or_else(|| content.attribute_dyn::<u32>(&name, index).map(|v| *v as f64));
		let Some(value) = value else { continue };
		result.push(Item::new_from_element(value));
	}
	result
}

/// Reads a named `bool` attribute from the input list, outputting each value as an element of a new `bool[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_bool(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<bool> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<bool>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `String` attribute from the input list, outputting each value as an element of a new `String[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_string(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<String> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<String>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named `DAffine2` transform attribute from the input list, outputting each value as an element of a new `DAffine2[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_transform(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<DAffine2> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<DAffine2>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `Color` attribute from the input list, outputting each value as an element of a new `Color[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_color(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<Color> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<Color>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `BlendMode` attribute from the input list, outputting each value as an element of a new `BlendMode[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_blend_mode(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<BlendMode> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<BlendMode>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `GradientType` attribute from the input list, outputting each value as an element of a new `GradientType[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_gradient_type(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<GradientType> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<GradientType>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `GradientSpreadMethod` attribute from the input list, outputting each value as an element of a new `GradientSpreadMethod[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_spread_method(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<GradientSpreadMethod> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<GradientSpreadMethod>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	result
}

/// Reads a named `Gradient` attribute from the input list, outputting each value as an element of a new `Gradient[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_gradient_stops(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<Gradient> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<Gradient>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named `Artboard` attribute from the input list, outputting each value as an element of a new `Artboard[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_artboard(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<Artboard> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<Artboard>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Reads a named `Raster` attribute from the input list, outputting each value as an element of a new `Raster[]`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_raster(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> List<Raster<CPU>> {
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute_dyn::<Raster<CPU>>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Joins two lists of the same type, extending the base list with the items from the new list.
#[node_macro::node(category("General"))]
pub async fn extend<T: 'n + Send + Clone>(
	_: impl Ctx,
	/// The list whose items will appear at the start of the extended list.
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
	base: List<T>,
	/// The list whose items will appear at the end of the extended list.
	#[expose]
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
	new: List<T>,
) -> List<T> {
	let mut base = base;
	base.extend(new);

	base
}

// TODO: Eventually remove this document upgrade code
/// Performs an obsolete function as part of a migration from an older document format.
/// Users are advised to delete this node and replace it with a new one.
#[node_macro::node(category(""))]
pub async fn legacy_layer_extend<T: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)] base: List<T>,
	#[expose]
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)]
	new: List<T>,
	nested_node_path: Item<NodeIdPath>,
) -> List<T> {
	// Drop this internal node's own trailing entry so the stamped path ends at the user-facing parent layer-style node (which encapsulates it)
	let nested_node_path = nested_node_path.into_element().0;
	let layer_path = {
		let len = nested_node_path.len();
		NodeIdPath(nested_node_path.into_iter().take(len.saturating_sub(1)).collect())
	};

	let mut base = base;
	for mut row in new.into_iter() {
		row.set_attr::<attr::editor::LayerPath>(layer_path.clone());
		base.push(row);
	}

	base
}

/// Nests the input graphical content in a wrapper graphic. This essentially "groups" the input.
/// The inverse of this node is 'Flatten Graphic'.
#[node_macro::node(category("General"))]
pub async fn wrap_graphic<T: Into<Graphic> + 'n>(
	_: impl Ctx,
	#[implementations(
		List<Graphic>,
	 	List<Vector>,
		List<Raster<CPU>>,
	 	List<Raster<GPU>>,
	 	List<Color>,
		List<Gradient>,
		List<String>,
		Item<DAffine2>,
		Item<DVec2>,
	)]
	content: T,
) -> Item<Graphic> {
	Item::new_from_element(content.into())
}

/// Converts a list of graphical content into a `Graphic[]` by placing it into an element of a new wrapper `Graphic[]`.
/// If it is already a `Graphic[]`, it is not wrapped again. Use the 'Wrap Graphic' node if wrapping is always desired.
#[node_macro::node(category("General"))]
pub async fn to_graphic<T: IntoGraphicList>(
	_: impl Ctx,
	#[implementations(
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<String>,
	)]
	content: T,
) -> List<Graphic> {
	content.into_graphic_list()
}

/// Removes a level of nesting from a `Graphic[]`, or all nesting if "Fully Flatten" is enabled.
#[node_macro::node(category("General"))]
pub async fn flatten_graphic(_: impl Ctx, content: List<Graphic>, fully_flatten: Item<bool>) -> List<Graphic> {
	let fully_flatten = fully_flatten.into_element();

	// TODO: Avoid mutable reference, instead return a new List<Graphic>?
	fn flatten_list(output_graphic_list: &mut List<Graphic>, current_graphic_list: List<Graphic>, fully_flatten: bool, recursion_depth: usize) {
		for index in 0..current_graphic_list.len() {
			let Some(current_element) = current_graphic_list.element(index) else { continue };
			let current_element = current_element.clone();
			let current_transform = current_graphic_list.attr_cloned_or_default::<attr::Transform>(index);

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any graphics we encounter
				Graphic::Graphic(mut current_element) if recurse => {
					// Apply the parent graphic's transform to all child elements
					for graphic_transform in current_element.iter_attr_values_mut_or_default::<attr::Transform>() {
						*graphic_transform = current_transform * *graphic_transform;
					}

					flatten_list(output_graphic_list, current_element, fully_flatten, recursion_depth + 1);
				}
				// Push any leaf elements we encounter: either `Graphic::Graphic(...)` values beyond the recursion depth, or non-`Graphic::Graphic` variants (e.g. `Graphic::Vector`, `Graphic::Raster*`, `Graphic::Color`, `Graphic::Gradient`, `Graphic::Text`)
				_ => {
					let attributes = current_graphic_list.clone_item_attributes(index);
					output_graphic_list.push(Item::from_parts(current_element, attributes));
				}
			}
		}
	}

	let mut output = List::new();
	flatten_list(&mut output, content, fully_flatten, 0);

	output
}

/// Converts a `Graphic[]` into a `Vector[]` by deeply flattening any vector content it contains, and discarding any non-vector content.
#[node_macro::node(category("Vector"))]
pub async fn flatten_vector<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Vector>)] content: T) -> List<Vector> {
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
		let item_0_transform = output.attr_cloned_or_default::<attr::Transform>(0);
		if item_0_transform.matrix2.determinant().abs() > f64::EPSILON {
			let inverse = item_0_transform.inverse();
			for transform in graphic_list.iter_attr_values_mut_or_default::<attr::Transform>() {
				*transform = inverse * *transform;
			}
		}

		output.set_attr::<graphic_types::attr::editor::MergedLayers>(0, graphic_list);
	}

	output
}

/// Converts a `Graphic[]` into a `Raster[]` by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Raster"))]
pub async fn flatten_raster<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Raster<CPU>>)] content: T) -> List<Raster<CPU>> {
	content.into_flattened_list()
}

/// Converts a `Graphic[]` into a `Color[]` by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("General"))]
pub async fn flatten_color<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Color>)] content: T) -> List<Color> {
	content.into_flattened_list()
}

/// Converts a `Graphic[]` into a `Gradient[]` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"))]
pub async fn flatten_gradient<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Gradient>)] content: T) -> List<Gradient> {
	content.into_flattened_list()
}

/// Constructs a gradient from a `Color[]`, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
#[node_macro::node(category("Color"), name("Colors to Gradient"))]
fn colors_to_gradient<T: IntoGraphicList>(_: impl Ctx, #[implementations(List<Graphic>, List<Color>)] colors: T) -> Item<Gradient> {
	let colors = colors.into_flattened_list::<Color>();
	let total_colors = colors.len();

	if total_colors == 0 {
		return Item::new_from_element(Gradient::new(vec![
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: Color::BLACK,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: Color::BLACK,
			},
		]));
	}

	if let (1, Some(&single_color)) = (total_colors, colors.element(0)) {
		return Item::new_from_element(Gradient::new(vec![
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: single_color,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: single_color,
			},
		]));
	}

	let colors = colors.into_iter().enumerate().map(|(index, row)| GradientStop {
		position: index as f64 / (total_colors - 1) as f64,
		midpoint: 0.5,
		color: row.into_element(),
	});
	Item::new_from_element(Gradient::new(colors))
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
		let sorted = sort((), list, List::<f64>::new(), Item::new_from_element(false));
		assert_eq!(elements(&sorted), ["apple", "banana", "cherry"]);
	}

	#[test]
	fn sorts_elements_in_reverse() {
		let list = list_of([3., 1., 2.]);
		let sorted = sort((), list, List::<f64>::new(), Item::new_from_element(true));
		assert_eq!(elements(&sorted), [3., 2., 1.]);
	}

	#[test]
	fn sort_order_keys_override_element_order() {
		let list = list_of(["apple".to_string(), "banana".to_string(), "cherry".to_string()]);
		let sorted = sort((), list, list_of([2., 0., 1.]), Item::new_from_element(false));
		assert_eq!(elements(&sorted), ["banana", "cherry", "apple"]);
	}

	#[test]
	fn short_sort_order_repeats_its_last_key() {
		let list = list_of(["a".to_string(), "b".to_string(), "c".to_string()]);
		let sorted = sort((), list, list_of([2., 1.]), Item::new_from_element(false));
		assert_eq!(elements(&sorted), ["b", "c", "a"]);
	}

	#[test]
	fn long_sort_order_ignores_its_extra_keys() {
		let list = list_of([1., 2.]);
		let sorted = sort((), list, list_of([3., 1., 0., 5.]), Item::new_from_element(false));
		assert_eq!(elements(&sorted), [2., 1.]);
	}

	#[test]
	fn text_sort_order_keys_order_items_alphabetically() {
		let list = list_of([1., 2., 3.]);
		let sorted = sort((), list, list_of(["c".to_string(), "a".to_string(), "b".to_string()]), Item::new_from_element(false));
		assert_eq!(elements(&sorted), [2., 3., 1.]);
	}

	#[test]
	fn unsortable_elements_keep_their_original_order() {
		let list = list_of([DVec2::new(3., 3.), DVec2::new(1., 1.), DVec2::new(2., 2.)]);
		let sorted = sort((), list, List::<f64>::new(), Item::new_from_element(false));
		assert_eq!(elements(&sorted), [DVec2::new(3., 3.), DVec2::new(1., 1.), DVec2::new(2., 2.)]);
	}

	#[test]
	fn shift_wraps_items_around() {
		let forward = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(1.), Item::new_from_element(true));
		assert_eq!(elements(&forward), [2., 3., 4., 1.]);

		let backward = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(-1.), Item::new_from_element(true));
		assert_eq!(elements(&backward), [4., 1., 2., 3.]);
	}

	#[test]
	fn shift_without_wrapping_drops_items() {
		let dropped_front = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(1.), Item::new_from_element(false));
		assert_eq!(elements(&dropped_front), [2., 3., 4.]);

		let dropped_back = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(-1.), Item::new_from_element(false));
		assert_eq!(elements(&dropped_back), [1., 2., 3.]);
	}

	#[test]
	fn shuffle_is_deterministic_and_preserves_elements() {
		let original = [1., 2., 3., 4., 5., 6., 7., 8.];
		let first = shuffle((), list_of(original), Item::new_from_element(42_u32));
		let second = shuffle((), list_of(original), Item::new_from_element(42_u32));
		assert_eq!(elements(&first), elements(&second), "the same seed should always produce the same ordering");

		let mut recovered = elements(&first);
		recovered.sort_by(|a, b| a.partial_cmp(b).unwrap());
		assert_eq!(recovered, original, "shuffling should preserve all the elements");
	}

	#[test]
	fn number_sequence_generates_evenly_spaced_numbers() {
		let sequence = number_sequence((), (), Item::new_from_element(0.), Item::new_from_element(2.), Item::new_from_element(4_u32));
		assert_eq!(elements(&sequence), [0., 2., 4., 6.]);
	}

	#[test]
	fn list_indices_counts_each_item() {
		let indices = list_indices((), ListDyn::from(list_of(["a".to_string(), "b".to_string(), "c".to_string()])), Item::new_from_element(0.));
		assert_eq!(elements(&indices), [0., 1., 2.]);

		let from_one = list_indices((), ListDyn::from(list_of(["a".to_string(), "b".to_string(), "c".to_string()])), Item::new_from_element(1.));
		assert_eq!(elements(&from_one), [1., 2., 3.]);
	}

	#[test]
	fn list_slice_takes_the_portion_between_start_and_end() {
		let portion = list_slice((), list_of([1., 2., 3., 4., 5.]), Item::new_from_element(1.), Item::new_from_element(3.));
		assert_eq!(elements(&portion), [2., 3.]);
	}

	#[test]
	fn list_slice_resolves_negative_indices_from_the_end() {
		let portion = list_slice((), list_of([1., 2., 3., 4., 5.]), Item::new_from_element(-2.), Item::new_from_element(0.));
		assert_eq!(elements(&portion), [4., 5.], "an end of zero reaches through the end of the list");
	}

	#[test]
	fn list_slice_yields_nothing_when_start_reaches_end() {
		let portion = list_slice((), list_of([1., 2., 3., 4., 5.]), Item::new_from_element(3.), Item::new_from_element(3.));
		assert!(elements(&portion).is_empty());
	}
}
