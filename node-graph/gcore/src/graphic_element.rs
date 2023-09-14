use crate::raster::{BlendMode, ImageFrame};
use crate::vector::{subpath, VectorData};
use crate::{Color, Node};

use bezier_rs::BezierHandles;
use dyn_any::{DynAny, StaticType};
use node_macro::node_fn;

use core::future::Future;
use core::ops::{Deref, DerefMut};
use glam::{DAffine2, DVec2, IVec2, UVec2};

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

impl Default for GraphicElement {
	fn default() -> Self {
		Self {
			name: "".to_owned(),
			blend_mode: BlendMode::Normal,
			opacity: 1.,
			visible: true,
			locked: false,
			collapsed: false,
			graphic_element_data: GraphicElementData::VectorShape(Box::new(VectorData::empty())),
		}
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

pub struct ConstructLayerNode<GraphicElementData, Name, BlendMode, Opacity, Visible, Locked, Collapsed, Stack> {
	graphic_element_data: GraphicElementData,
	name: Name,
	blend_mode: BlendMode,
	opacity: Opacity,
	visible: Visible,
	locked: Locked,
	collapsed: Collapsed,
	stack: Stack,
}

#[node_fn(ConstructLayerNode)]
async fn construct_layer<Data: Into<GraphicElementData>, Fut1: Future<Output = Data>, Fut2: Future<Output = GraphicGroup>>(
	footprint: crate::transform::Footprint,
	graphic_element_data: impl Node<crate::transform::Footprint, Output = Fut1>,
	name: String,
	blend_mode: BlendMode,
	opacity: f32,
	visible: bool,
	locked: bool,
	collapsed: bool,
	mut stack: impl Node<crate::transform::Footprint, Output = Fut2>,
) -> GraphicGroup {
	let graphic_element_data = self.graphic_element_data.eval(footprint).await;
	let mut stack = self.stack.eval(footprint).await;
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

pub struct ToGraphicElementData {}

#[node_fn(ToGraphicElementData)]
fn to_graphic_element_data<Data: Into<GraphicElementData>>(graphic_element_data: Data) -> GraphicElementData {
	graphic_element_data.into()
}

pub struct ConstructArtboardNode<Location, Dimensions, Background, Clip> {
	location: Location,
	dimensions: Dimensions,
	background: Background,
	clip: Clip,
}

#[node_fn(ConstructArtboardNode)]
fn construct_artboard(graphic_group: GraphicGroup, location: IVec2, dimensions: IVec2, background: Color, clip: bool) -> Artboard {
	Artboard {
		graphic_group,
		location: location.min(location + dimensions),
		dimensions: dimensions.abs(),
		background,
		clip,
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

/// This is a helper trait used for the Into Implementation.
/// We can't just implement this for all for which from is implemented
/// as that would conflict with the implementation for `Self`
trait ToGraphicElement: Into<GraphicElementData> {}

impl ToGraphicElement for VectorData {}
impl ToGraphicElement for ImageFrame<Color> {}
impl ToGraphicElement for Artboard {}

impl<T> From<T> for GraphicGroup
where
	T: ToGraphicElement,
{
	fn from(value: T) -> Self {
		let element = GraphicElement {
			graphic_element_data: value.into(),
			..Default::default()
		};
		Self(vec![element])
	}
}

impl GraphicGroup {
	pub const EMPTY: Self = Self(Vec::new());

	pub fn to_usvg_tree(&self, resolution: UVec2, viewbox: [DVec2; 2]) -> usvg::Tree {
		let root_node = usvg::Node::new(usvg::NodeKind::Group(usvg::Group::default()));
		let tree = usvg::Tree {
			size: usvg::Size::from_wh(resolution.x as f32, resolution.y as f32).unwrap(),
			view_box: usvg::ViewBox {
				rect: usvg::NonZeroRect::from_ltrb(viewbox[0].x as f32, viewbox[0].y as f32, viewbox[1].x as f32, viewbox[1].y as f32).unwrap(),
				aspect: usvg::AspectRatio::default(),
			},
			root: root_node.clone(),
		};

		for element in self.0.iter() {
			root_node.append(element.to_usvg_node());
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

		match &self.graphic_element_data {
			GraphicElementData::VectorShape(vector_data) => {
				use usvg::tiny_skia_path::PathBuilder;
				let mut builder = PathBuilder::new();

				let transform = to_transform(vector_data.transform);
				let style = &vector_data.style;
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
				path.transform = transform;
				// TODO: use proper style
				path.fill = None;
				path.stroke = Some(usvg::Stroke::default());
				usvg::Node::new(usvg::NodeKind::Path(path))
			}
			GraphicElementData::ImageFrame(image_frame) => {
				if image_frame.image.width * image_frame.image.height == 0 {
					return usvg::Node::new(usvg::NodeKind::Group(usvg::Group::default()));
				}
				let png = image_frame.image.to_png();
				usvg::Node::new(usvg::NodeKind::Image(usvg::Image {
					id: String::new(),
					transform: to_transform(image_frame.transform),
					visibility: usvg::Visibility::Visible,
					view_box: usvg::ViewBox {
						rect: usvg::NonZeroRect::from_xywh(0., 0., 1., 1.).unwrap(),
						aspect: usvg::AspectRatio::default(),
					},
					rendering_mode: usvg::ImageRendering::OptimizeSpeed,
					kind: usvg::ImageKind::PNG(png.into()),
				}))
			}
			GraphicElementData::Text(text) => usvg::Node::new(usvg::NodeKind::Text(usvg::Text {
				id: String::new(),
				transform: usvg::Transform::identity(),
				rendering_mode: usvg::TextRendering::OptimizeSpeed,
				positions: Vec::new(),
				rotate: Vec::new(),
				writing_mode: usvg::WritingMode::LeftToRight,
				chunks: vec![usvg::TextChunk {
					text: text.clone(),
					x: None,
					y: None,
					anchor: usvg::TextAnchor::Start,
					spans: vec![],
					text_flow: usvg::TextFlow::Linear,
				}],
			})),
			GraphicElementData::GraphicGroup(group) => {
				let group_element = usvg::Node::new(usvg::NodeKind::Group(usvg::Group::default()));

				for element in group.0.iter() {
					group_element.append(element.to_usvg_node());
				}
				group_element
			}
			// TODO
			GraphicElementData::Artboard(board) => usvg::Node::new(usvg::NodeKind::Group(usvg::Group::default())),
		}
	}
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
