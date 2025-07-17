use crate::blending::AlphaBlending;
use crate::bounds::BoundingBox;
use crate::instances::{Instance, Instances};
use crate::math::quad::Quad;
use crate::raster::image::Image;
use crate::raster_types::{CPU, GPU, Raster, RasterDataTable};
use crate::uuid::NodeId;
use crate::vector::{VectorData, VectorDataTable};
use crate::{ Color, Context, Ctx, ModifyDownstreamTransform};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, IVec2};
use std::hash::Hash;

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_graphic_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<GraphicGroupTable, D::Error> {
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
	pub type OldGraphicGroupTable = Instances<GraphicGroup>;

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum EitherFormat {
		OldGraphicGroup(OldGraphicGroup),
		InstanceTable(serde_json::Value),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::OldGraphicGroup(old) => {
			let mut graphic_group_table = GraphicGroupTable::default();
			for (graphic_element, source_node_id) in old.elements {
				graphic_group_table.push(Instance {
					instance: graphic_element,
					transform: old.transform,
					alpha_blending: old.alpha_blending,
					source_node_id,
				});
			}
			graphic_group_table
		}
		EitherFormat::InstanceTable(value) => {
			// Try to deserialize as either table format
			if let Ok(old_table) = serde_json::from_value::<OldGraphicGroupTable>(value.clone()) {
				let mut graphic_group_table = GraphicGroupTable::default();
				for instance in old_table.instance_ref_iter() {
					for (graphic_element, source_node_id) in &instance.instance.elements {
						graphic_group_table.push(Instance {
							instance: graphic_element.clone(),
							transform: *instance.transform,
							alpha_blending: *instance.alpha_blending,
							source_node_id: *source_node_id,
						});
					}
				}
				graphic_group_table
			} else if let Ok(new_table) = serde_json::from_value::<GraphicGroupTable>(value) {
				new_table
			} else {
				return Err(serde::de::Error::custom("Failed to deserialize GraphicGroupTable"));
			}
		}
	})
}

// TODO: Rename to GraphicElementTable
pub type GraphicGroupTable = Instances<GraphicElement>;

impl From<VectorData> for GraphicGroupTable {
	fn from(vector_data: VectorData) -> Self {
		Self::new(GraphicElement::VectorData(VectorDataTable::new(vector_data)))
	}
}
impl From<VectorDataTable> for GraphicGroupTable {
	fn from(vector_data: VectorDataTable) -> Self {
		Self::new(GraphicElement::VectorData(vector_data))
	}
}
impl From<Image<Color>> for GraphicGroupTable {
	fn from(image: Image<Color>) -> Self {
		Self::new(GraphicElement::RasterDataCPU(RasterDataTable::<CPU>::new(Raster::new_cpu(image))))
	}
}
impl From<RasterDataTable<CPU>> for GraphicGroupTable {
	fn from(raster_data_table: RasterDataTable<CPU>) -> Self {
		Self::new(GraphicElement::RasterDataCPU(raster_data_table))
	}
}
impl From<RasterDataTable<GPU>> for GraphicGroupTable {
	fn from(raster_data_table: RasterDataTable<GPU>) -> Self {
		Self::new(GraphicElement::RasterDataGPU(raster_data_table))
	}
}

/// The possible forms of graphical content held in a Vec by the `elements` field of [`GraphicElement`].
#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub enum GraphicElement {
	/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
	GraphicGroup(GraphicGroupTable),
	/// A vector shape, equivalent to the SVG <path> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path
	VectorData(VectorDataTable),
	RasterDataCPU(RasterDataTable<CPU>),
	RasterDataGPU(RasterDataTable<GPU>),
}

impl Default for GraphicElement {
	fn default() -> Self {
		Self::GraphicGroup(GraphicGroupTable::default())
	}
}

impl GraphicElement {
	pub fn as_group(&self) -> Option<&GraphicGroupTable> {
		match self {
			GraphicElement::GraphicGroup(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_group_mut(&mut self) -> Option<&mut GraphicGroupTable> {
		match self {
			GraphicElement::GraphicGroup(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_vector_data(&self) -> Option<&VectorDataTable> {
		match self {
			GraphicElement::VectorData(data) => Some(data),
			_ => None,
		}
	}

	pub fn as_vector_data_mut(&mut self) -> Option<&mut VectorDataTable> {
		match self {
			GraphicElement::VectorData(data) => Some(data),
			_ => None,
		}
	}

	pub fn as_raster(&self) -> Option<&RasterDataTable<CPU>> {
		match self {
			GraphicElement::RasterDataCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_mut(&mut self) -> Option<&mut RasterDataTable<CPU>> {
		match self {
			GraphicElement::RasterDataCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn had_clip_enabled(&self) -> bool {
		match self {
			GraphicElement::VectorData(data) => data.instance_ref_iter().all(|instance| instance.alpha_blending.clip),
			GraphicElement::GraphicGroup(data) => data.instance_ref_iter().all(|instance| instance.alpha_blending.clip),
			GraphicElement::RasterDataCPU(data) => data.instance_ref_iter().all(|instance| instance.alpha_blending.clip),
			GraphicElement::RasterDataGPU(data) => data.instance_ref_iter().all(|instance| instance.alpha_blending.clip),
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			GraphicElement::VectorData(vector_data_table) => vector_data_table.instance_ref_iter().all(|instance_data| {
				let style = &instance_data.instance.style;
				let alpha_blending = &instance_data.alpha_blending;
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

impl BoundingBox for GraphicGroupTable {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		self.instance_ref_iter()
			.filter_map(|element| element.instance.bounding_box(transform * *element.transform, include_stroke))
			.reduce(Quad::combine_bounds)
	}
}

impl<'de> serde::Deserialize<'de> for Raster<CPU> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(Raster::new_cpu(Image::deserialize(deserializer)?))
	}
}

impl serde::Serialize for Raster<CPU> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		self.data().serialize(serializer)
	}
}
impl<'de> serde::Deserialize<'de> for Raster<GPU> {
	fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		unimplemented!()
	}
}

impl serde::Serialize for Raster<GPU> {
	fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		unimplemented!()
	}
}

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
#[derive(Clone, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct Artboard {
	pub graphic_group: GraphicGroupTable,
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
			graphic_group: GraphicGroupTable::default(),
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
pub fn migrate_artboard_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<ArtboardGroupTable, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Default, Debug, Hash, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
	pub struct ArtboardGroup {
		pub artboards: Vec<(Artboard, Option<NodeId>)>,
	}

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum EitherFormat {
		ArtboardGroup(ArtboardGroup),
		ArtboardGroupTable(ArtboardGroupTable),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::ArtboardGroup(artboard_group) => {
			let mut table = ArtboardGroupTable::default();
			for (artboard, source_node_id) in artboard_group.artboards {
				table.push(Instance {
					instance: artboard,
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

pub type ArtboardGroupTable = Instances<Artboard>;

impl BoundingBox for ArtboardGroupTable {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> Option<[DVec2; 2]> {
		self.instance_ref_iter()
			.filter_map(|instance| instance.instance.bounding_box(transform, include_stroke))
			.reduce(Quad::combine_bounds)
	}
}

#[node_macro::node(category(""))]
async fn layer<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>, RasterDataTable<GPU>)] mut stack: Instances<I>,
	#[implementations(GraphicElement, VectorData, Raster<CPU>, Raster<GPU>)] element: I,
	node_path: Vec<NodeId>,
) -> Instances<I> {
	// Get the penultimate element of the node path, or None if the path is too short
	let source_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	stack.push(Instance {
		instance: element,
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
		GraphicGroupTable,
	 	VectorDataTable,
		RasterDataTable<CPU>,
	 	RasterDataTable<GPU>,
	)]
	data: Data,
) -> GraphicElement {
	data.into()
}

#[node_macro::node(category("General"))]
async fn to_group<Data: Into<GraphicGroupTable> + 'n>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
		RasterDataTable<GPU>,
	)]
	element: Data,
) -> GraphicGroupTable {
	element.into()
}

#[node_macro::node(category("General"))]
async fn flatten_group(_: impl Ctx, group: GraphicGroupTable, fully_flatten: bool) -> GraphicGroupTable {
	// TODO: Avoid mutable reference, instead return a new GraphicGroupTable?
	fn flatten_group(output_group_table: &mut GraphicGroupTable, current_group_table: GraphicGroupTable, fully_flatten: bool, recursion_depth: usize) {
		for current_instance in current_group_table.instance_ref_iter() {
			let current_element = current_instance.instance.clone();
			let reference = *current_instance.source_node_id;

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any GraphicGroups we encounter
				GraphicElement::GraphicGroup(mut current_element) if recurse => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.instance_mut_iter() {
						*graphic_element.transform = *current_instance.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Handle any leaf elements we encounter, which can be either non-GraphicGroup elements or GraphicGroups that we don't want to flatten
				_ => {
					output_group_table.push(Instance {
						instance: current_element,
						transform: *current_instance.transform,
						alpha_blending: *current_instance.alpha_blending,
						source_node_id: reference,
					});
				}
			}
		}
	}

	let mut output = GraphicGroupTable::default();
	flatten_group(&mut output, group, fully_flatten, 0);

	output
}

#[node_macro::node(category("Vector"))]
async fn flatten_vector(_: impl Ctx, group: GraphicGroupTable) -> VectorDataTable {
	// TODO: Avoid mutable reference, instead return a new GraphicGroupTable?
	fn flatten_group(output_group_table: &mut VectorDataTable, current_group_table: GraphicGroupTable) {
		for current_instance in current_group_table.instance_ref_iter() {
			let current_element = current_instance.instance.clone();
			let reference = *current_instance.source_node_id;

			match current_element {
				// If we're allowed to recurse, flatten any GraphicGroups we encounter
				GraphicElement::GraphicGroup(mut current_element) => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.instance_mut_iter() {
						*graphic_element.transform = *current_instance.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element);
				}
				// Handle any leaf elements we encounter, which can be either non-GraphicGroup elements or GraphicGroups that we don't want to flatten
				GraphicElement::VectorData(vector_instance) => {
					for current_element in vector_instance.instance_ref_iter() {
						output_group_table.push(Instance {
							instance: current_element.instance.clone(),
							transform: *current_instance.transform * *current_element.transform,
							alpha_blending: AlphaBlending {
								blend_mode: current_element.alpha_blending.blend_mode,
								opacity: current_instance.alpha_blending.opacity * current_element.alpha_blending.opacity,
								fill: current_element.alpha_blending.fill,
								clip: current_element.alpha_blending.clip,
							},
							source_node_id: reference,
						});
					}
				}
				_ => {}
			}
		}
	}

	let mut output = VectorDataTable::default();
	flatten_group(&mut output, group);

	output
}

#[node_macro::node(category(""))]
async fn to_artboard<Data: Into<GraphicGroupTable> + 'n>(
	ctx: impl Ctx + ModifyDownstreamTransform,
	#[implementations(
		Context -> GraphicGroupTable,
		Context -> VectorDataTable,
		Context -> RasterDataTable<CPU>,
		Context -> RasterDataTable<GPU>,
	)]
	contents: impl Node<Context<'static>, Output = Data>,
	label: String,
	location: IVec2,
	dimensions: IVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	let modified_ctx = ctx.apply_modification(&DAffine2::from_translation(location.as_dvec2()));
	let graphic_group = contents.eval(modified_ctx).await;

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
pub async fn append_artboard(_ctx: impl Ctx, mut artboards: ArtboardGroupTable, artboard: Artboard, node_path: Vec<NodeId>) -> ArtboardGroupTable {
	// Get the penultimate element of the node path, or None if the path is too short.
	// This is used to get the ID of the user-facing "Artboard" node (which encapsulates this internal "Append Artboard" node).
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	artboards.push(Instance {
		instance: artboard,
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::default(),
		source_node_id: encapsulating_node_id,
	});

	artboards
}

// TODO: Remove this one
impl From<Image<Color>> for GraphicElement {
	fn from(raster_data: Image<Color>) -> Self {
		GraphicElement::RasterDataCPU(RasterDataTable::<CPU>::new(Raster::new_cpu(raster_data)))
	}
}
impl From<RasterDataTable<CPU>> for GraphicElement {
	fn from(raster_data: RasterDataTable<CPU>) -> Self {
		GraphicElement::RasterDataCPU(raster_data)
	}
}
impl From<RasterDataTable<GPU>> for GraphicElement {
	fn from(raster_data: RasterDataTable<GPU>) -> Self {
		GraphicElement::RasterDataGPU(raster_data)
	}
}
impl From<Raster<CPU>> for GraphicElement {
	fn from(raster_data: Raster<CPU>) -> Self {
		GraphicElement::RasterDataCPU(RasterDataTable::new(raster_data))
	}
}
impl From<Raster<GPU>> for GraphicElement {
	fn from(raster_data: Raster<GPU>) -> Self {
		GraphicElement::RasterDataGPU(RasterDataTable::new(raster_data))
	}
}
// TODO: Remove this one
impl From<VectorData> for GraphicElement {
	fn from(vector_data: VectorData) -> Self {
		GraphicElement::VectorData(VectorDataTable::new(vector_data))
	}
}
impl From<VectorDataTable> for GraphicElement {
	fn from(vector_data: VectorDataTable) -> Self {
		GraphicElement::VectorData(vector_data)
	}
}
impl From<GraphicGroupTable> for GraphicElement {
	fn from(graphic_group: GraphicGroupTable) -> Self {
		GraphicElement::GraphicGroup(graphic_group)
	}
}

pub trait ToGraphicElement {
	fn to_graphic_element(&self) -> GraphicElement;
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
		VectorDataTable,
		RasterDataTable<CPU>,
		GraphicGroupTable,
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
impl<T: Clone> AtIndex for Instances<T> {
	type Output = Instances<T>;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		let mut result_table = Self::default();
		if let Some(row) = self.instance_ref_iter().nth(index) {
			result_table.push(row.to_instance_cloned());
			Some(result_table)
		} else {
			None
		}
	}
}
