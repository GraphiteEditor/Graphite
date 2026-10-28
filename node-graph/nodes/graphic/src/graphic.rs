use brush_types::Stroke;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::list::{AttributeTypeConflictError, AttributeValueDyn, Item, List, ListDyn, NodeIdPath, implicit_default};
use core_types::{
	ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_TRANSFORM, AnyHash, BlendMode, CacheHash, CloneVarArgs, Color, Context, Ctx, ExtractAll, Interrupt, OwnedContextImpl, descriptor,
};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList, is_lone_anonymous_leaf};
use graphic_types::{Artboard, Vector};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use raster_types::{CPU, GPU, Raster};
use std::cmp::Ordering;
use vector_types::gradient::{GradientForm, GradientHueDirection, GradientInterpolation, GradientSpace, GradientSpread};
use vector_types::{Gradient, ReferencePoint};

/// Returns the list with the item at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("Data: List"), icon("NodeRemoveAtIndex"), name("Remove at Index"))]
fn remove_at_index<T: graphic_types::graphic::OmitIndex + Clone + Default>(
	_: impl Ctx,
	/// The list of data.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
	index: Item<i64>,
) -> T {
	let index = index.into_element();

	if index < 0 {
		list.omit_index_from_end(usize::try_from(index.unsigned_abs()).unwrap_or(usize::MAX))
	} else {
		list.omit_index(usize::try_from(index).unwrap_or(usize::MAX))
	}
}

/// Returns the item at the specified index in a list, keeping its attributes.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("Data: List"), icon("NodeItemAtIndex"), name("Item at Index"))]
fn item_at_index<T: Clone + Default + Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to take the item from.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
	index: Item<i64>,
) -> Item<T> {
	let len = list.len();
	let index = index.into_element();
	let resolved = if index < 0 {
		let from_end = index.unsigned_abs();
		if from_end > len as u64 {
			return Item::default();
		}
		len - from_end as usize
	} else {
		index.min(len as i64) as usize
	};
	list.clone_item(resolved).unwrap_or_default()
}

/// Keeps chosen items from a list (those corresponding to `true` values) and discards the others (those corresponding to `false` values) based on the *Keep Pattern* `Bool` list. A short pattern is repeated over the remainder of the filtered list, allowing a pattern like `[true, false]` to keep every other item starting from the first. An empty pattern keeps all items.
#[node_macro::node(category("Data: List"), icon("NodeFilter"))]
fn filter<T: Send + Sync + Clone + 'static>(
	_: impl Ctx,
	/// The list of data to filter.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
#[node_macro::node(category("Data: List"), icon("NodeReverse"))]
fn reverse<T: Send + Sync + Clone + 'static>(
	_: impl Ctx,
	/// The list of data to reverse.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
#[node_macro::node(category("Data: List"), icon("NodeShift"))]
fn shift<T: Send + Sync + Clone + 'static>(
	_: impl Ctx,
	/// The list of data to shift.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
	amount: Item<i64>,
	/// Whether items shifted off one end wrap around to the other. When off, they are dropped and the list gets shorter.
	#[default(true)]
	wrap: Item<bool>,
) -> List<T> {
	let amount = amount.into_element();
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
		items.into_iter().take((len + amount.max(-len)) as usize).collect()
	}
}

/// Randomly reorders the items in a list. The same seed always produces the same ordering.
#[node_macro::node(category("Data: List"), icon("NodeShuffle"))]
fn shuffle<T: Send + Sync + Clone + 'static>(
	_: impl Ctx,
	/// The list to have its items randomly reordered.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
	#[hard(0..)]
	seed: Item<i64>,
) -> List<T> {
	let seed = seed.into_element();
	let mut items: Vec<Item<T>> = list.into_iter().collect();

	let mut rng = rand::rngs::StdRng::seed_from_u64(seed as u64);
	items.shuffle(&mut rng);

	items.into_iter().collect()
}

/// Generates a list of evenly spaced numbers, starting at a value and progressing by a step (which may be positive, negative, or zero) for a given count.
#[node_macro::node(category("Data: List"), icon("NodeNumberSequence"), name("Number Sequence"))]
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
	#[hard(0..)]
	count: Item<i64>,
) -> List<f64> {
	let (start, step, count) = (*start.element(), *step.element(), count.into_element());

	(0..count).map(|i| Item::new_from_element(start + step * i as f64)).collect()
}

// TODO: Return i64 instead of f64 once automatic type conversion is implemented for nodes with generic type inputs, so an integer output doesn't wall these indices off from the generic math nodes.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
/// Counts out the index of each item in a list (0, 1, 2, and so on), producing a list of numbers with one for each item.
#[node_macro::node(category("Data: List"), icon("NodeIndices"))]
fn indices(
	_: impl Ctx,
	/// The list whose items are counted.
	list: ListDyn,
	/// The number that the count begins from for the first item.
	start_index: Item<i64>,
) -> List<f64> {
	let start_index = start_index.into_element();

	(0..list.len()).map(|index| Item::new_from_element(start_index as f64 + index as f64)).collect()
}

/// Extracts a portion of a list, starting at "Start" and ending before "End".
///
/// Negative indices count from the end of the list. If the index of "Start" equals or exceeds "End", the result is an empty list.
#[node_macro::node(category("Data: List"), icon("NodeSlice"))]
fn slice<T: Send + Sync + Clone + 'static>(
	_: impl Ctx,
	/// The list of data to take a portion of.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
	start: Item<i64>,
	/// The index the portion ends before, which is not included. Zero or negative indices count from the end of the list.
	end: Item<i64>,
) -> List<T> {
	let (start, end) = (start.into_element(), end.into_element());
	let total_items = list.len() as i64;

	let start = if start < 0 { (total_items + start).max(0) } else { start.min(total_items) } as usize;
	let end = if end <= 0 { (total_items + end).max(0) } else { end.min(total_items) } as usize;

	if start >= end {
		return List::new();
	}

	list.into_iter().skip(start).take(end - start).collect()
}

/// Pairwise ordering used by the Sort node for element values. Types without a natural order give `None`,
/// so their items can only be sorted by a *Sort Order* list.
pub trait ElementOrder {
	fn element_order(&self, _other: &Self) -> Option<Ordering> {
		None
	}
}
impl ElementOrder for String {
	fn element_order(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
impl ElementOrder for bool {
	fn element_order(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
impl ElementOrder for f64 {
	fn element_order(&self, other: &Self) -> Option<Ordering> {
		Some(self.total_cmp(other))
	}
}
impl ElementOrder for i64 {
	fn element_order(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
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

#[derive(Debug, thiserror::Error)]
pub enum SortError {
	#[error("These items have no natural order, so connect a Sort Order list to sort them")]
	NoNaturalOrder,
}

/// Reorders a list's items from smallest to largest, either by each item's own value or by a parallel list of sortable values in the *Sort Order* input. The sort is stable, so items with the same sort order retain their relative positions.
#[node_macro::node(category("Data: List"), icon("NodeSort"))]
fn sort<T: ElementOrder + Clone + Send + Sync + 'static, U: ElementOrder + Send + Sync + 'static>(
	_: impl Ctx,
	/// The list of data to reorder.
	#[implementations(
		List<String>, List<bool>, List<f64>, List<i64>, List<DVec2>, List<DAffine2>, List<Vector>, List<Graphic>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<Artboard>,
		List<String>, List<bool>, List<f64>, List<i64>, List<DVec2>, List<DAffine2>, List<Vector>, List<Graphic>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<Artboard>,
		List<String>, List<bool>, List<f64>, List<i64>, List<DVec2>, List<DAffine2>, List<Vector>, List<Graphic>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>, List<Artboard>,
	)]
	list: List<T>,
	/// The optional list of orderable values, corresponding item-to-item with the input list, to sort by instead of the items' own values.
	#[expose]
	#[implementations(
		List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>, List<f64>,
		List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>, List<String>,
		List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>, List<bool>,
	)]
	sort_order: List<U>,
	/// Reverses the sorted list order, following descending order instead of ascending (numbers largest-to-smallest, strings Z-to-A, etc.).
	reverse: Item<bool>,
) -> Result<List<T>, SortError> {
	let reverse = reverse.into_element();

	// Order by the parallel keys when provided (repeating the last if there are fewer keys than items), otherwise by the element values themselves
	let keys = sort_order.iter_element_values().as_slice();
	let elements: Vec<&T> = list.iter_element_values().collect();

	let mut unordered = false;
	let mut order: Vec<usize> = (0..list.len()).collect();
	order.sort_by(|&a, &b| {
		let ordering = match keys {
			[] => elements[a].element_order(elements[b]),
			keys => keys[a.min(keys.len() - 1)].element_order(&keys[b.min(keys.len() - 1)]),
		};
		let ordering = ordering.unwrap_or_else(|| {
			unordered = true;
			Ordering::Equal
		});
		if reverse { ordering.reverse() } else { ordering }
	});
	if unordered {
		return Err(SortError::NoNaturalOrder);
	}

	let mut result = List::new();
	for index in order {
		if let Some(item) = list.clone_item(index) {
			result.push(item);
		}
	}

	Ok(result)
}

#[node_macro::node(category("Data: List"), icon("NodeMap"))]
fn map<Item: AnyHash + Send + Sync + CacheHash + Clone>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
		Context -> List<f64>,
		Context -> List<i64>,
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
) -> Result<List<Item>, Interrupt> {
	let mut rows = List::new();

	for (i, row) in content.into_iter().enumerate() {
		let owned_ctx = OwnedContextImpl::from(ctx.clone());
		let owned_ctx = owned_ctx.with_vararg(Box::new(row)).with_index(i);
		let list = mapped.eval(owned_ctx.into_context())?;

		rows.extend(list);
	}

	Ok(rows)
}

#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
	#[error("Text and fills have no bounds to mirror relative to, so set Relative to Bounds to None")]
	NoBounds,
}

/// Whether the content holds text, which has no bounds in the graph.
pub trait HasText {
	fn has_text(&self) -> bool {
		false
	}
}
impl HasText for String {
	fn has_text(&self) -> bool {
		!self.is_empty()
	}
}
impl HasText for Graphic {
	fn has_text(&self) -> bool {
		match self {
			Graphic::Text(item) => item.element().has_text(),
			Graphic::TextList(list) => list.iter_element_values().any(HasText::has_text),
			Graphic::Graphic(item) => item.element().has_text(),
			Graphic::GraphicList(list) => list.iter_element_values().any(HasText::has_text),
			_ => false,
		}
	}
}
impl HasText for Vector {}
impl HasText for Raster<CPU> {}
impl HasText for Raster<GPU> {}
impl HasText for Color {}
impl HasText for Gradient {}

#[node_macro::node(category("Graphic"), icon("NodeMirror"))]
fn mirror<T: BoundingBox + HasText + Send + Clone>(
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
	#[unit("°")]
	#[range]
	#[soft(-90..90)]
	angle: Item<f64>,
	#[default(true)] keep_original: Item<bool>,
) -> Result<List<T>, MirrorError> {
	let (relative_to_bounds, offset, angle, keep_original) = (relative_to_bounds.into_element(), offset.into_element(), angle.into_element(), keep_original.into_element());

	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let item_transform: DAffine2 = content.attribute_cloned_or_default(ATTR_TRANSFORM);
	let mirror_reference_point = match relative_to_bounds {
		ReferencePoint::None => None,
		reference_point => match content.bounding_box(DAffine2::IDENTITY, false) {
			RenderBoundingBox::Rectangle(bounding_box) => reference_point.point_in_bounding_box((bounding_box[0], bounding_box[1]).into()).map(|point| point + normal * offset),
			// Empty content looks the same mirrored
			RenderBoundingBox::None if !content.element().has_text() => return Ok(std::iter::repeat_n(content, if keep_original { 2 } else { 1 }).collect()),
			_ => return Err(MirrorError::NoBounds),
		},
	};

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
	mirrored.set_attribute(ATTR_TRANSFORM, reflected_transform * item_transform);
	result_list.push(mirrored);

	Ok(result_list)
}

/// Returns the path identifying the subgraph (network) that contains this proto node — i.e. the input `node_path`
/// with its own trailing entry dropped. The terminating element of the returned path is the document node whose
/// encapsulated network we live in, so the path doubles as a unique reference to that node at any nesting depth.
/// Used as the value source for stamping the `editor:layer_path` attribute on each item of a layer's output, which lets
/// editor tools (e.g. selection, click target routing) trace data back to its owning layer regardless of whether
/// the layer is at the root document network or nested inside a custom subgraph.
#[node_macro::node(name("Path of Subgraph"), category(""))]
fn path_of_subgraph(_: impl Ctx, node_path: Item<NodeIdPath>) -> Item<NodeIdPath> {
	let node_path = node_path.into_element().0;
	let len = node_path.len();
	Item::new_from_element(NodeIdPath(node_path.into_iter().take(len.saturating_sub(1)).collect()))
}

#[derive(Debug, thiserror::Error)]
pub enum AttributeTypeError {
	#[error("The \"{name}\" attribute holds {held} values, not {requested}")]
	WrongType { name: String, held: String, requested: String },
	#[error(transparent)]
	Conflict(#[from] AttributeTypeConflictError),
}

/// Sets a named attribute on the input list, computing one value per item via the value-producing input. That input
/// is evaluated once per item, with the item's index and the item itself (as a list containing only that item,
/// passed as a vararg) provided via context, so the upstream pipeline can return a different value per item that may
/// be derived from the item's own data. If the attribute already exists, its values are replaced; if not, it's added.
/// The value is type-erased into an `Item<AttributeValueDyn>` by the auto-inserted input adapter, so this node only
/// monomorphizes over `T` instead of the cartesian product `(T, U)`.
#[node_macro::node(category("Data: Attribute"), icon("NodeWriteAttribute"))]
fn write_attribute<T: AnyHash + Clone + Send + Sync + CacheHash>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	/// The list to set the named attribute on (one value per item).
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
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
		List<GradientForm>,
		List<GradientSpread>,
	)]
	content: List<T>,
	/// The attribute name (key) to write or replace.
	#[constant]
	name: Item<String>,
	/// The node that produces the attribute value for each item. Called once per item with the item's index in context.
	value: impl Node<Context<'static>, Output = Item<AttributeValueDyn>>,
) -> Result<List<T>, Interrupt> {
	let name = name.into_element();

	let mut content = content;
	for index in 0..content.len() {
		let row = content.clone_item(index).expect("index is within bounds");
		let owned_ctx = OwnedContextImpl::from(ctx.clone()).with_vararg(Box::new(row)).with_index(index);
		let v = value.eval(owned_ctx.into_context())?.into_element();
		content.set_attribute_value_dyn(&name, index, v);
	}
	Ok(content)
}

/// Reads a named `Vector` attribute from the input list, outputting each value as an element of a new `Vector[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_vector(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<Vector>, AttributeTypeError> {
	read_list_attribute(&content, name.element())
}

/// Reads a named `Number` or `Integer` attribute from the input list, outputting each value as an element of a new `Number[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_number(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<f64>, AttributeTypeError> {
	let name = name.into_element();
	if content.attribute_type(&name) == Some(descriptor!(i64)) {
		return Ok((0..content.len())
			.filter_map(|index| content.attribute::<i64>(&name, index))
			.map(|value| Item::new_from_element(*value as f64))
			.collect());
	}

	read_attribute(&content, &name)
}

/// Reads a named `Bool` attribute from the input list, outputting each value as an element of a new `Bool[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_bool(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<bool>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `String` attribute from the input list, outputting each value as an element of a new `String[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_string(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<String>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `Transform` attribute from the input list, outputting each value as an element of a new `Transform[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_transform(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<DAffine2>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `Color` attribute from the input list, outputting each value as an element of a new `Color[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_color(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<Color>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `BlendMode` attribute from the input list, outputting each value as an element of a new `BlendMode[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_blend_mode(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<BlendMode>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `GradientForm` attribute from the input list, outputting each value as an element of a new `GradientForm[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_gradient_form(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<GradientForm>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `GradientSpread` attribute from the input list, outputting each value as an element of a new `GradientSpread[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_gradient_spread(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<GradientSpread>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `GradientSpace` attribute from the input list, outputting each value as an element of a new `GradientSpace[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_gradient_space(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<GradientSpace>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `GradientInterpolation` attribute from the input list, outputting each value as an element of a new `GradientInterpolation[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_gradient_interpolation(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<GradientInterpolation>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `GradientHueDirection` attribute from the input list, outputting each value as an element of a new `GradientHueDirection[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_gradient_hue_direction(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<GradientHueDirection>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `Gradient` attribute from the input list, outputting each value as an element of a new `Gradient[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_gradient_stops(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<Gradient>, AttributeTypeError> {
	read_list_attribute(&content, name.element())
}

/// Reads a named `Artboard` attribute from the input list, outputting each value as an element of a new `Artboard[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_artboard(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<Artboard>, AttributeTypeError> {
	read_attribute(&content, name.element())
}

/// Reads a named `Raster` attribute from the input list, outputting each value as an element of a new `Raster[]`.
#[node_macro::node(category("Data: Attribute"), icon("NodeReadAttribute"))]
fn read_attribute_raster(
	_: impl Ctx,
	content: ListDyn,
	/// The attribute name (key) to read.
	#[constant]
	name: Item<String>,
) -> Result<List<Raster<CPU>>, AttributeTypeError> {
	read_list_attribute(&content, name.element())
}

/// Reads the named attribute's value for every item, reading an attribute no item has as each item's implicit default.
fn read_attribute<T: Clone + Default + 'static>(content: &ListDyn, name: &str) -> Result<List<T>, AttributeTypeError> {
	let requested = descriptor!(T);
	if let Some(held) = content.attribute_type(name)
		&& held != requested
	{
		return Err(AttributeTypeError::WrongType {
			name: name.to_string(),
			held: held.to_string(),
			requested: requested.to_string(),
		});
	}

	Ok((0..content.len())
		.map(|index| Item::new_from_element(content.attribute::<T>(name, index).cloned().unwrap_or_else(|| implicit_default(name))))
		.collect())
}

/// Reads the named attribute like [`read_attribute`], joining the lists of an attribute holding one list per item, as Write Attribute stores a list.
fn read_list_attribute<T: Clone + Default + 'static>(content: &ListDyn, name: &str) -> Result<List<T>, AttributeTypeError> {
	if content.attribute_type(name) != Some(descriptor!(List<T>)) {
		return read_attribute(content, name);
	}

	let mut result = List::new();
	for list in (0..content.len()).filter_map(|index| content.attribute::<List<T>>(name, index)) {
		if let Some(conflict) = result.extend(list.clone()) {
			return Err(conflict.into());
		}
	}
	Ok(result)
}

/// Joins two lists of the same type, extending the base list with the items from the new list.
#[node_macro::node(category("Data: List"), icon("NodeExtend"), memoize)]
fn extend<T: Send + Clone>(
	_: impl Ctx,
	/// The list whose items will appear at the start of the extended list.
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
		List<Stroke>,
	)]
	base: List<T>,
	/// The list whose items will appear at the end of the extended list.
	#[expose]
	#[implementations(
		List<String>,
		List<bool>,
		List<f64>,
		List<i64>,
		List<DVec2>,
		List<DAffine2>,
		List<Vector>,
		List<Graphic>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<Artboard>,
		List<Stroke>,
	)]
	new: List<T>,
) -> Result<List<T>, (List<T>, AttributeTypeConflictError)> {
	let mut base = base;
	match base.extend(new) {
		Some(conflict) => Err((base, conflict)),
		None => Ok(base),
	}
}

// TODO: Eventually remove this document upgrade code
/// Performs an obsolete function as part of a migration from an older document format.
/// Users are advised to delete this node and replace it with a new one.
#[node_macro::node(category(""))]
fn legacy_layer_extend<T: Send + Clone>(
	_: impl Ctx,
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)] base: List<T>,
	#[expose]
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)]
	new: List<T>,
	nested_node_path: Item<NodeIdPath>,
) -> Result<List<T>, (List<T>, AttributeTypeConflictError)> {
	// Drop this internal node's own trailing entry so the stamped path ends at the user-facing parent layer-style node (which encapsulates it)
	let nested_node_path = nested_node_path.into_element().0;
	let layer_path = {
		let len = nested_node_path.len();
		NodeIdPath(nested_node_path.into_iter().take(len.saturating_sub(1)).collect())
	};

	let mut base = base;
	let mut conflict = None;
	for mut row in new.into_iter() {
		row.set_attribute(ATTR_EDITOR_LAYER_PATH, layer_path.clone());
		if let Some(row_conflict) = base.push(row) {
			conflict.get_or_insert(row_conflict);
		}
	}

	match conflict {
		Some(conflict) => Err((base, conflict)),
		None => Ok(base),
	}
}

/// Nests the input graphical content in a wrapper graphic, collecting it all into a single group.
/// The inverse of this node is 'Flatten Graphic'.
#[node_macro::node(category("Graphic"), icon("NodeIntoGroup"))]
fn into_group<T: Into<Graphic>>(
	_: impl Ctx,
	#[implementations(
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
		List<String>,
		List<DVec2>,
		Item<DAffine2>, // TODO: Remove this
		List<Stroke>,
	)]
	content: T,
) -> Item<Graphic> {
	Item::new_from_element(content.into())
}

/// Type-asserts a value to be graphical content, converting each item of other content types into its matching form.
/// Use the 'Into Group' node instead to collect the content into a single group.
#[node_macro::node(category("Data: Type Assertion"), icon("NodeAsGraphic"))]
fn as_graphic(_: impl Ctx, value: Item<Graphic>) -> Item<Graphic> {
	value
}

/// Removes a level of nesting from a `Graphic[]`, or all nesting if "Fully Flatten" is enabled.
#[node_macro::node(category("Graphic"), icon("NodeFlattenGraphic"))]
fn flatten_graphic(_: impl Ctx, content: List<Graphic>, fully_flatten: Item<bool>) -> Result<List<Graphic>, (List<Graphic>, AttributeTypeConflictError)> {
	let fully_flatten = fully_flatten.into_element();

	// TODO: Avoid mutable reference, instead return a new List<Graphic>?
	fn flatten_list(output_graphic_list: &mut List<Graphic>, conflict: &mut Option<AttributeTypeConflictError>, current_graphic_list: List<Graphic>, fully_flatten: bool, recursion_depth: usize) {
		for index in 0..current_graphic_list.len() {
			let Some(current_element) = current_graphic_list.element(index) else { continue };
			let current_element = current_element.clone();
			let current_transform: DAffine2 = current_graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);

			let recurse = fully_flatten || recursion_depth == 0;

			// A boxed single graphic is the rank-0 spelling of the same nesting, so it flattens through the list path
			let current_element = match current_element {
				Graphic::Graphic(item) if recurse => Graphic::GraphicList(List::new_from_item(*item)),
				element => element,
			};

			match current_element {
				// If we're allowed to recurse, flatten any graphics we encounter
				Graphic::GraphicList(mut current_element) if recurse => {
					// Apply the parent graphic's transform to all child elements
					for graphic_transform in current_element.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
						*graphic_transform = current_transform * *graphic_transform;
					}

					flatten_list(output_graphic_list, conflict, current_element, fully_flatten, recursion_depth + 1);
				}
				// Push any leaf element: a group beyond the recursion depth, or any non-group variant
				_ => {
					let attributes = current_graphic_list.clone_item_attributes(index);
					if let Some(item_conflict) = output_graphic_list.push(Item::from_parts(current_element, attributes)) {
						conflict.get_or_insert(item_conflict);
					}
				}
			}
		}
	}

	let mut output = List::new();
	let mut conflict = None;
	flatten_list(&mut output, &mut conflict, content, fully_flatten, 0);

	match conflict {
		Some(conflict) => Err((output, conflict)),
		None => Ok(output),
	}
}

/// Converts a `Graphic[]` into a `Vector[]` by deeply flattening any vector content it contains, and discarding any non-vector content.
#[node_macro::node(category("Graphic"), icon("NodeFlattenVector"))]
pub fn flatten_vector(_: impl Ctx, content: List<Graphic>) -> Result<List<Vector>, (List<Vector>, AttributeTypeConflictError)> {
	let mut flattened = content.clone().try_into_flattened_list();
	let (Ok(output) | Err((output, _))) = &mut flattened;

	// TODO: Replace this snapshot hack with per-layer metadata driven by each layer's Monitor node.
	// TODO: Flattening here erases the upstream `List<Graphic>` hierarchy that editor metadata collection walks
	// TODO: to populate `upstream_footprints` / `local_transforms` / `click_targets` per child layer. As a workaround
	// TODO: we stash the pre-flattened list on the output so `List<Vector>::collect_metadata` can recurse into it,
	// TODO: which conflates render output with editor metadata and forces the pre-compensation dance below.
	// TODO: The cleaner fix is to drive each layer's metadata from its own Monitor's captured `(Context, List<Graphic>)`,
	// TODO: at which point this attribute (and the equivalents in Boolean Operation, Solidify Stroke, Combine Paths,
	// TODO: Morph, Rasterize) become unnecessary.
	if !output.is_empty() && !is_lone_anonymous_leaf(&content) {
		// Item 0 carries a composed transform inherited from the flattened input, but the merged_layers
		// already holds the original transforms; pre-compensate by item 0's inverse so the renderer's
		// `upstream_footprint *= item_0_transform` recursion cancels out and leaves the originals intact.
		let mut merged_layers = content;
		let item_0_transform: DAffine2 = output.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		if item_0_transform.matrix2.determinant().abs() > f64::EPSILON {
			let inverse = item_0_transform.inverse();
			for transform in merged_layers.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
				*transform = inverse * *transform;
			}
		}

		output.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, merged_layers);
	}

	flattened
}

#[derive(Debug, thiserror::Error)]
pub enum FlattenRasterError {
	#[error("GPU rasters such as brush strokes can't be flattened yet, so they were left out")]
	GpuRasters,
	#[error(transparent)]
	AttributeTypeConflict(#[from] AttributeTypeConflictError),
}

/// Whether the content holds GPU rasters, which flattening to CPU rasters leaves out.
fn has_gpu_raster(graphic: &Graphic) -> bool {
	match graphic {
		Graphic::RasterGPU(_) => true,
		Graphic::RasterGPUList(list) => !list.is_empty(),
		Graphic::Graphic(item) => has_gpu_raster(item.element()),
		Graphic::GraphicList(list) => list.iter_element_values().any(has_gpu_raster),
		_ => false,
	}
}

/// Converts a `Graphic[]` into a `Raster[]` by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Graphic"), icon("NodeFlattenRaster"))]
fn flatten_raster(_: impl Ctx, content: List<Graphic>) -> Result<List<Raster<CPU>>, (List<Raster<CPU>>, FlattenRasterError)> {
	let has_gpu_rasters = content.iter_element_values().any(has_gpu_raster);
	let flattened = content.try_into_flattened_list();

	if has_gpu_rasters {
		let rasters = flattened.unwrap_or_else(|(rasters, _)| rasters);
		return Err((rasters, FlattenRasterError::GpuRasters));
	}
	flattened.map_err(|(rasters, conflict)| (rasters, conflict.into()))
}

/// Converts a `Graphic[]` into a `Color[]` by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("Graphic"), icon("NodeFlattenColor"))]
fn flatten_color(_: impl Ctx, content: List<Graphic>) -> Result<List<Color>, (List<Color>, AttributeTypeConflictError)> {
	content.try_into_flattened_list()
}

/// Converts a `Graphic[]` into a `Gradient[]` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("Graphic"), icon("NodeFlattenGradient"))]
fn flatten_gradient(_: impl Ctx, content: List<Graphic>) -> Result<List<Gradient>, (List<Gradient>, AttributeTypeConflictError)> {
	content.try_into_flattened_list()
}

/// Constructs a gradient from a `Color[]`, where each color becomes a gradient stop. A `position` attribute on the colors places their stops along the ramp and a `midpoint` attribute skews each transition, while colors carrying neither are distributed evenly across the 0 to 1 range.
#[node_macro::node(category("Gradient"), icon("NodeColorsToGradient"), name("Colors to Gradient"))]
fn colors_to_gradient(_: impl Ctx, colors: List<Graphic>) -> Result<Item<Gradient>, (Item<Gradient>, AttributeTypeConflictError)> {
	colors
		.try_into_flattened_list::<Color>()
		.map(|colors| Item::new_from_element(Gradient::from(colors)))
		.map_err(|(colors, conflict)| (Item::new_from_element(Gradient::from(colors)), conflict))
}

/// Unwraps a gradient into a `Color[]` of its stops, keeping any `position` and `midpoint` attributes that place them along the ramp. Attributes belonging to the gradient as a whole (like spread and interpolation), rather than its individual color stops, are not preserved.
#[node_macro::node(category("Gradient"), icon("NodeGradientToColors"), name("Gradient to Colors"))]
fn gradient_to_colors(_: impl Ctx, gradient: Item<Gradient>) -> List<Color> {
	gradient.into_element().into_color_list()
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::ATTR_OPACITY;

	fn list_of<T>(elements: impl IntoIterator<Item = T>) -> List<T> {
		elements.into_iter().map(Item::new_from_element).collect()
	}

	/// Stands in for the embedding adapter a compiled graph inserts ahead of a `List<Graphic>` connector.
	fn embed_colors(colors: List<Color>) -> List<Graphic> {
		colors.into_iter().map(|item| Item::new_from_element(Graphic::from(item))).collect()
	}

	fn elements<T: Clone>(list: &List<T>) -> Vec<T> {
		list.iter_element_values().cloned().collect()
	}

	#[test]
	fn sorts_elements_by_their_natural_order() {
		let list = list_of(["banana".to_string(), "apple".to_string(), "cherry".to_string()]);
		let sorted = sort((), list, List::<f64>::new(), Item::new_from_element(false)).unwrap();
		assert_eq!(elements(&sorted), ["apple", "banana", "cherry"]);
	}

	#[test]
	fn sorts_elements_in_reverse() {
		let list = list_of([3., 1., 2.]);
		let sorted = sort((), list, List::<f64>::new(), Item::new_from_element(true)).unwrap();
		assert_eq!(elements(&sorted), [3., 2., 1.]);
	}

	#[test]
	fn sort_order_keys_override_element_order() {
		let list = list_of(["apple".to_string(), "banana".to_string(), "cherry".to_string()]);
		let sorted = sort((), list, list_of([2., 0., 1.]), Item::new_from_element(false)).unwrap();
		assert_eq!(elements(&sorted), ["banana", "cherry", "apple"]);
	}

	#[test]
	fn short_sort_order_repeats_its_last_key() {
		let list = list_of(["a".to_string(), "b".to_string(), "c".to_string()]);
		let sorted = sort((), list, list_of([2., 1.]), Item::new_from_element(false)).unwrap();
		assert_eq!(elements(&sorted), ["b", "c", "a"]);
	}

	#[test]
	fn long_sort_order_ignores_its_extra_keys() {
		let list = list_of([1., 2.]);
		let sorted = sort((), list, list_of([3., 1., 0., 5.]), Item::new_from_element(false)).unwrap();
		assert_eq!(elements(&sorted), [2., 1.]);
	}

	#[test]
	fn text_sort_order_keys_order_items_alphabetically() {
		let list = list_of([1., 2., 3.]);
		let sorted = sort((), list, list_of(["c".to_string(), "a".to_string(), "b".to_string()]), Item::new_from_element(false)).unwrap();
		assert_eq!(elements(&sorted), [2., 3., 1.]);
	}

	#[test]
	fn unsortable_elements_need_sort_order_keys() {
		let list = list_of([DVec2::new(3., 3.), DVec2::new(1., 1.), DVec2::new(2., 2.)]);
		assert!(matches!(sort((), list.clone(), List::<f64>::new(), Item::new_from_element(false)), Err(SortError::NoNaturalOrder)));

		let sorted = sort((), list, list_of([2., 0., 1.]), Item::new_from_element(false)).unwrap();
		assert_eq!(elements(&sorted), [DVec2::new(1., 1.), DVec2::new(2., 2.), DVec2::new(3., 3.)]);

		let single = sort((), list_of([DVec2::ONE]), List::<f64>::new(), Item::new_from_element(false)).unwrap();
		assert_eq!(elements(&single), [DVec2::ONE], "a single item is already in order");
	}

	fn mirrored<T: BoundingBox + HasText + Send + Clone>(content: T, relative_to_bounds: ReferencePoint) -> Result<List<T>, MirrorError> {
		mirror((), content.into(), relative_to_bounds.into(), 0.0.into(), 0.0.into(), true.into())
	}

	#[test]
	fn mirror_without_bounds_reflects_across_the_origin() {
		let text = mirrored("text".to_string(), ReferencePoint::None).unwrap();
		assert_eq!(text.len(), 2);
		assert_eq!(text.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, 1), DAffine2::from_scale(DVec2::new(-1., 1.)));

		assert!(matches!(mirrored("text".to_string(), ReferencePoint::Center), Err(MirrorError::NoBounds)));
		assert!(matches!(mirrored(Color::RED, ReferencePoint::Center), Err(MirrorError::NoBounds)));
	}

	#[test]
	fn mirroring_empty_content_keeps_the_original_and_the_copy() {
		assert_eq!(mirrored(Vector::default(), ReferencePoint::Center).unwrap().len(), 2);
		assert_eq!(mirrored(String::new(), ReferencePoint::Center).unwrap().len(), 2);
	}

	fn with_values<U: Clone + Send + Sync + Default + std::fmt::Debug + PartialEq + CacheHash + 'static>(key: &str, values: Vec<U>) -> ListDyn {
		let mut list = List::new_from_elements(vec![(); values.len()]);
		list.set_attribute_values(key, values);
		ListDyn::from(list)
	}

	#[test]
	fn reading_a_missing_attribute_gives_each_item_its_implicit_default() {
		let plain = ListDyn::from(List::new_from_elements(vec![(), ()]));

		assert_eq!(elements(&read_attribute_number((), plain.clone(), Item::new_from_element(ATTR_OPACITY.to_string())).unwrap()), [1., 1.]);
		assert_eq!(elements(&read_attribute_number((), plain.clone(), Item::new_from_element("misspelled".to_string())).unwrap()), [0., 0.]);
		assert_eq!(
			elements(&read_attribute_transform((), plain, Item::new_from_element(ATTR_TRANSFORM.to_string())).unwrap()),
			[DAffine2::IDENTITY; 2]
		);
	}

	#[test]
	fn reading_an_attribute_as_another_type_fails() {
		let numbers = with_values("tag", vec![1_f64, 2.]);
		let error = read_attribute_bool((), numbers.clone(), Item::new_from_element("tag".to_string())).unwrap_err();
		assert_eq!(error.to_string(), "The \"tag\" attribute holds Number values, not Bool");
		assert_eq!(
			elements(&read_attribute_number((), with_values("tag", vec![3_i64]), Item::new_from_element("tag".to_string())).unwrap()),
			[3.]
		);
	}

	#[test]
	fn reading_a_list_attribute_joins_the_lists() {
		let vectors = with_values("shapes", vec![list_of([Vector::default(), Vector::default()]), list_of([Vector::default()])]);
		assert_eq!(read_attribute_vector((), vectors, Item::new_from_element("shapes".to_string())).unwrap().len(), 3);

		let texts = with_values("words", vec![list_of(["a".to_string()])]);
		let error = read_attribute_string((), texts, Item::new_from_element("words".to_string())).unwrap_err();
		assert_eq!(error.to_string(), "The \"words\" attribute holds String[] values, not String");
	}

	#[test]
	fn reading_list_attributes_with_differently_typed_attributes_fails() {
		let mut first = list_of([Vector::default()]);
		first.set_attribute("tag", 0, 5_f64);
		let mut second = list_of([Vector::default()]);
		second.set_attribute("tag", 0, "text".to_string());

		let vectors = with_values("shapes", vec![first, second]);
		let error = read_attribute_vector((), vectors, Item::new_from_element("shapes".to_string())).unwrap_err();
		assert_eq!(error.to_string(), "The \"tag\" attribute holds different types of values in the lists being combined");
	}

	#[test]
	fn extending_with_an_attribute_of_another_type_falls_back_with_its_name() {
		let mut base = list_of([1., 2.]);
		base.set_attribute("tag", 0, 5_f64);
		let mut new = list_of([3.]);
		new.set_attribute("tag", 0, "text".to_string());

		let Err((extended, conflict)) = extend((), base, new) else {
			panic!("expected the conflicting tag to be reported")
		};
		assert_eq!(elements(&extended), [1., 2., 3.]);
		assert_eq!(extended.attribute::<f64>("tag", 2), Some(&0.));
		assert_eq!(conflict.name, "tag");
	}

	#[test]
	fn shift_wraps_items_around() {
		let forward = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(1_i64), Item::new_from_element(true));
		assert_eq!(elements(&forward), [2., 3., 4., 1.]);

		let backward = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(-1_i64), Item::new_from_element(true));
		assert_eq!(elements(&backward), [4., 1., 2., 3.]);
	}

	#[test]
	fn shift_wraps_by_more_than_the_list_length() {
		let forward = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(5_i64), Item::new_from_element(true));
		assert_eq!(elements(&forward), [2., 3., 4., 1.], "a wrapping shift past the length keeps rotating");

		let backward = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(-5_i64), Item::new_from_element(true));
		assert_eq!(elements(&backward), [4., 1., 2., 3.]);
	}

	#[test]
	fn shift_without_wrapping_drops_items() {
		let dropped_front = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(1_i64), Item::new_from_element(false));
		assert_eq!(elements(&dropped_front), [2., 3., 4.]);

		let dropped_back = shift((), list_of([1., 2., 3., 4.]), Item::new_from_element(-1_i64), Item::new_from_element(false));
		assert_eq!(elements(&dropped_back), [1., 2., 3.]);
	}

	#[test]
	fn shuffle_is_deterministic_and_preserves_elements() {
		let original = [1., 2., 3., 4., 5., 6., 7., 8.];
		let first = shuffle((), list_of(original), Item::new_from_element(42_i64));
		let second = shuffle((), list_of(original), Item::new_from_element(42_i64));
		assert_eq!(elements(&first), elements(&second), "the same seed should always produce the same ordering");

		let mut recovered = elements(&first);
		recovered.sort_by(|a, b| a.partial_cmp(b).unwrap());
		assert_eq!(recovered, original, "shuffling should preserve all the elements");
	}

	#[test]
	fn number_sequence_generates_evenly_spaced_numbers() {
		let sequence = number_sequence((), (), Item::new_from_element(0.), Item::new_from_element(2.), Item::new_from_element(4_i64));
		assert_eq!(elements(&sequence), [0., 2., 4., 6.]);
	}

	#[test]
	fn indices_counts_each_item() {
		let from_zero = indices((), ListDyn::from(list_of(["a".to_string(), "b".to_string(), "c".to_string()])), Item::new_from_element(0_i64));
		assert_eq!(elements(&from_zero), [0., 1., 2.]);

		let from_one = indices((), ListDyn::from(list_of(["a".to_string(), "b".to_string(), "c".to_string()])), Item::new_from_element(1_i64));
		assert_eq!(elements(&from_one), [1., 2., 3.]);
	}

	#[test]
	fn slice_takes_the_portion_between_start_and_end() {
		let portion = slice((), list_of([1., 2., 3., 4., 5.]), Item::new_from_element(1_i64), Item::new_from_element(3_i64));
		assert_eq!(elements(&portion), [2., 3.]);
	}

	#[test]
	fn slice_resolves_negative_indices_from_the_end() {
		let portion = slice((), list_of([1., 2., 3., 4., 5.]), Item::new_from_element(-2_i64), Item::new_from_element(0_i64));
		assert_eq!(elements(&portion), [4., 5.], "an end of zero reaches through the end of the list");
	}

	#[test]
	fn slice_yields_nothing_when_start_reaches_end() {
		let portion = slice((), list_of([1., 2., 3., 4., 5.]), Item::new_from_element(3_i64), Item::new_from_element(3_i64));
		assert!(elements(&portion).is_empty());
	}

	#[test]
	fn gradient_to_colors_round_trips_the_stops_with_their_placement() {
		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);
		gradient.set_positions(&[0., 0.25, 1.]);
		gradient.set_midpoints(&[0.3, 0.5, 0.5]);

		let colors = gradient_to_colors((), Item::new_from_element(gradient.clone()));
		assert_eq!(elements(&colors), [Color::RED, Color::GREEN, Color::BLUE], "every stop should come out as its color");

		let restored = colors_to_gradient((), embed_colors(colors)).unwrap();
		assert_eq!(
			restored.element(),
			&gradient,
			"wrapping the stops back up should restore the uneven placement rather than redistributing them"
		);
	}

	#[test]
	fn gradient_to_colors_leaves_an_untouched_ramp_without_placement_attributes() {
		let gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);
		assert!(!gradient.has_position_attribute(), "even spacing is stored as the attribute's absence");

		let restored = colors_to_gradient((), embed_colors(gradient_to_colors((), Item::new_from_element(gradient.clone())))).unwrap();
		assert_eq!(restored.element(), &gradient, "a default ramp should round trip without gaining attributes it never had");
	}
}
