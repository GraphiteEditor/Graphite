use crate::blending::AlphaBlending;
use crate::bounds::BoundingBox;
use crate::math::quad::Quad;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::{Table, TableRow};
use crate::uuid::NodeId;
use crate::vector::Vector;
use crate::{Color, Ctx};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use std::hash::Hash;

/// The possible forms of graphical content that can be rendered by the Render node into either an image or SVG syntax.
#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub enum Graphic {
	Group(Table<Graphic>),
	Vector(Table<Vector>),
	RasterCPU(Table<Raster<CPU>>),
	RasterGPU(Table<Raster<GPU>>),
}

impl Default for Graphic {
	fn default() -> Self {
		Self::Group(Default::default())
	}
}

// Group
impl From<Table<Graphic>> for Graphic {
	fn from(group: Table<Graphic>) -> Self {
		Graphic::Group(group)
	}
}

// Vector
impl From<Vector> for Graphic {
	fn from(vector: Vector) -> Self {
		Graphic::Vector(Table::new_from_element(vector))
	}
}
impl From<Table<Vector>> for Graphic {
	fn from(vector: Table<Vector>) -> Self {
		Graphic::Vector(vector)
	}
}
impl From<Vector> for Table<Graphic> {
	fn from(vector: Vector) -> Self {
		Table::new_from_element(Graphic::Vector(Table::new_from_element(vector)))
	}
}
impl From<Table<Vector>> for Table<Graphic> {
	fn from(vector: Table<Vector>) -> Self {
		Table::new_from_element(Graphic::Vector(vector))
	}
}

// Raster<CPU>
impl From<Raster<CPU>> for Graphic {
	fn from(raster: Raster<CPU>) -> Self {
		Graphic::RasterCPU(Table::new_from_element(raster))
	}
}
impl From<Table<Raster<CPU>>> for Graphic {
	fn from(raster: Table<Raster<CPU>>) -> Self {
		Graphic::RasterCPU(raster)
	}
}
impl From<Raster<CPU>> for Table<Graphic> {
	fn from(raster: Raster<CPU>) -> Self {
		Table::new_from_element(Graphic::RasterCPU(Table::new_from_element(raster)))
	}
}
impl From<Table<Raster<CPU>>> for Table<Graphic> {
	fn from(raster: Table<Raster<CPU>>) -> Self {
		Table::new_from_element(Graphic::RasterCPU(raster))
	}
}

// Raster<GPU>
impl From<Raster<GPU>> for Graphic {
	fn from(raster: Raster<GPU>) -> Self {
		Graphic::RasterGPU(Table::new_from_element(raster))
	}
}
impl From<Table<Raster<GPU>>> for Graphic {
	fn from(raster: Table<Raster<GPU>>) -> Self {
		Graphic::RasterGPU(raster)
	}
}
impl From<Raster<GPU>> for Table<Graphic> {
	fn from(raster: Raster<GPU>) -> Self {
		Table::new_from_element(Graphic::RasterGPU(Table::new_from_element(raster)))
	}
}
impl From<Table<Raster<GPU>>> for Table<Graphic> {
	fn from(raster: Table<Raster<GPU>>) -> Self {
		Table::new_from_element(Graphic::RasterGPU(raster))
	}
}

// DAffine2
impl From<DAffine2> for Graphic {
	fn from(_: DAffine2) -> Self {
		Graphic::default()
	}
}
impl From<DAffine2> for Table<Graphic> {
	fn from(_: DAffine2) -> Self {
		Table::new()
	}
}

impl Graphic {
	pub fn as_group(&self) -> Option<&Table<Graphic>> {
		match self {
			Graphic::Group(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_group_mut(&mut self) -> Option<&mut Table<Graphic>> {
		match self {
			Graphic::Group(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_vector(&self) -> Option<&Table<Vector>> {
		match self {
			Graphic::Vector(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_vector_mut(&mut self) -> Option<&mut Table<Vector>> {
		match self {
			Graphic::Vector(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_raster(&self) -> Option<&Table<Raster<CPU>>> {
		match self {
			Graphic::RasterCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_mut(&mut self) -> Option<&mut Table<Raster<CPU>>> {
		match self {
			Graphic::RasterCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn had_clip_enabled(&self) -> bool {
		match self {
			Graphic::Vector(vector) => vector.iter_ref().all(|row| row.alpha_blending.clip),
			Graphic::Group(group) => group.iter_ref().all(|row| row.alpha_blending.clip),
			Graphic::RasterCPU(raster) => raster.iter_ref().all(|row| row.alpha_blending.clip),
			Graphic::RasterGPU(raster) => raster.iter_ref().all(|row| row.alpha_blending.clip),
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			Graphic::Vector(vector) => vector.iter_ref().all(|row| {
				let style = &row.element.style;
				let alpha_blending = &row.alpha_blending;
				(alpha_blending.opacity > 1. - f32::EPSILON) && style.fill().is_opaque() && style.stroke().is_none_or(|stroke| !stroke.has_renderable_stroke())
			}),
			_ => false,
		}
	}
}

impl BoundingBox for Graphic {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		match self {
			Graphic::Vector(vector) => vector.bounding_box(transform, include_stroke),
			Graphic::RasterCPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::RasterGPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::Group(group) => group.bounding_box(transform, include_stroke),
		}
	}
}

impl BoundingBox for Table<Graphic> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		self.iter_ref()
			.filter_map(|element| element.element.bounding_box(transform * *element.transform, include_stroke))
			.reduce(Quad::combine_bounds)
	}
}

#[node_macro::node(category(""))]
async fn layer<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>)] mut stack: Table<I>,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>)] element: I,
	node_path: Vec<NodeId>,
) -> Table<I> {
	// Get the penultimate element of the node path, or None if the path is too short
	let source_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	stack.push(TableRow {
		element,
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::default(),
		source_node_id,
	});

	stack
}

#[node_macro::node(category("Debug"))]
async fn to_element<Data: Into<Graphic> + 'n>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
	 	Table<Vector>,
		Table<Raster<CPU>>,
	 	Table<Raster<GPU>>,
		DAffine2,
	)]
	data: Data,
) -> Graphic {
	data.into()
}

#[node_macro::node(category("General"))]
async fn to_group<Data: Into<Table<Graphic>> + 'n>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
	)]
	element: Data,
) -> Table<Graphic> {
	element.into()
}

#[node_macro::node(category("General"))]
async fn flatten_group(_: impl Ctx, group: Table<Graphic>, fully_flatten: bool) -> Table<Graphic> {
	// TODO: Avoid mutable reference, instead return a new Table<Graphic>?
	fn flatten_group(output_group_table: &mut Table<Graphic>, current_group_table: Table<Graphic>, fully_flatten: bool, recursion_depth: usize) {
		for current_row in current_group_table.iter_ref() {
			let current_element = current_row.element.clone();
			let reference = *current_row.source_node_id;

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any groups we encounter
				Graphic::Group(mut current_element) if recurse => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.iter_mut() {
						*graphic_element.transform = *current_row.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Handle any leaf elements we encounter, which can be either non-group elements or groups that we don't want to flatten
				_ => {
					output_group_table.push(TableRow {
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
	flatten_group(&mut output, group, fully_flatten, 0);

	output
}

#[node_macro::node(category("Vector"))]
async fn flatten_vector(_: impl Ctx, group: Table<Graphic>) -> Table<Vector> {
	// TODO: Avoid mutable reference, instead return a new Table<Graphic>?
	fn flatten_group(output_group_table: &mut Table<Vector>, current_group_table: Table<Graphic>) {
		for current_graphic_element_row in current_group_table.iter_ref() {
			let current_element = current_graphic_element_row.element.clone();
			let reference = *current_graphic_element_row.source_node_id;

			match current_element {
				// If we're allowed to recurse, flatten any groups we encounter
				Graphic::Group(mut current_element) => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.iter_mut() {
						*graphic_element.transform = *current_graphic_element_row.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element);
				}
				// Handle any leaf elements we encounter, which can be either non-group elements or groups that we don't want to flatten
				Graphic::Vector(vector_table) => {
					for current_vector_row in vector_table.iter_ref() {
						output_group_table.push(TableRow {
							element: current_vector_row.element.clone(),
							transform: *current_graphic_element_row.transform * *current_vector_row.transform,
							alpha_blending: AlphaBlending {
								blend_mode: current_vector_row.alpha_blending.blend_mode,
								opacity: current_graphic_element_row.alpha_blending.opacity * current_vector_row.alpha_blending.opacity,
								fill: current_vector_row.alpha_blending.fill,
								clip: current_vector_row.alpha_blending.clip,
							},
							source_node_id: reference,
						});
					}
				}
				_ => {}
			}
		}
	}

	let mut output = Table::new();
	flatten_group(&mut output, group);

	output
}

/// Returns the value at the specified index in the collection.
/// If that index has no value, the type's default value is returned.
#[node_macro::node(category("General"))]
fn index<T: AtIndex + Clone + Default>(
	_: impl Ctx,
	/// The collection of data, such as a list or table.
	#[implementations(
		Vec<Color>,
		Vec<Option<Color>>,
		Vec<f64>, Vec<u64>,
		Vec<DVec2>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Graphic>,
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

pub trait AtIndex {
	type Output;
	fn at_index(&self, index: usize) -> Option<Self::Output>;
}
impl<T: Clone> AtIndex for Vec<T> {
	type Output = T;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		self.get(index).cloned()
	}
}
impl<T: Clone> AtIndex for Table<T> {
	type Output = Table<T>;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		let mut result_table = Self::default();
		if let Some(row) = self.iter_ref().nth(index) {
			result_table.push(row.into_cloned());
			Some(result_table)
		} else {
			None
		}
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Graphic>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Debug, PartialEq, DynAny, Default, serde::Serialize, serde::Deserialize)]
	pub struct OldGraphicGroup {
		elements: Vec<(Graphic, Option<NodeId>)>,
		transform: DAffine2,
		alpha_blending: AlphaBlending,
	}
	#[derive(Clone, Debug, PartialEq, DynAny, Default, serde::Serialize, serde::Deserialize)]
	pub struct GraphicGroup {
		elements: Vec<(Graphic, Option<NodeId>)>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum EitherFormat {
		OldGraphicGroup(OldGraphicGroup),
		Table(serde_json::Value),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::OldGraphicGroup(old) => {
			let mut group_table = Table::new();
			for (graphic_element, source_node_id) in old.elements {
				group_table.push(TableRow {
					element: graphic_element,
					transform: old.transform,
					alpha_blending: old.alpha_blending,
					source_node_id,
				});
			}
			group_table
		}
		EitherFormat::Table(value) => {
			// Try to deserialize as either table format
			if let Ok(old_table) = serde_json::from_value::<Table<GraphicGroup>>(value.clone()) {
				let mut group_table = Table::new();
				for row in old_table.iter_ref() {
					for (graphic_element, source_node_id) in &row.element.elements {
						group_table.push(TableRow {
							element: graphic_element.clone(),
							transform: *row.transform,
							alpha_blending: *row.alpha_blending,
							source_node_id: *source_node_id,
						});
					}
				}
				group_table
			} else if let Ok(new_table) = serde_json::from_value::<Table<Graphic>>(value) {
				new_table
			} else {
				return Err(serde::de::Error::custom("Failed to deserialize Table<Graphic>"));
			}
		}
	})
}
