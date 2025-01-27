use crate::application_io::{TextureFrame, TextureFrameTable};
use crate::raster::image::{ImageFrame, ImageFrameTable};
use crate::raster::BlendMode;
use crate::transform::{ApplyTransform, Footprint, Transform, TransformSet};
use crate::uuid::NodeId;
use crate::vector::{InstanceId, VectorData, VectorDataTable};
use crate::Color;

use dyn_any::{DynAny, StaticType};

use core::ops::{Deref, DerefMut};
use glam::{DAffine2, IVec2};
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};

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

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Instances<T>
where
	T: Into<GraphicElement> + StaticType + 'static,
{
	id: Vec<InstanceId>,
	instances: Vec<Arc<Mutex<T>>>,
}

impl<T: Into<GraphicElement> + StaticType + 'static> Instances<T> {
	pub fn new(instance: T) -> Self {
		Self {
			id: vec![InstanceId::generate()],
			instances: vec![Arc::new(Mutex::new(instance))],
		}
	}

	pub fn instances(&self) -> impl Iterator<Item = MutexGuard<'_, T>> {
		self.instances.iter().map(|item| item.lock().expect("Failed to lock mutex"))
	}

	pub fn id(&self) -> impl Iterator<Item = InstanceId> + '_ {
		self.id.iter().copied()
	}

	pub fn push(&mut self, id: InstanceId, instance: T) {
		self.id.push(id);
		self.instances.push(Arc::new(Mutex::new(instance)));
	}

	pub fn replace_all(&mut self, id: InstanceId, instance: T) {
		let mut instance = Arc::new(Mutex::new(instance));

		for (old_id, old_instance) in self.id.iter_mut().zip(self.instances.iter_mut()) {
			let mut new_id = id;
			std::mem::swap(old_id, &mut new_id);
			std::mem::swap(&mut instance, old_instance);
		}
	}
}

impl<T: Into<GraphicElement> + Hash + StaticType + 'static> core::hash::Hash for Instances<T> {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
		for instance in &self.instances {
			let instance = instance.lock().unwrap();
			instance.hash(state);
		}
	}
}

impl<T: Into<GraphicElement> + PartialEq + StaticType + 'static> PartialEq for Instances<T> {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id && self.instances.len() == other.instances.len() && { self.instances.iter().zip(other.instances.iter()).all(|(a, b)| *a.lock().unwrap() == *b.lock().unwrap()) }
	}
}

unsafe impl<T: Into<GraphicElement> + StaticType + 'static> dyn_any::StaticType for Instances<T> {
	type Static = Instances<T>;
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
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphicElement {
	/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
	GraphicGroup(GraphicGroup),
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
impl TransformSet for RasterFrame {
	fn set_transform(&mut self, value: DAffine2) {
		match self {
			RasterFrame::ImageFrame(frame) => frame.set_transform(value),
			RasterFrame::TextureFrame(frame) => frame.set_transform(value),
		}
	}
}

/// Some [`ArtboardData`] with some optional clipping bounds that can be exported.
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

	fn append_artboard(&mut self, artboard: Artboard, node_id: Option<NodeId>) {
		self.artboards.push((artboard, node_id));
	}
}

#[node_macro::node(category(""))]
async fn layer<F: 'n + Send + Copy>(
	#[implementations(
		(),
		Footprint,
	)]
	footprint: F,
	#[implementations(
		() -> GraphicGroup,
		Footprint -> GraphicGroup,
	)]
	stack: impl Node<F, Output = GraphicGroup>,
	#[implementations(
		() -> GraphicElement,
		Footprint -> GraphicElement,
	)]
	element: impl Node<F, Output = GraphicElement>,
	node_path: Vec<NodeId>,
) -> GraphicGroup {
	let mut element = element.eval(footprint).await;
	let mut stack = stack.eval(footprint).await;
	if stack.transform.matrix2.determinant() != 0. {
		element.set_transform(stack.transform.inverse() * element.transform());
	} else {
		stack.clear();
		stack.transform = DAffine2::IDENTITY;
	}

	// Get the penultimate element of the node path, or None if the path is too short
	let encapsulating_node_id = node_path.get(node_path.len().wrapping_sub(2)).copied();
	stack.push((element, encapsulating_node_id));
	stack
}

#[node_macro::node(category("Debug"))]
async fn to_element<F: 'n + Send, Data: Into<GraphicElement> + 'n>(
	#[implementations(
		(),
		(),
		(),
		(),
		Footprint,
	)]
	footprint: F,
	#[implementations(
		() -> GraphicGroup,
	 	() -> VectorDataTable,
		() -> ImageFrameTable<Color>,
	 	() -> TextureFrame,
	 	Footprint -> GraphicGroup,
	 	Footprint -> VectorDataTable,
		Footprint -> ImageFrameTable<Color>,
	 	Footprint -> TextureFrame,
	)]
	data: impl Node<F, Output = Data>,
) -> GraphicElement {
	data.eval(footprint).await.into()
}

#[node_macro::node(category("General"))]
async fn to_group<F: 'n + Send, Data: Into<GraphicGroup> + 'n>(
	#[implementations(
		(),
		(),
		(),
		(),
		Footprint,
	)]
	footprint: F,
	#[implementations(
		() -> GraphicGroup,
		() -> VectorDataTable,
		() -> ImageFrameTable<Color>,
		() -> TextureFrame,
		Footprint -> GraphicGroup,
		Footprint -> VectorDataTable,
		Footprint -> ImageFrameTable<Color>,
		Footprint -> TextureFrame,
	)]
	element: impl Node<F, Output = Data>,
) -> GraphicGroup {
	element.eval(footprint).await.into()
}

#[node_macro::node(category("General"))]
async fn flatten_group<F: 'n + Send>(
	#[implementations(
		(),
		Footprint,
	)]
	footprint: F,
	#[implementations(
		() -> GraphicGroup,
		Footprint -> GraphicGroup,
	)]
	group: impl Node<F, Output = GraphicGroup>,
	fully_flatten: bool,
) -> GraphicGroup {
	let nested_group = group.eval(footprint).await;
	let mut flat_group = GraphicGroup::EMPTY;
	fn flatten_group(result_group: &mut GraphicGroup, current_group: GraphicGroup, fully_flatten: bool) {
		let mut collection_group = GraphicGroup::EMPTY;
		for (element, reference) in current_group.elements {
			if let GraphicElement::GraphicGroup(mut nested_group) = element {
				nested_group.transform *= current_group.transform;
				let mut sub_group = GraphicGroup::EMPTY;
				if fully_flatten {
					flatten_group(&mut sub_group, nested_group, fully_flatten);
				} else {
					for (collection_element, _) in &mut nested_group.elements {
						collection_element.set_transform(nested_group.transform * collection_element.transform());
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
	flat_group
}

#[node_macro::node(category(""))]
async fn to_artboard<F: 'n + Send + ApplyTransform, Data: Into<GraphicGroup> + 'n>(
	#[implementations(
		(),
		(),
		(),
		(),
		Footprint,
	)]
	mut footprint: F,
	#[implementations(
		() -> GraphicGroup,
		() -> VectorDataTable,
		() -> ImageFrameTable<Color>,
		() -> TextureFrame,
		Footprint -> GraphicGroup,
		Footprint -> VectorDataTable,
		Footprint -> ImageFrameTable<Color>,
		Footprint -> TextureFrame,
	)]
	contents: impl Node<F, Output = Data>,
	label: String,
	location: IVec2,
	dimensions: IVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	footprint.apply_transform(&DAffine2::from_translation(location.as_dvec2()));
	let graphic_group = contents.eval(footprint).await;

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
async fn append_artboard<F: 'n + Send + Copy>(
	#[implementations(
		(),
		Footprint,
	)]
	footprint: F,
	#[implementations(
		() -> ArtboardGroup,
		Footprint -> ArtboardGroup,
	)]
	artboards: impl Node<F, Output = ArtboardGroup>,
	#[implementations(
		() -> Artboard,
		Footprint -> Artboard,
	)]
	artboard: impl Node<F, Output = Artboard>,
	node_path: Vec<NodeId>,
) -> ArtboardGroup {
	let artboard = artboard.eval(footprint).await;
	let mut artboards = artboards.eval(footprint).await;

	// Get the penultimate element of the node path, or None if the path is too short
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
