use crate::application_io::TextureFrame;
use crate::raster::{BlendMode, ImageFrame};
use crate::transform::{ApplyTransform, Footprint, Transform, TransformMut};
use crate::uuid::NodeId;
use crate::vector::VectorData;
use crate::Color;

use dyn_any::DynAny;
use node_macro::node;

use core::ops::{Deref, DerefMut};
use glam::{DAffine2, IVec2};

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
	pub const EMPTY: Self = Self {
		elements: Vec::new(),
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::new(),
	};

	pub fn new(elements: Vec<GraphicElement>) -> Self {
		Self {
			elements: elements.into_iter().map(|element| (element, None)).collect(),
			transform: DAffine2::IDENTITY,
			alpha_blending: AlphaBlending::new(),
		}
	}
}

/// The possible forms of graphical content held in a Vec by the `elements` field of [`GraphicElement`].
/// Can be another recursively nested [`GraphicGroup`], a [`VectorData`] shape, an [`ImageFrame`], or an [`Artboard`].
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphicElement {
	/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
	GraphicGroup(GraphicGroup),
	/// A vector shape, equivalent to the SVG <path> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path
	VectorData(Box<VectorData>),
	Raster(Raster),
}

// TODO: Can this be removed? It doesn't necessarily make that much sense to have a default when, instead, the entire GraphicElement just shouldn't exist if there's no specific content to assign it.
impl Default for GraphicElement {
	fn default() -> Self {
		Self::VectorData(Box::new(VectorData::empty()))
	}
}

impl GraphicElement {
	pub fn as_group(&self) -> Option<&GraphicGroup> {
		match self {
			GraphicElement::GraphicGroup(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_group_mut(&mut self) -> Option<&mut GraphicGroup> {
		match self {
			GraphicElement::GraphicGroup(group) => Some(group),
			_ => None,
		}
	}

	pub fn as_vector_data(&self) -> Option<&VectorData> {
		match self {
			GraphicElement::VectorData(data) => Some(data),
			_ => None,
		}
	}

	pub fn as_vector_data_mut(&mut self) -> Option<&mut VectorData> {
		match self {
			GraphicElement::VectorData(data) => Some(data),
			_ => None,
		}
	}

	pub fn as_raster(&self) -> Option<&Raster> {
		match self {
			GraphicElement::Raster(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_mut(&mut self) -> Option<&mut Raster> {
		match self {
			GraphicElement::Raster(raster) => Some(raster),
			_ => None,
		}
	}
}

#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
pub enum Raster {
	/// A bitmap image with a finite position and extent, equivalent to the SVG <image> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/image
	ImageFrame(ImageFrame<Color>),
	Texture(TextureFrame),
}

impl<'de> serde::Deserialize<'de> for Raster {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let frame = ImageFrame::deserialize(deserializer)?;
		Ok(Raster::ImageFrame(frame))
	}
}

impl serde::Serialize for Raster {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		match self {
			Raster::ImageFrame(_) => self.serialize(serializer),
			Raster::Texture(_) => todo!(),
		}
	}
}

impl Transform for Raster {
	fn transform(&self) -> DAffine2 {
		match self {
			Raster::ImageFrame(frame) => frame.transform(),
			Raster::Texture(frame) => frame.transform(),
		}
	}
	fn local_pivot(&self, pivot: glam::DVec2) -> glam::DVec2 {
		match self {
			Raster::ImageFrame(frame) => frame.local_pivot(pivot),
			Raster::Texture(frame) => frame.local_pivot(pivot),
		}
	}
}
impl TransformMut for Raster {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			Raster::ImageFrame(frame) => frame.transform_mut(),
			Raster::Texture(frame) => frame.transform_mut(),
		}
	}
}

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
/// Similar to an Inkscape page: https://media.inkscape.org/media/doc/release_notes/1.2/Inkscape_1.2.html#Page_tool
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Artboard {
	pub graphic_group: GraphicGroup,
	pub label: String,
	pub location: IVec2,
	pub dimensions: IVec2,
	pub background: Color,
	pub clip: bool,
}

impl Artboard {
	pub fn new(location: IVec2, dimensions: IVec2) -> Self {
		Self {
			graphic_group: GraphicGroup::EMPTY,
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
	pub const EMPTY: Self = Self { artboards: Vec::new() };

	pub fn new() -> Self {
		Default::default()
	}

	fn add_artboard(&mut self, artboard: Artboard, node_id: Option<NodeId>) {
		self.artboards.push((artboard, node_id));
	}
}

#[node]
async fn construct_layer<F: 'n + Copy + Send>(
	#[implementations((), Footprint)] footprint: F,
	#[implementations(((), GraphicGroup), (Footprint, GraphicGroup))] stack: impl Node<F, Output = GraphicGroup>,
	#[implementations(((), GraphicElement), (Footprint, GraphicElement))] graphic_element: impl Node<F, Output = GraphicElement>,
	node_path: Vec<NodeId>,
) -> GraphicGroup {
	let mut element = graphic_element.eval(footprint).await;
	let mut stack = stack.eval(footprint).await;
	if stack.transform.matrix2.determinant() != 0. {
		*element.transform_mut() = stack.transform.inverse() * element.transform();
	} else {
		stack.clear();
		stack.transform = DAffine2::IDENTITY;
	}

	// Get the penultimate element of the node path, or None if the path is too short
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();
	stack.push((element, encapsulating_node_id));
	stack
}

#[node]
async fn to_graphic_element<F: 'n + Send, Data: Into<GraphicElement> + 'n>(
	#[implementations((), (), (), (), Footprint)] footprint: F,
	#[implementations(
	 	((), VectorData),
		((), ImageFrame<Color>),
	 	((), GraphicGroup),
	 	((), TextureFrame),
	 	(Footprint, VectorData),
		(Footprint, ImageFrame<Color>),
	 	(Footprint, GraphicGroup),
	 	(Footprint, TextureFrame),
	 )]
	data: impl Node<F, Output = Data>,
) -> GraphicElement {
	data.eval(footprint).await.into()
}

#[node(name("Group"), category("General"))]
async fn group<F: 'n + Send, Data: Into<GraphicGroup> + 'n>(
	#[implementations((), (), (), (), Footprint)] footprint: F,
	#[implementations(
		((), VectorData),
		((), ImageFrame<Color>),
		((), GraphicGroup),
		((), TextureFrame),
		(Footprint, VectorData),
		(Footprint, ImageFrame<Color>),
		(Footprint, GraphicGroup),
		(Footprint, TextureFrame),
	)]
	element: impl Node<F, Output = Data>,
) -> GraphicGroup {
	element.eval(footprint).await.into()
}

#[node]
async fn construct_artboard<F: 'n + Copy + Send + ApplyTransform>(
	#[implementations((), Footprint)] mut footprint: F,
	#[implementations(((), GraphicGroup), (Footprint, GraphicGroup))] contents: impl Node<F, Output = GraphicGroup>,
	label: String,
	location: IVec2,
	dimensions: IVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	footprint.apply_transform(&DAffine2::from_translation(location.as_dvec2()));
	let graphic_group = contents.eval(footprint).await;

	Artboard {
		graphic_group,
		label,
		location: location.min(location + dimensions),
		dimensions: dimensions.abs(),
		background,
		clip,
	}
}
#[node]
async fn add_artboard<F: 'n + Copy + Send>(
	#[implementations((), Footprint)] footprint: F,
	#[implementations(((), ArtboardGroup), (Footprint, ArtboardGroup))] artboards: impl Node<F, Output = ArtboardGroup>,
	#[implementations(((), Artboard), (Footprint, Artboard))] artboard: impl Node<F, Output = Artboard>,
	node_path: Vec<NodeId>,
) -> ArtboardGroup {
	let artboard = artboard.eval(footprint).await;
	let mut artboards = artboards.eval(footprint).await;

	// Get the penultimate element of the node path, or None if the path is too short
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();
	artboards.add_artboard(artboard, encapsulating_node_id);

	artboards
}

impl From<ImageFrame<Color>> for GraphicElement {
	fn from(image_frame: ImageFrame<Color>) -> Self {
		GraphicElement::Raster(Raster::ImageFrame(image_frame))
	}
}
impl From<TextureFrame> for GraphicElement {
	fn from(texture: TextureFrame) -> Self {
		GraphicElement::Raster(Raster::Texture(texture))
	}
}
impl From<VectorData> for GraphicElement {
	fn from(vector_data: VectorData) -> Self {
		GraphicElement::VectorData(Box::new(vector_data))
	}
}
impl From<GraphicGroup> for GraphicElement {
	fn from(graphic_group: GraphicGroup) -> Self {
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

impl ToGraphicElement for VectorData {}
impl ToGraphicElement for ImageFrame<Color> {}
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
