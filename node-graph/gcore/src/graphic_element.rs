use crate::raster::{BlendMode, ImageFrame};
use crate::transform::Footprint;
use crate::vector::VectorData;
use crate::{Color, Node};

use bezier_rs::BezierHandles;
use dyn_any::{DynAny, StaticType};
use node_macro::node_fn;

use core::future::Future;
use core::ops::{Deref, DerefMut};
use glam::{DAffine2, DVec2, IVec2, UVec2};

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
/// Can be another recursively nested [`GraphicGroup`], [`VectorData`], an [`ImageFrame`], text (not yet implemented), or an [`Artboard`].
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphicElement {
	/// Equivalent to the SVG <g> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g
	GraphicGroup(GraphicGroup),
	/// A vector shape, equivalent to the SVG <path> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path
	VectorData(Box<VectorData>),
	/// A bitmap image with a finite position and extent, equivalent to the SVG <image> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/image
	ImageFrame(ImageFrame<Color>),
	// TODO: Switch from `String` to a proper formatted typography type
	/// Text, equivalent to the SVG <text> tag: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/text
	/// (Not yet implemented.)
	Text(String),
	/// The bounds for displaying a page of contained content
	Artboard(Artboard),
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
	pub location: IVec2,
	pub dimensions: IVec2,
	pub background: Color,
	pub clip: bool,
}

impl Artboard {
	pub fn new(location: IVec2, dimensions: IVec2) -> Self {
		Self {
			graphic_group: GraphicGroup::EMPTY,
			location: location.min(location + dimensions),
			dimensions: dimensions.abs(),
			background: Color::WHITE,
			clip: false,
		}
	}
}

pub struct ConstructLayerNode<GraphicElement, Stack> {
	graphic_element: GraphicElement,
	stack: Stack,
}

#[node_fn(ConstructLayerNode)]
async fn construct_layer<Data: Into<GraphicElement>, Fut1: Future<Output = Data>, Fut2: Future<Output = GraphicGroup>>(
	footprint: crate::transform::Footprint,
	graphic_element: impl Node<crate::transform::Footprint, Output = Fut1>,
	mut stack: impl Node<crate::transform::Footprint, Output = Fut2>,
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

pub struct ConstructArtboardNode<Contents, Location, Dimensions, Background, Clip> {
	contents: Contents,
	location: Location,
	dimensions: Dimensions,
	background: Background,
	clip: Clip,
}

#[node_fn(ConstructArtboardNode)]
async fn construct_artboard<Fut: Future<Output = GraphicGroup>>(
	mut footprint: Footprint,
	contents: impl Node<Footprint, Output = Fut>,
	location: IVec2,
	dimensions: IVec2,
	background: Color,
	clip: bool,
) -> Artboard {
	footprint.transform *= DAffine2::from_translation(location.as_dvec2());
	let graphic_group = self.contents.eval(footprint).await;
	Artboard {
		graphic_group,
		location: location.min(location + dimensions),
		dimensions: dimensions.abs(),
		background,
		clip,
	}
}

impl From<ImageFrame<Color>> for GraphicElement {
	fn from(mut image_frame: ImageFrame<Color>) -> Self {
		use base64::Engine;

		let image = &image_frame.image;
		if !image.data.is_empty() {
			let output = image.to_png();
			let preamble = "data:image/png;base64,";
			let mut base64_string = String::with_capacity(preamble.len() + output.len() * 4);
			base64_string.push_str(preamble);
			base64::engine::general_purpose::STANDARD.encode_string(output, &mut base64_string);
			image_frame.image.base64_string = Some(base64_string);
		}

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
impl From<Artboard> for GraphicElement {
	fn from(artboard: Artboard) -> Self {
		GraphicElement::Artboard(artboard)
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
impl ToGraphicElement for Artboard {}

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

	pub fn to_usvg_tree(&self, resolution: UVec2, viewbox: [DVec2; 2]) -> usvg::Tree {
		let mut root_node = usvg::Group::default();
		let tree = usvg::Tree {
			size: usvg::Size::from_wh(resolution.x as f32, resolution.y as f32).unwrap(),
			view_box: usvg::ViewBox {
				rect: usvg::NonZeroRect::from_ltrb(viewbox[0].x as f32, viewbox[0].y as f32, viewbox[1].x as f32, viewbox[1].y as f32).unwrap(),
				aspect: usvg::AspectRatio::default(),
			},
			root: root_node.clone(),
		};

		for element in self.iter() {
			root_node.children.push(element.to_usvg_node());
		}
		tree
	}
}

impl GraphicElement {
	fn to_usvg_node(&self) -> usvg::Node {
		fn to_transform(transform: DAffine2) -> usvg::Transform {
			let cols = transform.to_cols_array();
			usvg::Transform::from_row(cols[0] as f32, cols[1] as f32, cols[2] as f32, cols[3] as f32, cols[4] as f32, cols[5] as f32)
		}

		match self {
			GraphicElement::VectorData(vector_data) => {
				use usvg::tiny_skia_path::PathBuilder;
				let mut builder = PathBuilder::new();

				let transform = to_transform(vector_data.transform);
				for subpath in vector_data.subpaths.iter() {
					let start = vector_data.transform.transform_point2(subpath[0].anchor);
					builder.move_to(start.x as f32, start.y as f32);
					for bezier in subpath.iter() {
						bezier.apply_transformation(|pos| vector_data.transform.transform_point2(pos));
						let end = bezier.end;
						match bezier.handles {
							BezierHandles::Linear => builder.line_to(end.x as f32, end.y as f32),
							BezierHandles::Quadratic { handle } => builder.quad_to(handle.x as f32, handle.y as f32, end.x as f32, end.y as f32),
							BezierHandles::Cubic { handle_start, handle_end } => {
								builder.cubic_to(handle_start.x as f32, handle_start.y as f32, handle_end.x as f32, handle_end.y as f32, end.x as f32, end.y as f32)
							}
						}
					}
					if subpath.closed {
						builder.close()
					}
				}
				let path = builder.finish().unwrap();
				let mut path = usvg::Path::new(path.into());
				path.abs_transform = transform;
				// TODO: use proper style
				path.fill = None;
				path.stroke = Some(usvg::Stroke::default());
				usvg::Node::Path(Box::new(path))
			}
			GraphicElement::ImageFrame(image_frame) => {
				if image_frame.image.width * image_frame.image.height == 0 {
					return usvg::Node::Group(Box::default());
				}
				let png = image_frame.image.to_png();
				usvg::Node::Image(Box::new(usvg::Image {
					id: String::new(),
					abs_transform: to_transform(image_frame.transform),
					visibility: usvg::Visibility::Visible,
					view_box: usvg::ViewBox {
						rect: usvg::NonZeroRect::from_xywh(0., 0., 1., 1.).unwrap(),
						aspect: usvg::AspectRatio::default(),
					},
					rendering_mode: usvg::ImageRendering::OptimizeSpeed,
					kind: usvg::ImageKind::PNG(png.into()),
					bounding_box: None,
				}))
			}
			GraphicElement::Text(text) => usvg::Node::Text(Box::new(usvg::Text {
				id: String::new(),
				abs_transform: usvg::Transform::identity(),
				rendering_mode: usvg::TextRendering::OptimizeSpeed,
				writing_mode: usvg::WritingMode::LeftToRight,
				chunks: vec![usvg::TextChunk {
					text: text.clone(),
					x: None,
					y: None,
					anchor: usvg::TextAnchor::Start,
					spans: vec![],
					text_flow: usvg::TextFlow::Linear,
				}],
				dx: Vec::new(),
				dy: Vec::new(),
				rotate: Vec::new(),
				bounding_box: None,
				abs_bounding_box: None,
				stroke_bounding_box: None,
				abs_stroke_bounding_box: None,
				flattened: None,
			})),
			GraphicElement::GraphicGroup(group) => {
				let mut group_element = usvg::Group::default();

				for element in group.iter() {
					group_element.children.push(element.to_usvg_node());
				}
				usvg::Node::Group(Box::new(group_element))
			}
			// TODO
			GraphicElement::Artboard(_board) => usvg::Node::Group(Box::default()),
		}
	}
}
