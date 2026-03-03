use core_types::Color;
use core_types::blending::AlphaBlending;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::ops::TableConvert;
use core_types::render_complexity::RenderComplexity;
use core_types::table::{Table, TableRow};
use core_types::uuid::NodeId;
use dyn_any::DynAny;
use glam::DAffine2;
use raster_types::{CPU, GPU, Raster};
use std::hash::Hash;
use vector_types::GradientStops;
// use vector_types::Vector;

pub type Vector = vector_types::Vector<Option<Table<Graphic>>>;

/// The possible forms of graphical content that can be rendered by the Render node into either an image or SVG syntax.
#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub enum Graphic {
	Graphic(Table<Graphic>),
	Vector(Table<Vector>),
	RasterCPU(Table<Raster<CPU>>),
	RasterGPU(Table<Raster<GPU>>),
	Color(Table<Color>),
	Gradient(Table<GradientStops>),
}

impl Default for Graphic {
	fn default() -> Self {
		Self::Graphic(Table::new())
	}
}

// Graphic
impl From<Table<Graphic>> for Graphic {
	fn from(graphic: Table<Graphic>) -> Self {
		Graphic::Graphic(graphic)
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

// Note: Table<Vector> -> Table<Graphic> conversion handled by blanket impl in gcore

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
// Note: Table conversions handled by blanket impl in gcore

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
// Note: Table conversions handled by blanket impl in gcore

// Color
impl From<Color> for Graphic {
	fn from(color: Color) -> Self {
		Graphic::Color(Table::new_from_element(color))
	}
}
impl From<Table<Color>> for Graphic {
	fn from(color: Table<Color>) -> Self {
		Graphic::Color(color)
	}
}
// Note: Table conversions handled by blanket impl in gcore
// Note: Table<Color> -> Option<Color> is in gcore (Color is defined there)

// GradientStops
impl From<GradientStops> for Graphic {
	fn from(gradient: GradientStops) -> Self {
		Graphic::Gradient(Table::new_from_element(gradient))
	}
}
impl From<Table<GradientStops>> for Graphic {
	fn from(gradient: Table<GradientStops>) -> Self {
		Graphic::Gradient(gradient)
	}
}

/// Deeply flattens a graphic table, collecting only elements matching a specific variant (extracted by `extract_variant`)
/// and discarding all other non-matching content. Recursion through `Graphic::Graphic` sub-tables composes transforms and opacity.
fn flatten_graphic_table<T>(content: Table<Graphic>, extract_variant: fn(Graphic) -> Option<Table<T>>) -> Table<T> {
	fn compose_alpha_blending(parent: AlphaBlending, child: AlphaBlending) -> AlphaBlending {
		AlphaBlending {
			blend_mode: child.blend_mode,
			opacity: parent.opacity * child.opacity,
			fill: child.fill,
			clip: child.clip,
		}
	}

	fn flatten_recursive<T>(output: &mut Table<T>, current_graphic_table: Table<Graphic>, extract_variant: fn(Graphic) -> Option<Table<T>>) {
		for current_graphic_row in current_graphic_table.into_iter() {
			let source_node_id = current_graphic_row.source_node_id;

			match current_graphic_row.element {
				// Recurse into nested graphic tables, composing the parent's transform onto each child
				Graphic::Graphic(mut sub_table) => {
					for graphic in sub_table.iter_mut() {
						*graphic.transform = current_graphic_row.transform * *graphic.transform;
						*graphic.alpha_blending = compose_alpha_blending(current_graphic_row.alpha_blending, *graphic.alpha_blending);
					}

					flatten_recursive(output, sub_table, extract_variant);
				}
				// Try to extract the target variant; if it matches, push its rows with composed transform and opacity
				other => {
					if let Some(typed_table) = extract_variant(other) {
						for row in typed_table.into_iter() {
							output.push(TableRow {
								element: row.element,
								transform: current_graphic_row.transform * row.transform,
								alpha_blending: compose_alpha_blending(current_graphic_row.alpha_blending, row.alpha_blending),
								source_node_id,
							});
						}
					}
				}
			}
		}
	}

	let mut output = Table::new();
	flatten_recursive(&mut output, content, extract_variant);
	output
}

/// Maps from a concrete element type to its corresponding `Graphic` enum variant,
/// enabling type-directed casting of typed tables from a `Graphic` value.
pub trait TryFromGraphic: Clone + Sized {
	fn try_from_graphic(graphic: Graphic) -> Option<Table<Self>>;
}

impl TryFromGraphic for Vector {
	fn try_from_graphic(graphic: Graphic) -> Option<Table<Self>> {
		if let Graphic::Vector(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Raster<CPU> {
	fn try_from_graphic(graphic: Graphic) -> Option<Table<Self>> {
		if let Graphic::RasterCPU(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Color {
	fn try_from_graphic(graphic: Graphic) -> Option<Table<Self>> {
		if let Graphic::Color(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for GradientStops {
	fn try_from_graphic(graphic: Graphic) -> Option<Table<Self>> {
		if let Graphic::Gradient(t) = graphic { Some(t) } else { None }
	}
}

// Local trait to convert types to Table<Graphic> (avoids orphan rule issues)
pub trait IntoGraphicTable {
	fn into_graphic_table(self) -> Table<Graphic>;

	/// Deeply flattens any content of type `T` within a graphic table, discarding all other content, and returning a flat table of only `T` elements.
	fn into_flattened_table<T: TryFromGraphic>(self) -> Table<T>
	where
		Self: std::marker::Sized,
	{
		flatten_graphic_table(self.into_graphic_table(), T::try_from_graphic)
	}
}

impl IntoGraphicTable for Table<Graphic> {
	fn into_graphic_table(self) -> Table<Graphic> {
		self
	}
}

impl IntoGraphicTable for Table<Vector> {
	fn into_graphic_table(self) -> Table<Graphic> {
		Table::new_from_element(Graphic::Vector(self))
	}
}

impl IntoGraphicTable for Table<Raster<CPU>> {
	fn into_graphic_table(self) -> Table<Graphic> {
		Table::new_from_element(Graphic::RasterCPU(self))
	}
}

impl IntoGraphicTable for Table<Raster<GPU>> {
	fn into_graphic_table(self) -> Table<Graphic> {
		Table::new_from_element(Graphic::RasterGPU(self))
	}
}

impl IntoGraphicTable for Table<Color> {
	fn into_graphic_table(self) -> Table<Graphic> {
		Table::new_from_element(Graphic::Color(self))
	}
}

impl IntoGraphicTable for Table<GradientStops> {
	fn into_graphic_table(self) -> Table<Graphic> {
		Table::new_from_element(Graphic::Gradient(self))
	}
}

impl IntoGraphicTable for DAffine2 {
	fn into_graphic_table(self) -> Table<Graphic> {
		Table::new_from_element(Graphic::default())
	}
}

// DAffine2
impl From<DAffine2> for Graphic {
	fn from(_: DAffine2) -> Self {
		Graphic::default()
	}
}
// Note: Table conversions handled by blanket impl in gcore

impl Graphic {
	pub fn as_graphic(&self) -> Option<&Table<Graphic>> {
		match self {
			Graphic::Graphic(graphic) => Some(graphic),
			_ => None,
		}
	}

	pub fn as_graphic_mut(&mut self) -> Option<&mut Table<Graphic>> {
		match self {
			Graphic::Graphic(graphic) => Some(graphic),
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
			Graphic::Vector(vector) => vector.iter().all(|row| row.alpha_blending.clip),
			Graphic::Graphic(graphic) => graphic.iter().all(|row| row.alpha_blending.clip),
			Graphic::RasterCPU(raster) => raster.iter().all(|row| row.alpha_blending.clip),
			Graphic::RasterGPU(raster) => raster.iter().all(|row| row.alpha_blending.clip),
			Graphic::Color(color) => color.iter().all(|row| row.alpha_blending.clip),
			Graphic::Gradient(gradient) => gradient.iter().all(|row| row.alpha_blending.clip),
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			Graphic::Vector(vector) => vector.iter().all(|row| {
				let style = &row.element.style;
				let alpha_blending = &row.alpha_blending;
				(alpha_blending.opacity > 1. - f32::EPSILON) && style.fill().is_opaque() && style.stroke().is_none_or(|stroke| !stroke.has_renderable_stroke())
			}),
			_ => false,
		}
	}
}

impl BoundingBox for Graphic {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::Vector(vector) => vector.bounding_box(transform, include_stroke),
			Graphic::RasterCPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::RasterGPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::Graphic(graphic) => graphic.bounding_box(transform, include_stroke),
			Graphic::Color(color) => color.bounding_box(transform, include_stroke),
			Graphic::Gradient(gradient) => gradient.bounding_box(transform, include_stroke),
		}
	}
}

impl TableConvert<Graphic> for Vector {
	fn convert_row(self) -> Graphic {
		Graphic::Vector(Table::new_from_element(self))
	}
}
impl TableConvert<Graphic> for Raster<CPU> {
	fn convert_row(self) -> Graphic {
		Graphic::RasterCPU(Table::new_from_element(self))
	}
}
impl TableConvert<Graphic> for Raster<GPU> {
	fn convert_row(self) -> Graphic {
		Graphic::RasterGPU(Table::new_from_element(self))
	}
}

impl RenderComplexity for Graphic {
	fn render_complexity(&self) -> usize {
		match self {
			Self::Graphic(table) => table.render_complexity(),
			Self::Vector(table) => table.render_complexity(),
			Self::RasterCPU(table) => table.render_complexity(),
			Self::RasterGPU(table) => table.render_complexity(),
			Self::Color(table) => table.render_complexity(),
			Self::Gradient(table) => table.render_complexity(),
		}
	}
}

// Node definitions moved to graphic-nodes crate

pub trait AtIndex {
	type Output;
	fn at_index(&self, index: usize) -> Option<Self::Output>;
	fn at_index_from_end(&self, index: usize) -> Option<Self::Output>;
}
impl<T: Clone> AtIndex for Vec<T> {
	type Output = T;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		self.get(index).cloned()
	}

	fn at_index_from_end(&self, index: usize) -> Option<Self::Output> {
		if index == 0 || index > self.len() { None } else { self.get(self.len() - index).cloned() }
	}
}
impl<T: Clone> AtIndex for Table<T> {
	type Output = Table<T>;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		let mut result_table = Self::default();
		if let Some(row) = self.iter().nth(index) {
			result_table.push(row.into_cloned());
			Some(result_table)
		} else {
			None
		}
	}

	fn at_index_from_end(&self, index: usize) -> Option<Self::Output> {
		let mut result_table = Self::default();
		if index == 0 || index > self.len() {
			None
		} else if let Some(row) = self.iter().nth(self.len() - index) {
			result_table.push(row.into_cloned());
			Some(result_table)
		} else {
			None
		}
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_graphic<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Graphic>, D::Error> {
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

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	pub struct OlderTable<T> {
		id: Vec<u64>,
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<T>,
	}

	#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
	pub struct OldTable<T> {
		id: Vec<u64>,
		#[serde(alias = "instances", alias = "instance")]
		element: Vec<T>,
		transform: Vec<DAffine2>,
		alpha_blending: Vec<AlphaBlending>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum GraphicFormat {
		OldGraphicGroup(OldGraphicGroup),
		OlderTableOldGraphicGroup(OlderTable<OldGraphicGroup>),
		OldTableOldGraphicGroup(OldTable<OldGraphicGroup>),
		OldTableGraphicGroup(OldTable<GraphicGroup>),
		Table(serde_json::Value),
	}

	Ok(match GraphicFormat::deserialize(deserializer)? {
		GraphicFormat::OldGraphicGroup(old) => {
			let mut graphic_table = Table::new();
			for (graphic, source_node_id) in old.elements {
				graphic_table.push(TableRow {
					element: graphic,
					transform: old.transform,
					alpha_blending: old.alpha_blending,
					source_node_id,
				});
			}
			graphic_table
		}
		GraphicFormat::OlderTableOldGraphicGroup(old) => old
			.element
			.into_iter()
			.flat_map(|element| {
				element.elements.into_iter().map(move |(graphic, source_node_id)| TableRow {
					element: graphic,
					transform: element.transform,
					alpha_blending: element.alpha_blending,
					source_node_id,
				})
			})
			.collect(),
		GraphicFormat::OldTableOldGraphicGroup(old) => old
			.element
			.into_iter()
			.flat_map(|element| {
				element.elements.into_iter().map(move |(graphic, source_node_id)| TableRow {
					element: graphic,
					transform: element.transform,
					alpha_blending: element.alpha_blending,
					source_node_id,
				})
			})
			.collect(),
		GraphicFormat::OldTableGraphicGroup(old) => old
			.element
			.into_iter()
			.flat_map(|element| {
				element.elements.into_iter().map(move |(graphic, source_node_id)| TableRow {
					element: graphic,
					transform: Default::default(),
					alpha_blending: Default::default(),
					source_node_id,
				})
			})
			.collect(),
		GraphicFormat::Table(value) => {
			// Try to deserialize as either table format
			if let Ok(old_table) = serde_json::from_value::<Table<GraphicGroup>>(value.clone()) {
				let mut graphic_table = Table::new();
				for row in old_table.iter() {
					for (graphic, source_node_id) in &row.element.elements {
						graphic_table.push(TableRow {
							element: graphic.clone(),
							transform: *row.transform,
							alpha_blending: *row.alpha_blending,
							source_node_id: *source_node_id,
						});
					}
				}
				graphic_table
			} else if let Ok(new_table) = serde_json::from_value::<Table<Graphic>>(value) {
				new_table
			} else {
				return Err(serde::de::Error::custom("Failed to deserialize Table<Graphic>"));
			}
		}
	})
}
