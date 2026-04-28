use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::registry::types::{Angle, SignedInteger};
use core_types::table::{Table, TableRow};
use core_types::uuid::NodeId;
use core_types::{AnyHash, CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicTable};
use graphic_types::{Artboard, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::{GradientStop, GradientStops, ReferencePoint};

/// Returns the value at the specified index in the collection.
/// If no value exists at that index, the type's default value is returned.
#[node_macro::node(category("General"))]
pub fn index_elements<T: graphic_types::graphic::AtIndex + Clone + Default>(
	_: impl Ctx,
	/// The collection of data, such as a list or table.
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
	collection: T,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the collection, starting from -1 for the last item.
	index: SignedInteger,
) -> T::Output
where
	T::Output: Clone + Default,
{
	let index = index as i32;

	if index < 0 {
		collection.at_index_from_end(-index as usize)
	} else {
		collection.at_index(index as usize)
	}
	.unwrap_or_default()
}

/// Returns the collection with the element at the specified index removed.
/// If no value exists at that index, the collection is returned unchanged.
#[node_macro::node(category("General"))]
pub fn omit_element<T: graphic_types::graphic::OmitIndex + Clone + Default>(
	_: impl Ctx,
	/// The collection of data, such as a list or table.
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
	collection: T,
	/// The index of the item to remove, starting from 0 for the first item. Negative indices count backwards from the end of the collection, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	let index = index as i32;

	if index < 0 {
		collection.omit_index_from_end(-index as usize)
	} else {
		collection.omit_index(index as usize)
	}
}

/// Returns the bare element (without its row attributes) at the specified index in a table.
/// Use this when downstream nodes want just the inner value rather than a single-row table.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"))]
pub fn extract_element<T: Clone + Default + Send + Sync + 'static>(
	_: impl Ctx,
	/// The table of data to extract from.
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
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the collection, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	let len = table.len();
	let index = index as i32;
	let resolved = if index < 0 {
		let from_end = (-index) as usize;
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

	// Add original instance depending on the keep_original flag
	if keep_original {
		for instance in content.clone().into_iter() {
			result_table.push(instance);
		}
	}

	// Create and add mirrored instance
	for mut row in content.into_iter() {
		let current_transform: DAffine2 = row.attribute_cloned_or_default("transform");
		row.set_attribute("transform", reflected_transform * current_transform);
		result_table.push(row);
	}

	result_table
}

/// Returns the NodeId of the user-facing parent layer node that encapsulates this sub-network.
/// Used as the value source for stamping the `editor:layer` attribute on each row of a layer's output,
/// which lets editor tools (e.g. selection, click target routing) trace data back to its owning layer.
#[node_macro::node(category(""))]
pub fn parent_layer(_: impl Ctx, node_path: Table<NodeId>) -> Option<NodeId> {
	// Get the penultimate element of the node path, or None if the path is too short
	let index = node_path.len().wrapping_sub(2);
	node_path.element(index).copied()
}

/// Writes a per-row attribute column on the input table. The value-producing input is evaluated once per row,
/// with the row's element index and the row itself (as a single-row table vararg) passed via context, so the
/// upstream pipeline can return a different value per row that may be derived from the row's own data.
/// If the column already exists, its values are replaced; if not, the column is created.
#[node_macro::node(category("General"))]
async fn write_attribute<T: AnyHash + Clone + Send + Sync + core_types::CacheHash, U: Clone + Send + Sync + Default + std::fmt::Debug + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	/// The table whose rows will gain or replace the named attribute column.
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
	/// The attribute name (column key) to write or replace.
	name: String,
	/// The node that produces the per-row value. Called once per row with the row index in context.
	#[implementations(
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Option<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Option<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Option<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Option<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Option<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Option<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
		Context -> f64, Context -> u32, Context -> bool, Context -> String, Context -> Table<String>, Context -> DVec2, Context -> DAffine2, Context -> Option<NodeId>, Context -> Table<Color>, Context -> Table<GradientStops>,
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

/// Joins two tables of the same type, extending the base table with the rows of the new table.
#[node_macro::node(category("General"))]
pub async fn extend<T: 'n + Send + Clone>(
	_: impl Ctx,
	/// The table whose rows will appear at the start of the extended table.
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)]
	base: Table<T>,
	/// The table whose rows will appear at the end of the extended table.
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
		row.set_attribute("editor:layer", layer);
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

/// Converts a table of graphical content into a graphic table by placing it into an element of a new wrapper graphic table.
/// If it is already a graphic table, it is not wrapped again. Use the 'Wrap Graphic' node if wrapping is always desired.
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

/// Removes a level of nesting from a graphic table, or all nesting if "Fully Flatten" is enabled.
#[node_macro::node(category("General"))]
pub async fn flatten_graphic(_: impl Ctx, content: Table<Graphic>, fully_flatten: bool) -> Table<Graphic> {
	// TODO: Avoid mutable reference, instead return a new Table<Graphic>?
	fn flatten_table(output_graphic_table: &mut Table<Graphic>, current_graphic_table: Table<Graphic>, fully_flatten: bool, recursion_depth: usize) {
		for index in 0..current_graphic_table.len() {
			let Some(current_element) = current_graphic_table.element(index) else { continue };
			let current_element = current_element.clone();
			let current_transform: DAffine2 = current_graphic_table.attribute_cloned_or_default("transform", index);

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any graphics we encounter
				Graphic::Graphic(mut current_element) if recurse => {
					// Apply the parent graphic's transform to all child elements
					for graphic_transform in current_element.iter_attribute_values_mut_or_default::<DAffine2>("transform") {
						*graphic_transform = current_transform * *graphic_transform;
					}

					flatten_table(output_graphic_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Push any leaf Graphic elements we encounter, which can be either Graphic table elements beyond the recursion depth, or table elements other than Graphic tables
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

/// Converts a graphic table into a vector table by deeply flattening any vector content it contains, and discarding any non-vector content.
#[node_macro::node(category("Vector"))]
pub async fn flatten_vector<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Vector>)] content: T) -> Table<Vector> {
	content.into_flattened_table()
}

/// Converts a graphic table into a raster table by deeply flattening any raster content it contains, and discarding any non-raster content.
#[node_macro::node(category("Raster"))]
pub async fn flatten_raster<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Raster<CPU>>)] content: T) -> Table<Raster<CPU>> {
	content.into_flattened_table()
}

/// Converts a graphic table into a color table by deeply flattening any color content it contains, and discarding any non-color content.
#[node_macro::node(category("General"))]
pub async fn flatten_color<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Color>)] content: T) -> Table<Color> {
	content.into_flattened_table()
}

/// Converts a graphic table into a gradient table by deeply flattening any gradient content it contains, and discarding any non-gradient content.
#[node_macro::node(category("General"))]
pub async fn flatten_gradient<T: IntoGraphicTable + 'n + Send + Clone>(_: impl Ctx, #[implementations(Table<Graphic>, Table<GradientStops>)] content: T) -> Table<GradientStops> {
	content.into_flattened_table()
}

/// Constructs a gradient from a table of colors, where the colors are evenly distributed as gradient stops across the range from 0 to 1.
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
