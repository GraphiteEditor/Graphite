use crate::application_io::{ImageTexture, TextureFrameTable};
use crate::instances::Instances;
use crate::raster::BlendMode;
use crate::raster::image::{Image, ImageFrameTable};
use crate::transform::TransformMut;
use crate::uuid::NodeId;
use crate::vector::{VectorData, VectorDataTable};
use crate::{CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};

use dyn_any::DynAny;

use glam::{DAffine2, IVec2};
use std::hash::Hash;

pub mod renderer;

#[derive(Copy, Clone, Debug, PartialEq, DynAny, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlphaBlending {
	pub opacity: f32,
	pub blend_mode: BlendMode,
}
impl Default for AlphaBlending {
	fn default() -> Self {
		Self::new()
	}
}
impl core::hash::Hash for AlphaBlending {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.opacity.to_bits().hash(state);
		self.blend_mode.hash(state);
	}
}
impl AlphaBlending {
	pub const fn new() -> Self {
		Self {
			opacity: 1.,
			blend_mode: BlendMode::Normal,
		}
	}
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_graphic_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<GraphicGroupTable, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Debug, PartialEq, DynAny, Default)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct OldGraphicGroup {
		elements: Vec<(GraphicElement, Option<NodeId>)>,
		transform: DAffine2,
		alpha_blending: AlphaBlending,
	}
	#[derive(Clone, Debug, PartialEq, DynAny, Default)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
			let mut graphic_group_table = GraphicGroupTable::empty();
			for (graphic_element, source_node_id) in old.elements {
				let last = graphic_group_table.push(graphic_element);
				*last.source_node_id = source_node_id;
				*last.transform = old.transform;
				*last.alpha_blending = old.alpha_blending;
			}
			graphic_group_table
		}
		EitherFormat::InstanceTable(value) => {
			// Try to deserialize as either table format
			if let Ok(old_table) = serde_json::from_value::<OldGraphicGroupTable>(value.clone()) {
				let mut graphic_group_table = GraphicGroupTable::empty();
				for instance in old_table.instances() {
					for (graphic_element, source_node_id) in &instance.instance.elements {
						let new_row = graphic_group_table.push(graphic_element.clone());
						*new_row.source_node_id = *source_node_id;
						*new_row.transform = *instance.transform;
						*new_row.alpha_blending = *instance.alpha_blending;
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
		Self::new(GraphicElement::RasterFrame(RasterFrame::ImageFrame(ImageFrameTable::new(image))))
	}
}
impl From<ImageFrameTable<Color>> for GraphicGroupTable {
	fn from(image_frame: ImageFrameTable<Color>) -> Self {
		Self::new(GraphicElement::RasterFrame(RasterFrame::ImageFrame(image_frame)))
	}
}
impl From<ImageTexture> for GraphicGroupTable {
	fn from(image_texture: ImageTexture) -> Self {
		Self::new(GraphicElement::RasterFrame(RasterFrame::TextureFrame(TextureFrameTable::new(image_texture))))
	}
}
impl From<TextureFrameTable> for GraphicGroupTable {
	fn from(texture_frame: TextureFrameTable) -> Self {
		Self::new(GraphicElement::RasterFrame(RasterFrame::TextureFrame(texture_frame)))
	}
}

/// The possible forms of graphical content held in a Vec by the `elements` field of [`GraphicElement`].
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphicElement {
	/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
	GraphicGroup(GraphicGroupTable),
	/// A vector shape, equivalent to the SVG <path> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path
	VectorData(VectorDataTable),
	RasterFrame(RasterFrame),
}

// TODO: Can this be removed? It doesn't necessarily make that much sense to have a default when, instead, the entire GraphicElement just shouldn't exist if there's no specific content to assign it.
impl Default for GraphicElement {
	fn default() -> Self {
		Self::VectorData(VectorDataTable::default())
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

	pub fn as_raster(&self) -> Option<&RasterFrame> {
		match self {
			GraphicElement::RasterFrame(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_mut(&mut self) -> Option<&mut RasterFrame> {
		match self {
			GraphicElement::RasterFrame(raster) => Some(raster),
			_ => None,
		}
	}
}

// TODO: Rename to Raster
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
pub enum RasterFrame {
	/// A CPU-based bitmap image with a finite position and extent, equivalent to the SVG <image> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/image
	// TODO: Rename to ImageTable
	ImageFrame(ImageFrameTable<Color>),
	/// A GPU texture with a finite position and extent
	// TODO: Rename to ImageTextureTable
	TextureFrame(TextureFrameTable),
}

impl<'de> serde::Deserialize<'de> for RasterFrame {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(RasterFrame::ImageFrame(ImageFrameTable::new(Image::deserialize(deserializer)?)))
	}
}

impl serde::Serialize for RasterFrame {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		match self {
			RasterFrame::ImageFrame(_) => self.serialize(serializer),
			RasterFrame::TextureFrame(_) => todo!(),
		}
	}
}

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_artboard_group<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<ArtboardGroupTable, D::Error> {
	use serde::Deserialize;

	#[derive(Clone, Default, Debug, Hash, PartialEq, DynAny)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
			let mut table = ArtboardGroupTable::empty();
			for (artboard, source_node_id) in artboard_group.artboards {
				let pushed = table.push(artboard);
				*pushed.source_node_id = source_node_id;
			}
			table
		}
		EitherFormat::ArtboardGroupTable(artboard_group_table) => artboard_group_table,
	})
}

pub type ArtboardGroupTable = Instances<Artboard>;

#[node_macro::node(category(""))]
async fn layer(_: impl Ctx, mut stack: GraphicGroupTable, element: GraphicElement, node_path: Vec<NodeId>) -> GraphicGroupTable {
	// Get the penultimate element of the node path, or None if the path is too short
	let pushed = stack.push(element);
	*pushed.source_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	stack
}

// TODO: Once we have nicely working spreadsheet tables, test this and make it nicely user-facing and move it from "Debug" to "General"
#[node_macro::node(category("Debug"))]
async fn concatenate<T: Clone>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
	 	VectorDataTable,
		ImageFrameTable<Color>,
	 	TextureFrameTable,
	)]
	from: Instances<T>,
	#[expose]
	#[implementations(
		GraphicGroupTable,
	 	VectorDataTable,
		ImageFrameTable<Color>,
	 	TextureFrameTable,
	)]
	mut to: Instances<T>,
) -> Instances<T> {
	for instance in from.instances() {
		to.push_instance(instance);
	}
	to
}

#[node_macro::node(category("Debug"))]
async fn to_element<Data: Into<GraphicElement> + 'n>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
	 	VectorDataTable,
		ImageFrameTable<Color>,
	 	TextureFrameTable,
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
		ImageFrameTable<Color>,
		TextureFrameTable,
	)]
	element: Data,
) -> GraphicGroupTable {
	element.into()
}

#[node_macro::node(category("General"))]
async fn flatten_group(_: impl Ctx, group: GraphicGroupTable, fully_flatten: bool) -> GraphicGroupTable {
	// TODO: Avoid mutable reference, instead return a new GraphicGroupTable?
	fn flatten_group(output_group_table: &mut GraphicGroupTable, current_group_table: GraphicGroupTable, fully_flatten: bool, recursion_depth: usize) {
		for current_instance in current_group_table.instances() {
			let current_element = current_instance.instance.clone();
			let reference = *current_instance.source_node_id;

			let recurse = fully_flatten || recursion_depth == 0;

			match current_element {
				// If we're allowed to recurse, flatten any GraphicGroups we encounter
				GraphicElement::GraphicGroup(mut current_element) if recurse => {
					// Apply the parent group's transform to all child elements
					for graphic_element in current_element.instances_mut() {
						*graphic_element.transform = *current_instance.transform * *graphic_element.transform;
					}

					flatten_group(output_group_table, current_element, fully_flatten, recursion_depth + 1);
				}
				// Handle any leaf elements we encounter, which can be either non-GraphicGroup elements or GraphicGroups that we don't want to flatten
				_ => {
					let pushed = output_group_table.push(current_element);
					*pushed.source_node_id = reference;
					// Apply the parent group's transform to the leaf element
					*pushed.transform = *current_instance.transform * *pushed.transform;
				}
			}
		}
	}

	let mut output = GraphicGroupTable::default();
	flatten_group(&mut output, group, fully_flatten, 0);

	output
}

#[node_macro::node(category(""))]
async fn to_artboard<Data: Into<GraphicGroupTable> + 'n>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> GraphicGroupTable,
		Context -> VectorDataTable,
		Context -> ImageFrameTable<Color>,
		Context -> TextureFrameTable,
	)]
	contents: impl Node<Context<'static>, Output = Data>,
	label: String,
	location: IVec2,
	dimensions: IVec2,
	background: Color,
	clip: bool,
) -> Artboard {
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
async fn append_artboard(_ctx: impl Ctx, mut artboards: ArtboardGroupTable, artboard: Artboard, node_path: Vec<NodeId>) -> ArtboardGroupTable {
	// Get the penultimate element of the node path, or None if the path is too short.
	// This is used to get the ID of the user-facing "Artboard" node (which encapsulates this internal "Append Artboard" node).
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();

	let pushed = artboards.push(artboard);
	*pushed.source_node_id = encapsulating_node_id;

	artboards
}

// TODO: Remove this one
impl From<Image<Color>> for GraphicElement {
	fn from(image_frame: Image<Color>) -> Self {
		GraphicElement::RasterFrame(RasterFrame::ImageFrame(ImageFrameTable::new(image_frame)))
	}
}
impl From<ImageFrameTable<Color>> for GraphicElement {
	fn from(image_frame: ImageFrameTable<Color>) -> Self {
		GraphicElement::RasterFrame(RasterFrame::ImageFrame(image_frame))
	}
}
// TODO: Remove this one
impl From<ImageTexture> for GraphicElement {
	fn from(texture: ImageTexture) -> Self {
		GraphicElement::RasterFrame(RasterFrame::TextureFrame(TextureFrameTable::new(texture)))
	}
}
impl From<TextureFrameTable> for GraphicElement {
	fn from(texture: TextureFrameTable) -> Self {
		GraphicElement::RasterFrame(RasterFrame::TextureFrame(texture))
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
