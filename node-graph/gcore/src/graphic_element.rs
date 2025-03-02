use crate::application_io::{TextureFrame, TextureFrameTable};
use crate::instances::Instances;
use crate::raster::image::{ImageFrame, ImageFrameTable};
use crate::raster::BlendMode;
use crate::transform::{Transform, TransformMut};
use crate::uuid::NodeId;
use crate::vector::{VectorData, VectorDataTable};
use crate::{CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};

use dyn_any::DynAny;

use core::ops::{Deref, DerefMut};
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

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum EitherFormat {
		GraphicGroup(GraphicGroup),
		GraphicGroupTable(GraphicGroupTable),
	}

	Ok(match EitherFormat::deserialize(deserializer)? {
		EitherFormat::GraphicGroup(graphic_group) => GraphicGroupTable::new(graphic_group),
		EitherFormat::GraphicGroupTable(graphic_group_table) => graphic_group_table,
	})
}

pub type GraphicGroupTable = Instances<GraphicGroup>;

/// A list of [`GraphicElement`]s
#[derive(Clone, Debug, PartialEq, DynAny, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphicGroup {
	elements: Vec<(GraphicElement, Option<NodeId>)>,
	pub transform: DAffine2,
	pub alpha_blending: AlphaBlending,
}

impl core::hash::Hash for GraphicGroup {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.transform.to_cols_array().iter().for_each(|element| element.to_bits().hash(state));
		self.elements.hash(state);
		self.alpha_blending.hash(state);
	}
}

impl GraphicGroup {
	pub fn new(elements: Vec<GraphicElement>) -> Self {
		Self {
			elements: elements.into_iter().map(|element| (element, None)).collect(),
			transform: DAffine2::IDENTITY,
			alpha_blending: AlphaBlending::new(),
		}
	}
}

impl From<GraphicGroup> for GraphicGroupTable {
	fn from(graphic_group: GraphicGroup) -> Self {
		Self::new(graphic_group)
	}
}
impl From<VectorData> for GraphicGroupTable {
	fn from(vector_data: VectorData) -> Self {
		Self::new(GraphicGroup::new(vec![GraphicElement::VectorData(VectorDataTable::new(vector_data))]))
	}
}
impl From<VectorDataTable> for GraphicGroupTable {
	fn from(vector_data: VectorDataTable) -> Self {
		Self::new(GraphicGroup::new(vec![GraphicElement::VectorData(vector_data)]))
	}
}
impl From<ImageFrame<Color>> for GraphicGroupTable {
	fn from(image_frame: ImageFrame<Color>) -> Self {
		Self::new(GraphicGroup::new(vec![GraphicElement::RasterFrame(RasterFrame::ImageFrame(ImageFrameTable::new(image_frame)))]))
	}
}
impl From<ImageFrameTable<Color>> for GraphicGroupTable {
	fn from(image_frame: ImageFrameTable<Color>) -> Self {
		Self::new(GraphicGroup::new(vec![GraphicElement::RasterFrame(RasterFrame::ImageFrame(image_frame))]))
	}
}
impl From<TextureFrame> for GraphicGroupTable {
	fn from(texture_frame: TextureFrame) -> Self {
		Self::new(GraphicGroup::new(vec![GraphicElement::RasterFrame(RasterFrame::TextureFrame(TextureFrameTable::new(texture_frame)))]))
	}
}
impl From<TextureFrameTable> for GraphicGroupTable {
	fn from(texture_frame: TextureFrameTable) -> Self {
		Self::new(GraphicGroup::new(vec![GraphicElement::RasterFrame(RasterFrame::TextureFrame(texture_frame))]))
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

#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
pub enum RasterFrame {
	/// A CPU-based bitmap image with a finite position and extent, equivalent to the SVG <image> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/image
	ImageFrame(ImageFrameTable<Color>),
	/// A GPU texture with a finite position and extent
	TextureFrame(TextureFrameTable),
}

impl<'de> serde::Deserialize<'de> for RasterFrame {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(RasterFrame::ImageFrame(ImageFrameTable::new(ImageFrame::deserialize(deserializer)?)))
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

impl Transform for RasterFrame {
	fn transform(&self) -> DAffine2 {
		match self {
			RasterFrame::ImageFrame(frame) => frame.transform(),
			RasterFrame::TextureFrame(frame) => frame.transform(),
		}
	}
	fn local_pivot(&self, pivot: glam::DVec2) -> glam::DVec2 {
		match self {
			RasterFrame::ImageFrame(frame) => frame.local_pivot(pivot),
			RasterFrame::TextureFrame(frame) => frame.local_pivot(pivot),
		}
	}
}
impl TransformMut for RasterFrame {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			RasterFrame::ImageFrame(frame) => frame.transform_mut(),
			RasterFrame::TextureFrame(frame) => frame.transform_mut(),
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

impl Artboard {
	pub fn new(location: IVec2, dimensions: IVec2) -> Self {
		Self {
			graphic_group: GraphicGroupTable::default(),
			label: String::from("Artboard"),
			location: location.min(location + dimensions),
			dimensions: dimensions.abs(),
			background: Color::WHITE,
			clip: false,
		}
	}
}

/// Contains multiple artboards.
#[derive(Clone, Default, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtboardGroup {
	pub artboards: Vec<(Artboard, Option<NodeId>)>,
}

impl ArtboardGroup {
	pub fn new() -> Self {
		Default::default()
	}

	fn append_artboard(&mut self, artboard: Artboard, node_id: Option<NodeId>) {
		self.artboards.push((artboard, node_id));
	}
}

#[node_macro::node(category(""))]
async fn layer(_: impl Ctx, stack: GraphicGroupTable, mut element: GraphicElement, node_path: Vec<NodeId>) -> GraphicGroupTable {
	let mut stack = stack.one_item().clone();

	if stack.transform.matrix2.determinant() != 0. {
		*element.transform_mut() = stack.transform.inverse() * element.transform();
	} else {
		stack.clear();
		stack.transform = DAffine2::IDENTITY;
	}

	// Get the penultimate element of the node path, or None if the path is too short
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();
	stack.push((element, encapsulating_node_id));

	GraphicGroupTable::new(stack)
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
	let nested_group = group.one_item().clone();

	let mut flat_group = GraphicGroup::default();

	fn flatten_group(result_group: &mut GraphicGroup, current_group: GraphicGroup, fully_flatten: bool) {
		let mut collection_group = GraphicGroup::default();
		for (element, reference) in current_group.elements {
			if let GraphicElement::GraphicGroup(nested_group) = element {
				let nested_group = nested_group.one_item();
				let mut nested_group = nested_group.clone();

				*nested_group.transform_mut() = nested_group.transform() * current_group.transform;

				let mut sub_group = GraphicGroup::default();
				if fully_flatten {
					flatten_group(&mut sub_group, nested_group, fully_flatten);
				} else {
					for (collection_element, _) in &mut nested_group.elements {
						*collection_element.transform_mut() = nested_group.transform * collection_element.transform();
					}
					sub_group = nested_group;
				}
				collection_group.append(&mut sub_group.elements);
			} else {
				collection_group.push((element, reference));
			}
		}

		result_group.append(&mut collection_group.elements);
	}

	flatten_group(&mut flat_group, nested_group, fully_flatten);

	GraphicGroupTable::new(flat_group)
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
async fn append_artboard(ctx: impl Ctx, mut artboards: ArtboardGroup, artboard: Artboard, node_path: Vec<NodeId>) -> ArtboardGroup {
	// let mut artboards = artboards.eval(ctx.clone()).await;
	// let artboard = artboard.eval(ctx).await;
	// let foot = ctx.footprint();
	// log::debug!("{:?}", foot);
	// Get the penultimate element of the node path, or None if the path is too short

	// TODO: Delete this line
	let _ctx = ctx;

	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();
	artboards.append_artboard(artboard, encapsulating_node_id);

	artboards
}

// TODO: Remove this one
impl From<ImageFrame<Color>> for GraphicElement {
	fn from(image_frame: ImageFrame<Color>) -> Self {
		GraphicElement::RasterFrame(RasterFrame::ImageFrame(ImageFrameTable::new(image_frame)))
	}
}
impl From<ImageFrameTable<Color>> for GraphicElement {
	fn from(image_frame: ImageFrameTable<Color>) -> Self {
		GraphicElement::RasterFrame(RasterFrame::ImageFrame(image_frame))
	}
}
// TODO: Remove this one
impl From<TextureFrame> for GraphicElement {
	fn from(texture: TextureFrame) -> Self {
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
// TODO: Remove this one
impl From<GraphicGroup> for GraphicElement {
	fn from(graphic_group: GraphicGroup) -> Self {
		GraphicElement::GraphicGroup(GraphicGroupTable::new(graphic_group))
	}
}
impl From<GraphicGroupTable> for GraphicElement {
	fn from(graphic_group: GraphicGroupTable) -> Self {
		GraphicElement::GraphicGroup(graphic_group)
	}
}

impl Deref for GraphicGroup {
	type Target = Vec<(GraphicElement, Option<NodeId>)>;
	fn deref(&self) -> &Self::Target {
		&self.elements
	}
}
impl DerefMut for GraphicGroup {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.elements
	}
}

/// This is a helper trait used for the Into Implementation.
/// We can't just implement this for all for which from is implemented
/// as that would conflict with the implementation for `Self`
trait ToGraphicElement: Into<GraphicElement> {}

impl ToGraphicElement for VectorDataTable {}
impl ToGraphicElement for ImageFrameTable<Color> {}
impl ToGraphicElement for TextureFrame {}

impl<T> From<T> for GraphicGroup
where
	T: ToGraphicElement,
{
	fn from(value: T) -> Self {
		Self {
			elements: (vec![(value.into(), None)]),
			transform: DAffine2::IDENTITY,
			alpha_blending: AlphaBlending::default(),
		}
	}
}
