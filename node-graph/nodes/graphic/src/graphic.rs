use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::registry::types::{Angle, SignedInteger};
use core_types::table::{Table, TableRow};
use core_types::uuid::NodeId;
use core_types::{AnyHash, CloneVarArgs, Color, Context, Ctx, EDITOR_LAYER_PATH, ExtractAll, OwnedContextImpl, TRANSFORM};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicTable};
use graphic_types::{Artboard, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::{GradientStop, GradientStops, ReferencePoint};

/// Returns the value at the specified index in the list.
/// If no value exists at that index, the type's default value is returned.
#[node_macro::node(category("General"))]
pub fn index_elements<T: graphic_types::graphic::AtIndex + Clone + Default>(
	_: impl Ctx,
	/// The list of data.
	#[implementations(
		Table<Artboard>,
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
		Table<String>,
		Table<f64>,
		Table<u8>,
		Table<NodeId>,
	)]
	list: T,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> T::Output
where
	T::Output: Clone + Default,
{
	let index = index as i32;

	if index < 0 { list.at_index_from_end(-index as usize) } else { list.at_index(index as usize) }.unwrap_or_default()
}

/// Returns the list with the element at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"))]
pub fn omit_element<T: graphic_types::graphic::OmitIndex + Clone + Default>(
	_: impl Ctx,
	/// The list of data.
	#[implementations(
		Table<String>,
		Table<Artboard>,
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	list: T,
	/// The index of the item to remove, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	let index = index as i32;

	if index < 0 {
		list.omit_index_from_end(index.unsigned_abs() as usize)
	} else {
		list.omit_index(index as usize)
	}
}

/// Returns the bare element (without the item's attributes) at the specified index in a `Table`.
/// Use this when downstream nodes want just the inner value rather than a `Table` containing a single item.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"))]
pub fn extract_element<T: Clone + Default + Send + Sync + 'static>(
	_: impl Ctx,
	/// The `Table` of data to extract from.
	#[implementations(
		Table<String>,
		Table<f64>,
		Table<u8>,
		Table<NodeId>,
		Table<Color>,
		Table<GradientStops>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Graphic>,
		Table<Artboard>,
	)]
	table: Table<T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	let len = table.len();
	let index = index as i32;
	let resolved = if index < 0 {
		let from_end = index.unsigned_abs() as usize;
		if from_end > len {
			return T::default();
		}
		len - from_end
	} else {
		index as usize
	};
	table.element(resolved).cloned().unwrap_or_default()
}

#[node_macro::node(category("General"))]
async fn map<Item: AnyHash + Send + Sync + core_types::CacheHash>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	content: Table<Item>,
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	mapped: impl Node<Context<'static>, Output = Table<Item>>,
) -> Table<Item> {
	let mut rows = Table::new();

	for (i, row) in content.into_iter().enumerate() {
		let owned_ctx = OwnedContextImpl::from(ctx.clone());
		let owned_ctx = owned_ctx.with_vararg(Box::new(Table::new_from_row(row))).with_index(i);
		let table = mapped.eval(owned_ctx.into_context()).await;

		rows.extend(table);
	}

	rows
}

#[node_macro::node(category("General"))]
async fn mirror<T: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	content: Table<T>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range((-90., 90.))] angle: Angle,
	#[default(true)] keep_original: bool,
) -> Table<T>
where
	Table<T>: BoundingBox,
{
	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let RenderBoundingBox::Rectangle(bounding_box) = content.bounding_box(DAffine2::IDENTITY, false) else {
		return content;
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

	let mut result_table = Table::new();

	// Add original items depending on the keep_original flag
	if keep_original {
		for item in content.clone().into_iter() {
			result_table.push(item);
		}
	}

	// Create and add mirrored items
	for mut row in content.into_iter() {
		let current_transform: DAffine2 = row.attribute_cloned_or_default(TRANSFORM);
		row.set_attribute(TRANSFORM, reflected_transform * current_transform);
		result_table.push(row);
	}

	result_table
}

/// Returns the path identifying the subgraph (network) that contains this proto node — i.e. the input `node_path`
/// with its own trailing entry dropped. The terminating element of the returned path is the document node whose
/// encapsulated network we live in, so the path doubles as a unique reference to that node at any nesting depth.
/// Used as the value source for stamping the `editor:layer_path` attribute on each item of a layer's output, which lets
/// editor tools (e.g. selection, click target routing) trace data back to its owning layer regardless of whether
/// the layer is at the root document network or nested inside a custom subgraph.
#[node_macro::node(name("Path of Subgraph"), category(""))]
pub fn path_of_subgraph(_: impl Ctx, node_path: Table<NodeId>) -> Table<NodeId> {
	let len = node_path.len();
	node_path.into_iter().take(len.saturating_sub(1)).collect()
}

/// Writes a named attribute on each item of the input `Table`. The value-producing input is evaluated once per item,
/// with the item's index and the item itself (as a `Table` containing only that item, passed as a vararg) provided via
/// context, so the upstream pipeline can return a different value per item that may be derived from the item's own data.
/// If the attribute already exists, its values are replaced; if not, the attribute is added.
#[node_macro::node(category("General"))]
async fn write_attribute<T: AnyHash + Clone + Send + Sync + core_types::CacheHash, U: Clone + Send + Sync + Default + std::fmt::Debug + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	/// The `Table` whose items will gain or have replaced the named attribute.
	#[implementations(
		Table<Artboard>, Table<Artboard>, Table<Artboard>, Table<Artboard>, Table<Artboard>, Table<Artboard>, Table<Artboard>, Table<Artboard>, Table<Artboard>, Table<Artboard>,
		Table<Graphic>, Table<Graphic>, Table<Graphic>, Table<Graphic>, Table<Graphic>, Table<Graphic>, Table<Graphic>, Table<Graphic>, Table<Graphic>, Table<Graphic>,
		Table<Vector>, Table<Vector>, Table<Vector>, Table<Vector>, Table<Vector>, Table<Vector>, Table<Vector>, Table<Vector>, Table<Vector>, Table<Vector>,
		Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>, Table<Raster<CPU>>,
		Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>, Table<Raster<GPU>>,
		Table<Color>, Table<Color>, Table<Color>, Table<Color>, Table<Color>, Table<Color>, Table<Color>, Table<Color>, Table<Color>, Table<Color>,
		Table<GradientStops>, Table<GradientStops>, Table<GradientStops>, Table<GradientStops>, Table<GradientStops>, Table<GradientStops>, Table<GradientStops>, Table<GradientStops>, Table<GradientStops>, Table<GradientStops>,
	)]
	mut content: Table<T>,
	/// The attribute name (key) to write or replace.
	name: String,
	/// The node that produces the attribute value for each item. Called once per item with the item's index in context.
	#[implementations(
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Table<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Table<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Table<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Table<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Table<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Table<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Table<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
	)]
	value: impl Node<'n, Context<'static>, Output = U>,
) -> Table<T> {
	for index in 0..content.len() {
		let row = content.clone_row(index).expect("index is within bounds");
		let owned_ctx = OwnedContextImpl::from(ctx.clone()).with_vararg(Box::new(Table::new_from_row(row))).with_index(index);
		let v = value.eval(owned_ctx.into_context()).await;
		content.set_attribute(&name, index, v);
	}
	content
}

/// Joins two `Table`s of the same type, extending the base `Table` with the items from the new `Table`.
#[node_macro::node(category("General"))]
pub async fn extend<T: 'n + Send + Clone>(
	_: impl Ctx,
	/// The `Table` whose items will appear at the start of the extended `Table`.
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)]
	base: Table<T>,
	/// The `Table` whose items will appear at the end of the extended `Table`.
	#[expose]
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)]
	new: Table<T>,
) -> Table<T> {
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
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)] base: Table<T>,
	#[expose]
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)]
	new: Table<T>,
	nested_node_path: Table<NodeId>,
) -> Table<T> {
	// Get the penultimate element of the node path, or None if the path is too short
	// This is used to get the ID of the user-facing parent layer-style node (which encapsulates this internal node).
	let layer = {
		let index = nested_node_path.len().wrapping_sub(2);
		nested_node_path.element(index).copied()
	};

	let mut base = base;
	for mut row in new.into_iter() {
		row.set_attribute(EDITOR_LAYER_PATH, layer);
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
		Table<Graphic>,
	 	Table<Vector>,
		Table<Raster<CPU>>,
	 	Table<Raster<GPU>>,
	 	Table<Color>,
		Table<GradientStops>,
		DAffine2,
	)]
	content: T,
) -> Table<Graphic> {
	Table::new_from_element(content.into())
}

/// Converts a `Table` of graphical content into a `Table<Graphic>` by placing it into an element of a new wrapper `Table<Graphic>`.
/// If it is already a `Table<Graphic>`, it is not wrapped again. Use the 'Wrap Graphic' node if wrapping is always desired.
#[node_macro::node(category("General"))]
pub async fn to_graphic<T: IntoGraphicTable + 'n>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	content: T,
) -> Table<Graphic> {
	content.into_graphic_table()
}

/// Removes a level of nesting from a `Table<Graphic>`, or all nesting if "Fully Flatten" is enabled.
#[node_macro::node(category("General"))]
pub async fn flatten_graphic(_: impl Ctx, content: Table<Graphic>, fully_flatten: bool) -> Table<Graphic> {
	// TODO: Avoid mutable reference, instead return a new Table<Graphic>?
	fn flatten_table(output_graphic_table: &mut Table<Graphic>, current_graphic_table: Table<Graphic>, fully_flatten: bool, recursion_depth: usize) {
		for index in 0..current_graphic_table.len() {
			let Some(current_element) = current_graphic_table.element(index) else { continue };
			let current_element = current_element.clone();
			let current_transform: DAffine2 = current_graphic_table.attribute_cloned_or_default(TRANSFORM, index);

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any graphics we encounter
				Graphic::Graphic(mut current_element) if recurse => {
					// Apply the parent graphic's transform to all child elements
					for graphic_transform in current_element.iter_attribute_values_mut_or_default::<DAffine2>(TRANSFORM) {
						*graphic_transform = current_transform * *graphic_transform;
					}

					flatten_table(output_graphic_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Push any leaf elements we encounter: either `Graphic::Graphic(...)` values beyond the recursion depth, or non-`Graphic::Graphic` variants (e.g. `Graphic::Vector`, `Graphic::Raster*`, `Graphic::Color`, `Graphic::Gradient`)
				_ => {
					let attributes = current_graphic_table.clone_row_attributes(index);
					output_graphic_table.push(TableRow::from_parts(current_element, attributes));
				}
			}
		}
	}

	let mut output = Table::new();
	flatten_table(&mut output, content, fully_flatten, 0);

	output
}

/// Converts a `Table<Graphic>` into a `Table<Vector>` by deeply flattening any vector content it contains, and discarding any non-vector content.
#[node_macro::node(category("Vector"))]
pub async fn flatten_vector<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Vector>)] content: T) -> Table<Vector> {
	content.into_flattened_table()
}

/// Converts a `Table<Graphic>` into a `Table<Raster>` by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Raster"))]
pub async fn flatten_raster<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Raster<CPU>>)] content: T) -> Table<Raster<CPU>> {
	content.into_flattened_table()
}

/// Converts a `Table<Graphic>` into a `Table<Color>` by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("General"))]
pub async fn flatten_color<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Color>)] content: T) -> Table<Color> {
	content.into_flattened_table()
}

/// Converts a `Table<Graphic>` into a `Table<GradientStops>` by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"))]
pub async fn flatten_gradient<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<GradientStops>)] content: T) -> Table<GradientStops> {
	content.into_flattened_table()
}

/// Constructs a gradient from a `Table<Color>`, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
#[node_macro::node(category("Color"))]
fn colors_to_gradient<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Color>)] colors: T) -> Table<GradientStops> {
	let colors = colors.into_flattened_table::<Color>();
	let total_colors = colors.len();

	if total_colors == 0 {
		return Table::new_from_element(GradientStops::new(vec![
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
		return Table::new_from_element(GradientStops::new(vec![
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
	Table::new_from_element(GradientStops::new(colors))
}
