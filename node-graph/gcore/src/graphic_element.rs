use crate::raster::{BlendMode, ImageFrame};
use crate::vector::VectorData;
use crate::{Color, Node};

use dyn_any::{DynAny, StaticType};

use core::ops::{Deref, DerefMut};
use glam::IVec2;
use node_macro::node_fn;

pub mod renderer;

/// A list of [`GraphicElement`]s
#[derive(Clone, Debug, Hash, PartialEq, DynAny, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphicGroup(Vec<GraphicElement>);

/// Internal data for a [`GraphicElement`]. Can be [`VectorData`], [`ImageFrame`], text, or a nested [`GraphicGroup`]
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphicElementData {
	VectorShape(Box<VectorData>),
	ImageFrame(ImageFrame<Color>),
	Text(String),
	GraphicGroup(GraphicGroup),
	Artboard(Artboard),
}

/// A named [`GraphicElementData`] with a blend mode, opacity, as well as visibility, locked, and collapsed states.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphicElement {
	pub name: String,
	pub blend_mode: BlendMode,
	/// In range 0..=1
	pub opacity: f32,
	pub visible: bool,
	pub locked: bool,
	pub collapsed: bool,
	pub graphic_element_data: GraphicElementData,
}

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
/// Similar to an Inkscape page: https://media.inkscape.org/media/doc/release_notes/1.2/Inkscape_1.2.html#Page_tool
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Artboard {
	pub graphic_group: GraphicGroup,
	pub location: IVec2,
	pub dimensions: IVec2,
	pub background: Color,
}

pub struct ConstructLayerNode<Name, BlendMode, Opacity, Visible, Locked, Collapsed, Stack> {
	name: Name,
	blend_mode: BlendMode,
	opacity: Opacity,
	visible: Visible,
	locked: Locked,
	collapsed: Collapsed,
	stack: Stack,
}

#[node_fn(ConstructLayerNode)]
fn construct_layer<Data: Into<GraphicElementData>>(
	graphic_element_data: Data,
	name: String,
	blend_mode: BlendMode,
	opacity: f32,
	visible: bool,
	locked: bool,
	collapsed: bool,
	mut stack: GraphicGroup,
) -> GraphicGroup {
	stack.push(GraphicElement {
		name,
		blend_mode,
		opacity: opacity / 100.,
		visible,
		locked,
		collapsed,
		graphic_element_data: graphic_element_data.into(),
	});
	stack
}

pub struct ConstructArtboardNode<Location, Dimensions, Background> {
	location: Location,
	dimensions: Dimensions,
	background: Background,
}

#[node_fn(ConstructArtboardNode)]
fn construct_artboard(graphic_group: GraphicGroup, location: IVec2, dimensions: IVec2, background: Color) -> Artboard {
	Artboard {
		graphic_group,
		location,
		dimensions,
		background,
	}
}

impl From<ImageFrame<Color>> for GraphicElementData {
	fn from(image_frame: ImageFrame<Color>) -> Self {
		GraphicElementData::ImageFrame(image_frame)
	}
}
impl From<VectorData> for GraphicElementData {
	fn from(vector_data: VectorData) -> Self {
		GraphicElementData::VectorShape(Box::new(vector_data))
	}
}
impl From<GraphicGroup> for GraphicElementData {
	fn from(graphic_group: GraphicGroup) -> Self {
		GraphicElementData::GraphicGroup(graphic_group)
	}
}

impl From<Artboard> for GraphicElementData {
	fn from(artboard: Artboard) -> Self {
		GraphicElementData::Artboard(artboard)
	}
}

impl Deref for GraphicGroup {
	type Target = Vec<GraphicElement>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl DerefMut for GraphicGroup {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl GraphicGroup {
	pub const EMPTY: Self = Self(Vec::new());
}

impl core::hash::Hash for GraphicElement {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.name.hash(state);
		self.blend_mode.hash(state);
		self.opacity.to_bits().hash(state);
		self.visible.hash(state);
		self.locked.hash(state);
		self.collapsed.hash(state);
		self.graphic_element_data.hash(state);
	}
}
