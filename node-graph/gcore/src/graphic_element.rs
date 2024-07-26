use crate::application_io::SurfaceHandleFrame;
use crate::raster::{BlendMode, ImageFrame};
use crate::transform::Footprint;
use crate::vector::VectorData;
use crate::{Color, Node, SurfaceFrame};

use dyn_any::{DynAny, StaticType};
use node_macro::node_fn;

use core::ops::{Deref, DerefMut};
use glam::{DAffine2, IVec2, UVec2};
use web_sys::HtmlCanvasElement;

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
	elements: Vec<GraphicElement>,
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

/// The possible forms of graphical content held in a Vec by the `elements` field of [`GraphicElement`].
/// Can be another recursively nested [`GraphicGroup`], a [`VectorData`] shape, an [`ImageFrame`], or an [`Artboard`].
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphicElement {
	/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
	GraphicGroup(GraphicGroup),
	/// A vector shape, equivalent to the SVG <path> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path
	VectorData(Box<VectorData>),
	/// A bitmap image with a finite position and extent, equivalent to the SVG <image> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/image
	ImageFrame(ImageFrame<Color>),
	/// A Canvas element
	Surface(SurfaceFrame),
}

// TODO: Can this be removed? It doesn't necessarily make that much sense to have a default when, instead, the entire GraphicElement just shouldn't exist if there's no specific content to assign it.
impl Default for GraphicElement {
	fn default() -> Self {
		Self::VectorData(Box::new(VectorData::empty()))
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
	pub artboards: Vec<Artboard>,
}

impl ArtboardGroup {
	pub const EMPTY: Self = Self { artboards: Vec::new() };

	pub fn new() -> Self {
		Default::default()
	}

	fn add_artboard(&mut self, artboard: Artboard) {
		self.artboards.push(artboard);
	}
}

pub struct ConstructLayerNode<Stack, GraphicElement> {
	stack: Stack,
	graphic_element: GraphicElement,
}

#[node_fn(ConstructLayerNode)]
async fn construct_layer<Data: Into<GraphicElement> + Send>(
	footprint: crate::transform::Footprint,
	mut stack: impl Node<crate::transform::Footprint, Output = GraphicGroup>,
	graphic_element: impl Node<crate::transform::Footprint, Output = Data>,
) -> GraphicGroup {
	let graphic_element = self.graphic_element.eval(footprint).await;
	let mut stack = self.stack.eval(footprint).await;
	stack.push(graphic_element.into());
	stack
}

pub struct ToGraphicElementNode {}

#[node_fn(ToGraphicElementNode)]
fn to_graphic_element<Data: Into<GraphicElement>>(data: Data) -> GraphicElement {
	data.into()
}

pub struct ToGraphicGroupNode {}

#[node_fn(ToGraphicGroupNode)]
fn to_graphic_group<Data: Into<GraphicGroup>>(data: Data) -> GraphicGroup {
	data.into()
}

pub struct ConstructArtboardNode<Contents, Label, Location, Dimensions, Background, Clip> {
	contents: Contents,
	label: Label,
	location: Location,
	dimensions: Dimensions,
	background: Background,
	clip: Clip,
}

#[node_fn(ConstructArtboardNode)]
async fn construct_artboard(
	mut footprint: Footprint,
	contents: impl Node<Footprint, Output = GraphicGroup>,
	label: String,
	location: IVec2,
	dimensions: IVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	footprint.transform *= DAffine2::from_translation(location.as_dvec2());
	let graphic_group = self.contents.eval(footprint).await;

	Artboard {
		graphic_group,
		label,
		location: location.min(location + dimensions),
		dimensions: dimensions.abs(),
		background,
		clip,
	}
}
pub struct AddArtboardNode<ArtboardGroup, Artboard> {
	artboards: ArtboardGroup,
	artboard: Artboard,
}

#[node_fn(AddArtboardNode)]
async fn add_artboard<Data: Into<Artboard> + Send>(footprint: Footprint, artboards: impl Node<Footprint, Output = ArtboardGroup>, artboard: impl Node<Footprint, Output = Data>) -> ArtboardGroup {
	let artboard = self.artboard.eval(footprint).await;
	let mut artboards = self.artboards.eval(footprint).await;

	artboards.add_artboard(artboard.into());

	artboards
}

impl From<ImageFrame<Color>> for GraphicElement {
	fn from(image_frame: ImageFrame<Color>) -> Self {
		GraphicElement::ImageFrame(image_frame)
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
impl From<SurfaceFrame> for GraphicElement {
	fn from(surface: SurfaceFrame) -> Self {
		GraphicElement::Surface(surface)
	}
}
impl From<alloc::sync::Arc<SurfaceHandleFrame<HtmlCanvasElement>>> for GraphicElement {
	fn from(surface: alloc::sync::Arc<SurfaceHandleFrame<HtmlCanvasElement>>) -> Self {
		let surface_id = surface.surface_handle.surface_id;
		let transform = surface.transform;
		GraphicElement::Surface(SurfaceFrame {
			surface_id,
			transform,
			resolution: UVec2 {
				x: surface.surface_handle.surface.width(),
				y: surface.surface_handle.surface.height(),
			},
		})
	}
}
impl From<SurfaceHandleFrame<HtmlCanvasElement>> for GraphicElement {
	fn from(surface: SurfaceHandleFrame<HtmlCanvasElement>) -> Self {
		let surface_id = surface.surface_handle.surface_id;
		let transform = surface.transform;
		GraphicElement::Surface(SurfaceFrame {
			surface_id,
			transform,
			resolution: UVec2 {
				x: surface.surface_handle.surface.width(),
				y: surface.surface_handle.surface.height(),
			},
		})
	}
}

impl Deref for GraphicGroup {
	type Target = Vec<GraphicElement>;
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

impl<T> From<T> for GraphicGroup
where
	T: ToGraphicElement,
{
	fn from(value: T) -> Self {
		Self {
			elements: (vec![value.into()]),
			transform: DAffine2::IDENTITY,
			alpha_blending: AlphaBlending::default(),
		}
	}
}

impl GraphicGroup {
	pub const EMPTY: Self = Self {
		elements: Vec::new(),
		transform: DAffine2::IDENTITY,
		alpha_blending: AlphaBlending::new(),
	};
}
