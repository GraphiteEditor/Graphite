use core_types::Color;
use core_types::{
	Ctx,
	blending::AlphaBlending,
	table::{Table, TableRow},
	uuid::NodeId,
};
use glam::{DAffine2, DVec2};
use graphic_types::{
	Artboard, Vector,
	graphic::{Graphic, IntoGraphicTable},
};
use raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;

/// Performs internal editor record-keeping that enables tools to target this network's layer.
/// This node associates the ID of the network's parent layer to every element of output data.
/// This technical detail may be ignored by users, and will be phased out in the future.
#[node_macro::node(category(""))]
pub async fn source_node_id<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(
		Table<Artboard>,
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	content: Table<I>,
	node_path: Vec<NodeId>,
) -> Table<I> {
	// Get the penultimate element of the node path, or None if the path is too short
	// This is used to get the ID of the user-facing parent layer node (whose network contains this internal node).
	let source_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	let mut content = content;
	for row in content.iter_mut() {
		*row.source_node_id = source_node_id;
	}

	content
}

/// Joins two tables of the same type, extending the base table with the rows of the new table.
#[node_macro::node(category("General"))]
pub async fn extend<I: 'n + Send + Clone>(
	_: impl Ctx,
	/// The table whose rows will appear at the start of the extended table.
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)]
	base: Table<I>,
	/// The table whose rows will appear at the end of the extended table.
	#[expose]
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)]
	new: Table<I>,
) -> Table<I> {
	let mut base = base;
	base.extend(new);

	base
}

// TODO: Eventually remove this document upgrade code
/// Performs an obsolete function as part of a migration from an older document format.
/// Users are advised to delete this node and replace it with a new one.
#[node_macro::node(category(""))]
pub async fn legacy_layer_extend<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)] base: Table<I>,
	#[expose]
	#[implementations(Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)]
	new: Table<I>,
	nested_node_path: Vec<NodeId>,
) -> Table<I> {
	// Get the penultimate element of the node path, or None if the path is too short
	// This is used to get the ID of the user-facing parent layer-style node (which encapsulates this internal node).
	let source_node_id = nested_node_path.get(nested_node_path.len().wrapping_sub(2)).copied();

	let mut base = base;
	for row in new.into_iter() {
		base.push(TableRow { source_node_id, ..row });
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
		for current_row in current_graphic_table.iter() {
			let current_element = current_row.element.clone();
			let reference = *current_row.source_node_id;

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any graphics we encounter
				Graphic::Graphic(mut current_element) if recurse => {
					// Apply the parent graphic's transform to all child elements
					for graphic in current_element.iter_mut() {
						*graphic.transform = *current_row.transform * *graphic.transform;
					}

					flatten_table(output_graphic_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Push any leaf Graphic elements we encounter, which can be either Graphic table elements beyond the recursion depth, or table elements other than Graphic tables
				_ => {
					output_graphic_table.push(TableRow {
						element: current_element,
						transform: *current_row.transform,
						alpha_blending: *current_row.alpha_blending,
						source_node_id: reference,
					});
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
pub async fn flatten_vector(_: impl Ctx, content: Table<Graphic>) -> Table<Vector> {
	// TODO: Avoid mutable reference, instead return a new Table<Graphic>?
	fn flatten_table(output_vector_table: &mut Table<Vector>, current_graphic_table: Table<Graphic>) {
		for current_graphic_row in current_graphic_table.iter() {
			let current_graphic = current_graphic_row.element.clone();
			let source_node_id = *current_graphic_row.source_node_id;

			match current_graphic {
				// If we're allowed to recurse, flatten any tables we encounter
				Graphic::Graphic(mut current_graphic_table) => {
					// Apply the parent graphic's transform to all child elements
					for graphic in current_graphic_table.iter_mut() {
						*graphic.transform = *current_graphic_row.transform * *graphic.transform;
					}

					flatten_table(output_vector_table, current_graphic_table);
				}
				// Push any leaf Vector elements we encounter
				Graphic::Vector(vector_table) => {
					for current_vector_row in vector_table.iter() {
						output_vector_table.push(TableRow {
							element: current_vector_row.element.clone(),
							transform: *current_graphic_row.transform * *current_vector_row.transform,
							alpha_blending: AlphaBlending {
								blend_mode: current_vector_row.alpha_blending.blend_mode,
								opacity: current_graphic_row.alpha_blending.opacity * current_vector_row.alpha_blending.opacity,
								fill: current_vector_row.alpha_blending.fill,
								clip: current_vector_row.alpha_blending.clip,
							},
							source_node_id,
						});
					}
				}
				_ => {}
			}
		}
	}

	let mut output = Table::new();
	flatten_table(&mut output, content);

	output
}

/// Returns the value at the specified index in the collection.
/// If no value exists at that index, the type's default value is returned.
#[node_macro::node(category("General"))]
pub fn index_elements<T: graphic_types::graphic::AtIndex + Clone + Default>(
	_: impl Ctx,
	/// The collection of data, such as a list or table.
	#[implementations(
		Vec<f64>,
		Vec<u32>,
		Vec<u64>,
		Vec<DVec2>,
		Vec<String>,
		Table<Artboard>,
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	collection: T,
	/// The index of the item to retrieve, starting from 0 for the first item.
	index: u32,
) -> T::Output
where
	T::Output: Clone + Default,
{
	collection.at_index(index as usize).unwrap_or_default()
}
