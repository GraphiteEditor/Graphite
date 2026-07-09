use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::list::{AttributeValueDyn, Item, List, ListDyn, NodeIdPath};
use core_types::registry::types::{Angle, SignedInteger};
use core_types::uuid::NodeId;
use core_types::{ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_TRANSFORM, AnyHash, BlendMode, CacheHash, CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{Artboard, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::gradient::{GradientSpreadMethod, GradientType};
use vector_types::{Gradient, GradientStop, ReferencePoint};

/// Returns the list with the element at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"))]
pub fn omit_element<T: graphic_types::graphic::OmitIndex + Clone + Default>(
	_: impl Ctx,
	/// The list of data.
	#[implementations(
		List<String>,
		List<Artboard>,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Color>,
		List<Gradient>,
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

/// Returns the item at the specified index in a `List`, keeping its attributes.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"))]
pub fn extract_element<T: Clone + Default + Send + Sync + 'static>(
	_: impl Ctx,
	/// The `List` of data to extract from.
	#[implementations(
		List<String>,
		List<f64>,
		List<u8>,
		List<NodeId>,
		List<Color>,
		List<Gradient>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Graphic>,
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

/// Keeps only the items whose corresponding mask value is true, pairing the list and mask by index.
/// Where the two differ in length, the trailing items of the longer one are dropped.
#[node_macro::node(category("General"))]
fn filter<T: Clone + Send + Sync + 'static>(
	_: impl Ctx,
	/// The `List` of data to filter.
	#[implementations(
		List<String>,
		List<f64>,
		List<Color>,
		List<Gradient>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Graphic>,
		List<Artboard>,
	)]
	list: List<T>,
	/// The mask deciding which items to keep: an item survives where its matching mask value is true.
	mask: List<bool>,
) -> List<T> {
	let mut result = List::new();

	for (index, &keep) in mask.iter_element_values().enumerate() {
		let Some(item) = list.clone_item(index) else { break };
		if keep {
			result.push(item);
		}
	}

	result
}

/// Reorders the items by their paired sort keys, smallest key first. Items and keys pair by index, and extra items or keys are dropped.
/// Items with equal keys keep their original relative order.
#[node_macro::node(category("General"))]
fn sort<T: Clone + Send + Sync + 'static>(
	_: impl Ctx,
	/// The `List` of data to reorder.
	#[implementations(
		List<String>,
		List<f64>,
		List<Color>,
		List<Gradient>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Raster<GPU>>,
		List<Graphic>,
		List<Artboard>,
	)]
	list: List<T>,
	/// A sort key for each item, paired by index. Items are ordered by ascending key.
	keys: List<f64>,
	/// Reverses the order so the largest key comes first.
	reverse: Item<bool>,
) -> List<T> {
	let reverse = reverse.into_element();

	let key_values: Vec<f64> = keys.iter_element_values().copied().collect();
	let count = list.len().min(key_values.len());

	let mut order: Vec<usize> = (0..count).collect();
	order.sort_by(|&a, &b| {
		let ordering = key_values[a].partial_cmp(&key_values[b]).unwrap_or(std::cmp::Ordering::Equal);
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
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Color>,
		List<Gradient>,
		List<String>,
	)]
	content: List<Item>,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<String>,
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
		String,
		Raster<CPU>,
		Color,
		Gradient,
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
	let item_transform: DAffine2 = content.attribute_cloned_or_default(ATTR_TRANSFORM);
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
	mirrored.set_attribute(ATTR_TRANSFORM, reflected_transform * item_transform);
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
		List<Artboard>,
		List<Graphic>,
		List<Vector>,
		List<Raster<CPU>>,
		List<Color>,
		List<Gradient>,
		List<f64>,
		List<bool>,
		List<String>,
		List<DAffine2>,
		List<BlendMode>,
		List<GradientType>,
		List<GradientSpreadMethod>,
	)]
	content: List<T>,
	/// The attribute name (key) to write or replace.
	name: Item<String>,
	/// The node that produces the attribute value for each item. Called once per item with the item's index in context.
	#[implementations(Context -> AttributeValueDyn)]
	value: impl Node<'n, Context<'static>, Output = AttributeValueDyn>,
) -> List<T> {
	let name = name.into_element();

	let mut content = content;
	for index in 0..content.len() {
		let row = content.clone_item(index).expect("index is within bounds");
		let owned_ctx = OwnedContextImpl::from(ctx.clone()).with_vararg(Box::new(row)).with_index(index);
		let v = value.eval(owned_ctx.into_context()).await;
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
		let Some(value) = content.attribute::<Vector>(&name, index) else { continue };
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
			.attribute::<f64>(&name, index)
			.copied()
			.or_else(|| content.attribute::<u64>(&name, index).map(|v| *v as f64))
			.or_else(|| content.attribute::<u32>(&name, index).map(|v| *v as f64));
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
		let Some(value) = content.attribute::<bool>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<String>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<DAffine2>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<Color>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<BlendMode>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<GradientType>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<GradientSpreadMethod>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<Gradient>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<Artboard>(&name, index) else { continue };
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
		let Some(value) = content.attribute::<Raster<CPU>>(&name, index) else { continue };
		result.push(Item::new_from_element(value.clone()));
	}
	result
}

/// Joins two `List`s of the same type, extending the base `List` with the items from the new `List`.
#[node_macro::node(category("General"))]
pub async fn extend<T: 'n + Send + Clone>(
	_: impl Ctx,
	/// The `List` whose items will appear at the start of the extended `List`.
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)]
	base: List<T>,
	/// The `List` whose items will appear at the end of the extended `List`.
	#[expose]
	#[implementations(List<Artboard>, List<Graphic>, List<Vector>, List<String>, List<Raster<CPU>>, List<Raster<GPU>>, List<Color>, List<Gradient>)]
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
		row.set_attribute(ATTR_EDITOR_LAYER_PATH, Item::new_from_element(layer_path.clone()));
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
		DAffine2,
		DVec2,
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
pub async fn flatten_graphic(_: impl Ctx, content: List<Graphic>, fully_flatten: bool) -> List<Graphic> {
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
		let item_0_transform: DAffine2 = output.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		if item_0_transform.matrix2.determinant().abs() > f64::EPSILON {
			let inverse = item_0_transform.inverse();
			for transform in graphic_list.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
				*transform = inverse * *transform;
			}
		}

		output.set_attribute(ATTR_EDITOR_MERGED_LAYERS, 0, graphic_list);
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
#[node_macro::node(category("Color"))]
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
