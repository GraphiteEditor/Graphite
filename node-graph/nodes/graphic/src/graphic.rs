use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::list::{AttributeDyn, AttributeValueDyn, Item, List, ListDyn};
use core_types::registry::types::{Angle, SignedInteger};
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_TRANSFORM, AnyHash, BlendMode, CacheHash, CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{Artboard, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::gradient::{GradientSpreadMethod, GradientType};
use vector_types::{GradientStop, GradientStops, ReferencePoint};

/// Returns the value at the specified index in the list.
/// If no value exists at that index, the type's default value is returned.
#[node_macro::node(category("General"))]
pub fn index_elements<T: graphic_types::graphic::AtIndex + Clone + Default>(
	_: impl Ctx,
	/// The list of data.
	#[implementations(
		Item<List<Artboard>>,
		Item<List<Graphic>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Raster<GPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
		Item<List<String>>,
		Item<List<f64>>,
		Item<List<u8>>,
		Item<List<NodeId>>,
	)]
	list: Item<T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: Item<SignedInteger>,
) -> Item<T::Output>
where
	T::Output: Clone + Default,
{
	let list = list.into_element();
	let index = index.into_element();
	let index = index as i32;

	Item::new_from_element(if index < 0 { list.at_index_from_end(-index as usize) } else { list.at_index(index as usize) }.unwrap_or_default())
}

/// Returns the list with the element at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"))]
pub fn omit_element<T: graphic_types::graphic::OmitIndex + Clone + Default>(
	_: impl Ctx,
	/// The list of data.
	#[implementations(
		Item<List<String>>,
		Item<List<Artboard>>,
		Item<List<Graphic>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Raster<GPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
	)]
	list: Item<T>,
	/// The index of the item to remove, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: Item<SignedInteger>,
) -> Item<T> {
	let list = list.into_element();
	let index = index.into_element();
	let index = index as i32;

	Item::new_from_element(if index < 0 {
		list.omit_index_from_end(index.unsigned_abs() as usize)
	} else {
		list.omit_index(index as usize)
	})
}

/// Returns the bare element (without the item's attributes) at the specified index in a `List`.
/// Use this when downstream nodes want just the inner value rather than a `List` containing a single item.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"))]
pub fn extract_element<T: Clone + Default + Send + Sync + 'static>(
	_: impl Ctx,
	/// The `List` of data to extract from.
	#[implementations(
		Item<List<String>>,
		Item<List<f64>>,
		Item<List<u8>>,
		Item<List<NodeId>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Graphic>>,
		Item<List<Artboard>>,
	)]
	list: Item<List<T>>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: Item<SignedInteger>,
) -> Item<T> {
	let list = list.into_element();
	let index = index.into_element();
	let len = list.len();
	let index = index as i32;
	let resolved = if index < 0 {
		let from_end = index.unsigned_abs() as usize;
		if from_end > len {
			return Item::new_from_element(T::default());
		}
		len - from_end
	} else {
		index as usize
	};
	Item::new_from_element(list.element(resolved).cloned().unwrap_or_default())
}

#[node_macro::node(category("General"))]
async fn map<T: AnyHash + Send + Sync + CacheHash>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Item<List<Graphic>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
	)]
	content: Item<List<T>>,
	#[implementations(
		Context -> Item<List<Graphic>>,
		Context -> Item<List<Vector>>,
		Context -> Item<List<Raster<CPU>>>,
		Context -> Item<List<Color>>,
		Context -> Item<List<GradientStops>>,
	)]
	mapped: impl Node<Context<'static>, Output = Item<List<T>>>,
) -> Item<List<T>> {
	let content = content.into_element();
	let mut rows = List::new();

	for (i, row) in content.into_iter().enumerate() {
		let owned_ctx = OwnedContextImpl::from(ctx.clone());
		let owned_ctx = owned_ctx.with_vararg(Box::new(List::new_from_item(row))).with_index(i);
		let list = mapped.eval(owned_ctx.into_context()).await.into_element();

		rows.extend(list);
	}

	Item::new_from_element(rows)
}

#[node_macro::node(category("General"))]
async fn mirror<T: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(
		Item<List<Graphic>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
	)]
	content: Item<List<T>>,
	#[default(ReferencePoint::Center)] relative_to_bounds: Item<ReferencePoint>,
	#[unit(" px")] offset: Item<f64>,
	#[range((-90., 90.))] angle: Item<Angle>,
	#[default(true)] keep_original: Item<bool>,
) -> Item<List<T>>
where
	List<T>: BoundingBox,
{
	let content = content.into_element();
	let relative_to_bounds = relative_to_bounds.into_element();
	let offset = offset.into_element();
	let angle = angle.into_element();
	let keep_original = keep_original.into_element();
	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let RenderBoundingBox::Rectangle(bounding_box) = content.bounding_box(DAffine2::IDENTITY, false) else {
		return Item::new_from_element(content);
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

	// Add original items depending on the keep_original flag
	if keep_original {
		for item in content.clone().into_iter() {
			result_list.push(item);
		}
	}

	// Create and add mirrored items
	for mut row in content.into_iter() {
		let current_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
		row.set_attribute(ATTR_TRANSFORM, reflected_transform * current_transform);
		result_list.push(row);
	}

	Item::new_from_element(result_list)
}

/// Returns the path identifying the subgraph (network) that contains this proto node — i.e. the input `node_path`
/// with its own trailing entry dropped. The terminating element of the returned path is the document node whose
/// encapsulated network we live in, so the path doubles as a unique reference to that node at any nesting depth.
/// Used as the value source for stamping the `editor:layer_path` attribute on each item of a layer's output, which lets
/// editor tools (e.g. selection, click target routing) trace data back to its owning layer regardless of whether
/// the layer is at the root document network or nested inside a custom subgraph.
#[node_macro::node(name("Path of Subgraph"), category(""))]
pub fn path_of_subgraph(_: impl Ctx, node_path: Item<List<NodeId>>) -> Item<List<NodeId>> {
	let node_path = node_path.into_element();
	let len = node_path.len();
	Item::new_from_element(node_path.into_iter().take(len.saturating_sub(1)).collect())
}

/// Sets a named attribute on the input `List`, computing one value per item via the value-producing input. That input
/// is evaluated once per item, with the item's index and the item itself (as a `List` containing only that item,
/// passed as a vararg) provided via context, so the upstream pipeline can return a different value per item that may
/// be derived from the item's own data. If the attribute already exists, its values are replaced; if not, it's added.
/// The value is type-erased into an `AttributeValueDyn` by an auto-inserted convert node, so this node only
/// monomorphizes over `T` instead of the cartesian product `(T, U)`.
#[node_macro::node(category("Attributes: Write"))]
async fn write_attribute<T: AnyHash + Clone + Send + Sync + CacheHash>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	/// The `List` to set the named attribute on (one value per item).
	#[implementations(
		Item<List<Artboard>>,
		Item<List<Graphic>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
		Item<List<f64>>,
		Item<List<bool>>,
		Item<List<String>>,
		Item<List<DAffine2>>,
		Item<List<BlendMode>>,
		Item<List<GradientType>>,
		Item<List<GradientSpreadMethod>>,
	)]
	content: Item<List<T>>,
	/// The attribute name (key) to write or replace.
	name: Item<String>,
	/// The node that produces the attribute value for each item. Called once per item with the item's index in context.
	#[implementations(Context -> Item<AttributeValueDyn>)]
	value: impl Node<'n, Context<'static>, Output = Item<AttributeValueDyn>>,
) -> Item<List<T>> {
	let mut content = content.into_element();
	let name = name.into_element();
	for index in 0..content.len() {
		let row = content.clone_item(index).expect("index is within bounds");
		let owned_ctx = OwnedContextImpl::from(ctx.clone()).with_vararg(Box::new(List::new_from_item(row))).with_index(index);
		let v = value.eval(owned_ctx.into_context()).await.into_element();
		content.set_attribute_value_dyn(&name, index, v);
	}
	Item::new_from_element(content)
}

/// Sets a named attribute on the primary list, with each value taken from the corresponding item's element in the source list (paired by index, wrapping if the source has fewer items).
/// The source is type-erased into an `AttributeDyn` by an auto-inserted convert node, so this node only monomorphizes over `T` instead of the cartesian product `(T, U)`.
#[node_macro::node(category("Attributes: Write"))]
fn attach_attribute<T: AnyHash + Clone + Send + Sync + CacheHash>(
	_: impl Ctx,
	/// The `List` to attach the new attribute to.
	#[implementations(
		Item<List<Artboard>>,
		Item<List<Graphic>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
		Item<List<f64>>,
		Item<List<bool>>,
		Item<List<String>>,
		Item<List<DAffine2>>,
		Item<List<BlendMode>>,
		Item<List<GradientType>>,
		Item<List<GradientSpreadMethod>>,
	)]
	content: Item<List<T>>,
	/// The source values to attach. Any `List<U>` wired here is type-erased via an auto-inserted convert.
	#[expose]
	source: Item<AttributeDyn>,
	/// The name to assign to the new destination attribute.
	name: Item<String>,
) -> Item<List<T>> {
	let mut content = content.into_element();
	let source = source.into_element();
	let name = name.into_element();
	if source.is_empty() {
		return Item::new_from_element(content);
	}
	content.set_attribute_dyn(name, source);
	Item::new_from_element(content)
}

/// Reads a named `Vector` attribute from the input list, outputting each value as an element of a new `List<Vector>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_vector(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<Vector>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Vector>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	Item::new_from_element(result)
}

/// Reads a named numeric attribute (`f64`, `u64`, or `u32`) from the input list, outputting each value as an element of a new `List<f64>`. Integer values are converted to `f64`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_number(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<f64>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let value = content
			.attribute::<f64>(&name, index)
			.copied()
			.or_else(|| content.attribute::<u64>(&name, index).map(|v| *v as f64))
			.or_else(|| content.attribute::<u32>(&name, index).map(|v| *v as f64));
		let Some(value) = value else { continue };
		result.push(Item::new_from_element(value));
	}
	Item::new_from_element(result)
}

/// Reads a named `bool` attribute from the input list, outputting each value as an element of a new `List<bool>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_bool(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<bool>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<bool>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	Item::new_from_element(result)
}

/// Reads a named `String` attribute from the input list, outputting each value as an element of a new `List<String>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_string(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<String>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<String>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	Item::new_from_element(result)
}

/// Reads a named `DAffine2` transform attribute from the input list, outputting each value as an element of a new `List<DAffine2>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_transform(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<DAffine2>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<DAffine2>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	Item::new_from_element(result)
}

/// Reads a named `Color` attribute from the input list, outputting each value as an element of a new `List<Color>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_color(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<Color>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Color>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	Item::new_from_element(result)
}

/// Reads a named `BlendMode` attribute from the input list, outputting each value as an element of a new `List<BlendMode>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_blend_mode(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<BlendMode>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<BlendMode>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	Item::new_from_element(result)
}

/// Reads a named `GradientType` attribute from the input list, outputting each value as an element of a new `List<GradientType>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_gradient_type(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<GradientType>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<GradientType>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	Item::new_from_element(result)
}

/// Reads a named `GradientSpreadMethod` attribute from the input list, outputting each value as an element of a new `List<GradientSpreadMethod>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_spread_method(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<GradientSpreadMethod>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<GradientSpreadMethod>(&name, index) else { continue };
		result.push(Item::new_from_element(*value));
	}
	Item::new_from_element(result)
}

/// Reads a named `GradientStops` attribute from the input list, outputting each value as an element of a new `List<GradientStops>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_gradient_stops(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<GradientStops>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<GradientStops>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	Item::new_from_element(result)
}

/// Reads a named `Artboard` attribute from the input list, outputting each value as an element of a new `List<Artboard>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_artboard(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<Artboard>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Artboard>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	Item::new_from_element(result)
}

/// Reads a named `Raster<CPU>` attribute from the input list, outputting each value as an element of a new `List<Raster<CPU>>`.
#[node_macro::node(category("Attributes: Read"))]
fn read_attribute_raster(
	_: impl Ctx,
	content: Item<ListDyn>,
	/// The attribute name (key) to read.
	name: Item<String>,
) -> Item<List<Raster<CPU>>> {
	let content = content.into_element();
	let name = name.into_element();
	let mut result = List::with_capacity(content.len());
	for index in 0..content.len() {
		let Some(value) = content.attribute::<Raster<CPU>>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	Item::new_from_element(result)
}

/// Joins two `List`s of the same type, extending the base `List` with the items from the new `List`.
#[node_macro::node(category("General"))]
pub async fn extend<T: 'n + Send + Clone>(
	_: impl Ctx,
	/// The `List` whose items will appear at the start of the extended `List`.
	#[implementations(Item<List<Artboard>>, Item<List<Graphic>>, Item<List<Vector>>, Item<List<Raster<CPU>>>, Item<List<Raster<GPU>>>, Item<List<Color>>, Item<List<GradientStops>>)]
	base: Item<List<T>>,
	/// The `List` whose items will appear at the end of the extended `List`.
	#[expose]
	#[implementations(Item<List<Artboard>>, Item<List<Graphic>>, Item<List<Vector>>, Item<List<Raster<CPU>>>, Item<List<Raster<GPU>>>, Item<List<Color>>, Item<List<GradientStops>>)]
	new: Item<List<T>>,
) -> Item<List<T>> {
	let base = base.into_element();
	let new = new.into_element();
	let mut base = base;
	base.extend(new);

	Item::new_from_element(base)
}

// TODO: Eventually remove this document upgrade code
/// Performs an obsolete function as part of a migration from an older document format.
/// Users are advised to delete this node and replace it with a new one.
#[node_macro::node(category(""))]
pub async fn legacy_layer_extend<T: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(Item<List<Artboard>>, Item<List<Graphic>>, Item<List<Vector>>, Item<List<Raster<CPU>>>, Item<List<Raster<GPU>>>, Item<List<Color>>, Item<List<GradientStops>>)] base: Item<List<T>>,
	#[expose]
	#[implementations(Item<List<Artboard>>, Item<List<Graphic>>, Item<List<Vector>>, Item<List<Raster<CPU>>>, Item<List<Raster<GPU>>>, Item<List<Color>>, Item<List<GradientStops>>)]
	new: Item<List<T>>,
	nested_node_path: Item<List<NodeId>>,
) -> Item<List<T>> {
	let base = base.into_element();
	let new = new.into_element();
	let nested_node_path = nested_node_path.into_element();
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

	Item::new_from_element(base)
}

/// Nests the input graphical content in a wrapper graphic. This essentially "groups" the input.
/// The inverse of this node is 'Flatten Graphic'.
#[node_macro::node(category("General"))]
pub async fn wrap_graphic<T: Into<Graphic> + 'n>(
	_: impl Ctx,
	#[implementations(
		Item<List<Graphic>>,
	 	Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
	 	Item<List<Raster<GPU>>>,
	 	Item<List<Color>>,
		Item<List<GradientStops>>,
		Item<DAffine2>,
	)]
	content: Item<T>,
) -> Item<List<Graphic>> {
	let content = content.into_element();
	Item::new_from_element(List::new_from_element(content.into()))
}

/// Converts a `List` of graphical content into a `List<Graphic>` by placing it into an element of a new wrapper `List<Graphic>`.
/// If it is already a `List<Graphic>`, it is not wrapped again. Use the 'Wrap Graphic' node if wrapping is always desired.
#[node_macro::node(category("General"))]
pub async fn to_graphic<T: IntoGraphicList + 'n>(
	_: impl Ctx,
	#[implementations(
		Item<List<Graphic>>,
		Item<List<Vector>>,
		Item<List<Raster<CPU>>>,
		Item<List<Raster<GPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
	)]
	content: Item<T>,
) -> Item<List<Graphic>> {
	let content = content.into_element();
	Item::new_from_element(content.into_graphic_list())
}

/// Removes a level of nesting from a `List<Graphic>`, or all nesting if "Fully Flatten" is enabled.
#[node_macro::node(category("General"))]
pub async fn flatten_graphic(_: impl Ctx, content: Item<List<Graphic>>, fully_flatten: Item<bool>) -> Item<List<Graphic>> {
	let content = content.into_element();
	let fully_flatten = fully_flatten.into_element();
	// TODO: Avoid mutable reference, instead return a new List<Graphic>?
	fn flatten_list(output_graphic_list: &mut List<Graphic>, current_graphic_list: List<Graphic>, fully_flatten: bool, recursion_depth: usize) {
		for index in 0..current_graphic_list.len() {
			let Some(current_element) = current_graphic_list.element(index) else { continue };
			let current_element = current_element.clone();
			let current_transform: DAffine2 = current_graphic_list.attribute_cloned_or_default(ATTR_TRANSFORM, index);

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any graphics we encounter
				Graphic::Graphic(mut current_element) if recurse => {
					// Apply the parent graphic's transform to all child elements
					for graphic_transform in current_element.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
						*graphic_transform = current_transform * *graphic_transform;
					}

					flatten_list(output_graphic_list, current_element, fully_flatten, recursion_depth + 1);
				}
				// Push any leaf elements we encounter: either `Graphic::Graphic(...)` values beyond the recursion depth, or non-`Graphic::Graphic` variants (e.g. `Graphic::Vector`, `Graphic::Raster*`, `Graphic::Color`, `Graphic::Gradient`)
				_ => {
					let attributes = current_graphic_list.clone_item_attributes(index);
					output_graphic_list.push(Item::from_parts(current_element, attributes));
				}
			}
		}
	}

	let mut output = List::new();
	flatten_list(&mut output, content, fully_flatten, 0);

	Item::new_from_element(output)
}

/// Converts a `List<Graphic>` into a `List<Vector>` by deeply flattening any vector content it contains, and discarding any non-vector content.
#[node_macro::node(category("Vector"))]
pub async fn flatten_vector<T: IntoGraphicList + 'n + Send + Clone>(_: impl Ctx, #[implementations(Item<List<Graphic>>, Item<List<Vector>>)] content: Item<T>) -> Item<List<Vector>> {
	let content = content.into_element();
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

		output.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, graphic_list);
	}

	Item::new_from_element(output)
}

/// Converts a `List<Graphic>` into a `List<Raster>` by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Raster"))]
pub async fn flatten_raster<T: IntoGraphicList + 'n + Send + Clone>(_: impl Ctx, #[implementations(Item<List<Graphic>>, Item<List<Raster<CPU>>>)] content: Item<T>) -> Item<List<Raster<CPU>>> {
	let content = content.into_element();
	Item::new_from_element(content.into_flattened_list())
}

/// Converts a `List<Graphic>` into a `List<Color>` by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("General"))]
pub async fn flatten_color<T: IntoGraphicList + 'n + Send + Clone>(_: impl Ctx, #[implementations(Item<List<Graphic>>, Item<List<Color>>)] content: Item<T>) -> Item<List<Color>> {
	let content = content.into_element();
	Item::new_from_element(content.into_flattened_list())
}

/// Converts a `List<Graphic>` into a `List<GradientStops>` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"))]
pub async fn flatten_gradient<T: IntoGraphicList + 'n + Send + Clone>(_: impl Ctx, #[implementations(Item<List<Graphic>>, Item<List<GradientStops>>)] content: Item<T>) -> Item<List<GradientStops>> {
	let content = content.into_element();
	Item::new_from_element(content.into_flattened_list())
}

/// Constructs a gradient from a `List<Color>`, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
#[node_macro::node(category("Color"))]
fn colors_to_gradient<T: IntoGraphicList + 'n + Send + Clone>(_: impl Ctx, #[implementations(Item<List<Graphic>>, Item<List<Color>>)] colors: Item<T>) -> Item<List<GradientStops>> {
	let colors = colors.into_element();
	let colors = colors.into_flattened_list::<Color>();
	let total_colors = colors.len();

	if total_colors == 0 {
		return Item::new_from_element(List::new_from_element(GradientStops::new(vec![
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
		])));
	}

	if let (1, Some(&single_color)) = (total_colors, colors.element(0)) {
		return Item::new_from_element(List::new_from_element(GradientStops::new(vec![
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
		])));
	}

	let colors = colors.into_iter().enumerate().map(|(index, row)| GradientStop {
		position: index as f64 / (total_colors - 1) as f64,
		midpoint: 0.5,
		color: row.into_element(),
	});
	Item::new_from_element(List::new_from_element(GradientStops::new(colors)))
}
