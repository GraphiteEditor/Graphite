use crate::blending::AlphaBlending;
use crate::bounds::BoundingBox;
use crate::math::quad::Quad;
use crate::raster::image::Image;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::{Table, TableRow};
use crate::transform::TransformMut;
use crate::uuid::NodeId;
use crate::vector::VectorData;
use crate::{CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, IVec2};
use std::hash::Hash;

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_graphic_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<GraphicElement>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Debug, PartialEq, DynAny, Default, serde::Serialize, serde::Deserialize)]
	pub struct OldGraphicGroup {
		elements: Vec<(GraphicElement, Option<NodeId>)>,
		transform: DAffine2,
		alpha_blending: AlphaBlending,
	}
	#[derive(Clone, Debug, PartialEq, DynAny, Default, serde::Serialize, serde::Deserialize)]
	pub struct GraphicGroup {
		elements: Vec<(GraphicElement, Option<NodeId>)>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum EitherFormat {
		OldGraphicGroup(OldGraphicGroup),
		Table(serde_json::Value),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::OldGraphicGroup(old) => {
			let mut graphic_group_table = Table::new();
			for (graphic_element, source_node_id) in old.elements {
				graphic_group_table.push(TableRow {
					element: graphic_element,
					transform: old.transform,
					alpha_blending: old.alpha_blending,
					source_node_id,
				});
			}
			graphic_group_table
		}
		EitherFormat::Table(value) => {
			// Try to deserialize as either table format
			if let Ok(old_table) = serde_json::from_value::<Table<GraphicGroup>>(value.clone()) {
				let mut graphic_group_table = Table::new();
				for row in old_table.iter_ref() {
					for (graphic_element, source_node_id) in &row.element.elements {
						graphic_group_table.push(TableRow {
							element: graphic_element.clone(),
							transform: *row.transform,
							alpha_blending: *row.alpha_blending,
							source_node_id: *source_node_id,
						});
					}
				}
				graphic_group_table
			} else if let Ok(new_table) = serde_json::from_value::<Table<GraphicElement>>(value) {
				new_table
			} else {
				return Err(serde::de::Error::custom("Failed to deserialize Table<GraphicElement>"));
			}
		}
	})
}

impl From<VectorData> for Table<GraphicElement> {
	fn from(vector_data: VectorData) -> Self {
		Table::new_from_element(GraphicElement::VectorData(Table::new_from_element(vector_data)))
	}
}
impl From<Table<VectorData>> for Table<GraphicElement> {
	fn from(vector_data: Table<VectorData>) -> Self {
		Table::new_from_element(GraphicElement::VectorData(vector_data))
	}
}
impl From<Image<Color>> for Table<GraphicElement> {
	fn from(image: Image<Color>) -> Self {
		Table::new_from_element(GraphicElement::RasterDataCPU(Table::new_from_element(Raster::new_cpu(image))))
	}
}
impl From<Table<Raster<CPU>>> for Table<GraphicElement> {
	fn from(raster_data_table: Table<Raster<CPU>>) -> Self {
		Table::new_from_element(GraphicElement::RasterDataCPU(raster_data_table))
	}
}
impl From<Table<Raster<GPU>>> for Table<GraphicElement> {
	fn from(raster_data_table: Table<Raster<GPU>>) -> Self {
		Table::new_from_element(GraphicElement::RasterDataGPU(raster_data_table))
	}
}
impl From<DAffine2> for Table<GraphicElement> {
	fn from(_: DAffine2) -> Self {
		Table::new()
	}
}

/// The possible forms of graphical content held in a Vec by the `elements` field of [`GraphicElement`].
#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub enum GraphicElement {
	/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
	GraphicGroup(Table<GraphicElement>),
	/// A vector shape, equivalent to the SVG <path> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path
	VectorData(Table<VectorData>),
	RasterDataCPU(Table<Raster<CPU>>),
	RasterDataGPU(Table<Raster<GPU>>),
}

impl Default for GraphicElement {
	fn default() -> Self {
		Self::GraphicGroup(Default::default())
	}
}

impl From<DAffine2> for GraphicElement {
	fn from(_: DAffine2) -> Self {
		GraphicElement::default()
	}
}

impl GraphicElement {
	pub fn as_group(&self) -> Option<&Table<GraphicElement>> {
		match self {
			GraphicElement::GraphicGroup(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_group_mut(&mut self) -> Option<&mut Table<GraphicElement>> {
		match self {
			GraphicElement::GraphicGroup(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_vector_data(&self) -> Option<&Table<VectorData>> {
		match self {
			GraphicElement::VectorData(data) => Some(data),
			_ => None,
		}
	}

	pub fn as_vector_data_mut(&mut self) -> Option<&mut Table<VectorData>> {
		match self {
			GraphicElement::VectorData(data) => Some(data),
			_ => None,
		}
	}

	pub fn as_raster(&self) -> Option<&Table<Raster<CPU>>> {
		match self {
			GraphicElement::RasterDataCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_mut(&mut self) -> Option<&mut Table<Raster<CPU>>> {
		match self {
			GraphicElement::RasterDataCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn had_clip_enabled(&self) -> bool {
		match self {
			GraphicElement::VectorData(data) => data.iter_ref().all(|row| row.alpha_blending.clip),
			GraphicElement::GraphicGroup(data) => data.iter_ref().all(|row| row.alpha_blending.clip),
			GraphicElement::RasterDataCPU(data) => data.iter_ref().all(|row| row.alpha_blending.clip),
			GraphicElement::RasterDataGPU(data) => data.iter_ref().all(|row| row.alpha_blending.clip),
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			GraphicElement::VectorData(vector_data_table) => vector_data_table.iter_ref().all(|row| {
				let style = &row.element.style;
				let alpha_blending = &row.alpha_blending;
				(alpha_blending.opacity > 1. - f32::EPSILON) && style.fill().is_opaque() && style.stroke().is_none_or(|stroke| !stroke.has_renderable_stroke())
			}),
			_ => false,
		}
	}
}

impl BoundingBox for GraphicElement {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.bounding_box(transform, include_stroke),
			GraphicElement::RasterDataCPU(raster) => raster.bounding_box(transform, include_stroke),
			GraphicElement::RasterDataGPU(raster) => raster.bounding_box(transform, include_stroke),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.bounding_box(transform, include_stroke),
		}
	}
}

impl BoundingBox for Table<GraphicElement> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		self.iter_ref()
			.filter_map(|element| element.element.bounding_box(transform * *element.transform, include_stroke))
			.reduce(Quad::combine_bounds)
	}
}

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct Artboard {
	pub graphic_group: Table<GraphicElement>,
	pub label: String,
	pub location: IVec2,
	pub dimensions: IVec2,
	pub background: Color,
	pub clip: bool,
}

impl Default for Artboard {
	fn default() -> Self {
		Self::new(IVec2::ZERO, IVec2::new(1920, 1080))
	}
}

impl Artboard {
	pub fn new(location: IVec2, dimensions: IVec2) -> Self {
		Self {
			graphic_group: Table::new(),
			label: "Artboard".to_string(),
			location: location.min(location + dimensions),
			dimensions: dimensions.abs(),
			background: Color::WHITE,
			clip: false,
		}
	}
}

impl BoundingBox for Artboard {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		let artboard_bounds = (transform * Quad::from_box([self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()])).bounding_box();
		if self.clip {
			Some(artboard_bounds)
		} else {
			[self.graphic_group.bounding_box(transform, include_stroke), Some(artboard_bounds)]
				.into_iter()
				.flatten()
				.reduce(Quad::combine_bounds)
		}
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_artboard_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<Artboard>, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Default, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
	pub struct ArtboardGroup {
		pub artboards: Vec<(Artboard, Option<NodeId>)>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum EitherFormat {
		ArtboardGroup(ArtboardGroup),
		ArtboardGroupTable(Table<Artboard>),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::ArtboardGroup(artboard_group) => {
			let mut table = Table::new();
			for (artboard, source_node_id) in artboard_group.artboards {
				table.push(TableRow {
					element: artboard,
					transform: DAffine2::IDENTITY,
					alpha_blending: AlphaBlending::default(),
					source_node_id,
				});
			}
			table
		}
		EitherFormat::ArtboardGroupTable(artboard_group_table) => artboard_group_table,
	})
}

impl BoundingBox for Table<Artboard> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		self.iter_ref().filter_map(|row| row.element.bounding_box(transform, include_stroke)).reduce(Quad::combine_bounds)
	}
}

#[node_macro::node(category(""))]
async fn layer<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(Table<GraphicElement>, Table<VectorData>, Table<Raster<CPU>>, Table<Raster<GPU>>)] mut stack: Table<I>,
	#[implementations(GraphicElement, VectorData, Raster<CPU>, Raster<GPU>)] element: I,
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
async fn to_element<Data: Into<GraphicElement> + 'n>(
	_: impl Ctx,
	#[implementations(
		Table<GraphicElement>,
	 	Table<VectorData>,
		Table<Raster<CPU>>,
	 	Table<Raster<GPU>>,
		DAffine2,
	)]
	data: Data,
) -> GraphicElement {
	data.into()
}

#[node_macro::node(category("General"))]
async fn to_group<Data: Into<Table<GraphicElement>> + 'n>(
	_: impl Ctx,
	#[implementations(
		Table<GraphicElement>,
		Table<VectorData>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
	)]
	element: Data,
) -> Table<GraphicElement> {
	element.into()
}

#[node_macro::node(category("General"))]
async fn flatten_group(_: impl Ctx, group: Table<GraphicElement>, fully_flatten: bool) -> Table<GraphicElement> {
	// TODO: Avoid mutable reference, instead return a new Table<GraphicElement>?
	fn flatten_group(output_group_table: &mut Table<GraphicElement>, current_group_table: Table<GraphicElement>, fully_flatten: bool, recursion_depth: usize) {
		for current_row in current_group_table.iter_ref() {
			let current_element = current_row.element.clone();
			let reference = *current_row.source_node_id;

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any GraphicGroups we encounter
				GraphicElement::GraphicGroup(mut current_element) if recurse => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.iter_mut() {
						*graphic_element.transform = *current_row.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Handle any leaf elements we encounter, which can be either non-GraphicGroup elements or GraphicGroups that we don't want to flatten
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
async fn flatten_vector(_: impl Ctx, group: Table<GraphicElement>) -> Table<VectorData> {
	// TODO: Avoid mutable reference, instead return a new Table<GraphicElement>?
	fn flatten_group(output_group_table: &mut Table<VectorData>, current_group_table: Table<GraphicElement>) {
		for current_graphic_element_row in current_group_table.iter_ref() {
			let current_element = current_graphic_element_row.element.clone();
			let reference = *current_graphic_element_row.source_node_id;

			match current_element {
				// If we're allowed to recurse, flatten any GraphicGroups we encounter
				GraphicElement::GraphicGroup(mut current_element) => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.iter_mut() {
						*graphic_element.transform = *current_graphic_element_row.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element);
				}
				// Handle any leaf elements we encounter, which can be either non-GraphicGroup elements or GraphicGroups that we don't want to flatten
				GraphicElement::VectorData(vector_table) => {
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

#[node_macro::node(category(""))]
async fn to_artboard<Data: Into<Table<GraphicElement>> + 'n>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> Table<GraphicElement>,
		Context -> Table<VectorData>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> DAffine2,
	)]
	contents: impl Node<Context<'static>, Output = Data>,
	label: String,
	location: DVec2,
	dimensions: DVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	let location = location.as_ivec2();
	let dimensions = dimensions.as_ivec2().max(IVec2::ONE);

	let footprint = ctx.try_footprint().copied();
	let mut new_ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.translate(location.as_dvec2());
		new_ctx = new_ctx.with_footprint(footprint);
	}
	let graphic_group = contents.eval(new_ctx.into_context()).await;

	Artboard {
		graphic_group: graphic_group.into(),
		label,
		location: location.min(location + dimensions),
		dimensions: dimensions.abs(),
		background,
		clip,
	}
}

#[node_macro::node(category(""))]
pub async fn append_artboard(_ctx: impl Ctx, mut artboards: Table<Artboard>, artboard: Artboard, node_path: Vec<NodeId>) -> Table<Artboard> {
	// Get the penultimate element of the node path, or None if the path is too short.
	// This is used to get the ID of the user-facing "Artboard" node (which encapsulates this internal "Append Artboard" node).
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	artboards.push(TableRow {
		element: artboard,
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::default(),
		source_node_id: encapsulating_node_id,
	});

	artboards
}

// TODO: Remove this one
impl From<Image<Color>> for GraphicElement {
	fn from(raster_data: Image<Color>) -> Self {
		GraphicElement::RasterDataCPU(Table::new_from_element(Raster::new_cpu(raster_data)))
	}
}
impl From<Table<Raster<CPU>>> for GraphicElement {
	fn from(raster_data: Table<Raster<CPU>>) -> Self {
		GraphicElement::RasterDataCPU(raster_data)
	}
}
impl From<Table<Raster<GPU>>> for GraphicElement {
	fn from(raster_data: Table<Raster<GPU>>) -> Self {
		GraphicElement::RasterDataGPU(raster_data)
	}
}
impl From<Raster<CPU>> for GraphicElement {
	fn from(raster_data: Raster<CPU>) -> Self {
		GraphicElement::RasterDataCPU(Table::new_from_element(raster_data))
	}
}
impl From<Raster<GPU>> for GraphicElement {
	fn from(raster_data: Raster<GPU>) -> Self {
		GraphicElement::RasterDataGPU(Table::new_from_element(raster_data))
	}
}
// TODO: Remove this one
impl From<VectorData> for GraphicElement {
	fn from(vector_data: VectorData) -> Self {
		GraphicElement::VectorData(Table::new_from_element(vector_data))
	}
}
impl From<Table<VectorData>> for GraphicElement {
	fn from(vector_data: Table<VectorData>) -> Self {
		GraphicElement::VectorData(vector_data)
	}
}
impl From<Table<GraphicElement>> for GraphicElement {
	fn from(graphic_group: Table<GraphicElement>) -> Self {
		GraphicElement::GraphicGroup(graphic_group)
	}
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
		Table<VectorData>,
		Table<Raster<CPU>>,
		Table<GraphicElement>,
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
