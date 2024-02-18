use super::DocumentNode;
use crate::graphene_compiler::Any;
pub use crate::imaginate_input::{ImaginateCache, ImaginateController, ImaginateMaskStartingFill, ImaginateSamplingMethod};
use crate::proto::{Any as DAny, FutureAny};

use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::{BlendMode, LuminanceCalculation};
use graphene_core::{Color, Node, Type};

use dyn_any::DynAny;
pub use dyn_any::StaticType;
pub use glam::{DAffine2, DVec2, UVec2};
use std::hash::Hash;
pub use std::sync::Arc;

/// A type that is known, allowing serialization (serde::Deserialize is not object safe)
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TaggedValue {
	None,
	String(String),
	U32(u32),
	U64(u64),
	F32(f32),
	F64(f64),
	Bool(bool),
	UVec2(UVec2),
	DVec2(DVec2),
	OptionalDVec2(Option<DVec2>),
	DAffine2(DAffine2),
	Image(graphene_core::raster::Image<Color>),
	ImaginateCache(ImaginateCache),
	ImageFrame(graphene_core::raster::ImageFrame<Color>),
	Color(graphene_core::raster::color::Color),
	Subpaths(Vec<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
	RcSubpath(Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
	BlendMode(BlendMode),
	LuminanceCalculation(LuminanceCalculation),
	ImaginateSamplingMethod(ImaginateSamplingMethod),
	ImaginateMaskStartingFill(ImaginateMaskStartingFill),
	ImaginateController(ImaginateController),
	VectorData(graphene_core::vector::VectorData),
	Fill(graphene_core::vector::style::Fill),
	Stroke(graphene_core::vector::style::Stroke),
	VecF64(Vec<f64>),
	VecDVec2(Vec<DVec2>),
	RedGreenBlue(graphene_core::raster::RedGreenBlue),
	NoiseType(graphene_core::raster::NoiseType),
	FractalType(graphene_core::raster::FractalType),
	CellularDistanceFunction(graphene_core::raster::CellularDistanceFunction),
	CellularReturnType(graphene_core::raster::CellularReturnType),
	DomainWarpType(graphene_core::raster::DomainWarpType),
	RelativeAbsolute(graphene_core::raster::RelativeAbsolute),
	SelectiveColorChoice(graphene_core::raster::SelectiveColorChoice),
	LineCap(graphene_core::vector::style::LineCap),
	LineJoin(graphene_core::vector::style::LineJoin),
	FillType(graphene_core::vector::style::FillType),
	GradientType(graphene_core::vector::style::GradientType),
	GradientPositions(Vec<(f64, graphene_core::Color)>),
	Quantization(graphene_core::quantization::QuantizationChannels),
	OptionalColor(Option<graphene_core::raster::color::Color>),
	ManipulatorGroupIds(Vec<graphene_core::uuid::ManipulatorGroupId>),
	Font(graphene_core::text::Font),
	BrushStrokes(Vec<graphene_core::vector::brush_stroke::BrushStroke>),
	BrushCache(BrushCache),
	Segments(Vec<graphene_core::raster::ImageFrame<Color>>),
	DocumentNode(DocumentNode),
	GraphicGroup(graphene_core::GraphicGroup),
	Artboard(graphene_core::Artboard),
	Curve(graphene_core::raster::curve::Curve),
	IVec2(glam::IVec2),
	SurfaceFrame(graphene_core::SurfaceFrame),
	Footprint(graphene_core::transform::Footprint),
	RenderOutput(RenderOutput),
	Palette(Vec<Color>),
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for TaggedValue {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		core::mem::discriminant(self).hash(state);
		match self {
			Self::None => {}
			Self::String(x) => x.hash(state),
			Self::U32(x) => x.hash(state),
			Self::U64(x) => x.hash(state),
			Self::F32(x) => x.to_bits().hash(state),
			Self::F64(x) => x.to_bits().hash(state),
			Self::Bool(x) => x.hash(state),
			Self::UVec2(x) => x.to_array().iter().for_each(|x| x.hash(state)),
			Self::DVec2(x) => x.to_array().iter().for_each(|x| x.to_bits().hash(state)),
			Self::OptionalDVec2(None) => 0.hash(state),
			Self::OptionalDVec2(Some(x)) => {
				1.hash(state);
				Self::DVec2(*x).hash(state)
			}
			Self::DAffine2(x) => x.to_cols_array().iter().for_each(|x| x.to_bits().hash(state)),
			Self::Image(x) => x.hash(state),
			Self::ImaginateCache(x) => x.hash(state),
			Self::Color(x) => x.hash(state),
			Self::Subpaths(x) => x.iter().for_each(|subpath| subpath.hash(state)),
			Self::RcSubpath(x) => x.hash(state),
			Self::BlendMode(x) => x.hash(state),
			Self::LuminanceCalculation(x) => x.hash(state),
			Self::ImaginateSamplingMethod(x) => x.hash(state),
			Self::ImaginateMaskStartingFill(x) => x.hash(state),
			Self::ImaginateController(x) => x.hash(state),
			Self::ImageFrame(x) => x.hash(state),
			Self::VectorData(x) => x.hash(state),
			Self::Fill(x) => x.hash(state),
			Self::Stroke(x) => x.hash(state),
			Self::VecF64(x) => x.iter().for_each(|val| val.to_bits().hash(state)),
			Self::VecDVec2(x) => x.iter().for_each(|val| val.to_array().iter().for_each(|x| x.to_bits().hash(state))),
			Self::RedGreenBlue(x) => x.hash(state),
			Self::NoiseType(x) => x.hash(state),
			Self::FractalType(x) => x.hash(state),
			Self::CellularDistanceFunction(x) => x.hash(state),
			Self::CellularReturnType(x) => x.hash(state),
			Self::DomainWarpType(x) => x.hash(state),
			Self::RelativeAbsolute(x) => x.hash(state),
			Self::SelectiveColorChoice(x) => x.hash(state),
			Self::LineCap(x) => x.hash(state),
			Self::LineJoin(x) => x.hash(state),
			Self::FillType(x) => x.hash(state),
			Self::GradientType(x) => x.hash(state),
			Self::GradientPositions(x) => {
				x.len().hash(state);
				for (position, color) in x {
					position.to_bits().hash(state);
					color.hash(state);
				}
			}
			Self::Quantization(x) => x.hash(state),
			Self::OptionalColor(x) => x.hash(state),
			Self::ManipulatorGroupIds(x) => x.hash(state),
			Self::Font(x) => x.hash(state),
			Self::BrushStrokes(x) => x.hash(state),
			Self::BrushCache(x) => x.hash(state),
			Self::Segments(x) => {
				for segment in x {
					segment.hash(state)
				}
			}
			Self::DocumentNode(x) => x.hash(state),
			Self::GraphicGroup(x) => x.hash(state),
			Self::Artboard(x) => x.hash(state),
			Self::Curve(x) => x.hash(state),
			Self::IVec2(x) => x.hash(state),
			Self::SurfaceFrame(x) => x.hash(state),
			Self::Footprint(x) => x.hash(state),
			Self::RenderOutput(x) => x.hash(state),
			Self::Palette(x) => x.hash(state),
		}
	}
}

impl<'a> TaggedValue {
	/// Converts to a Box<dyn DynAny> - this isn't very neat but I'm not sure of a better approach
	pub fn to_any(self) -> Any<'a> {
		match self {
			TaggedValue::None => Box::new(()),
			TaggedValue::String(x) => Box::new(x),
			TaggedValue::U32(x) => Box::new(x),
			TaggedValue::U64(x) => Box::new(x),
			TaggedValue::F32(x) => Box::new(x),
			TaggedValue::F64(x) => Box::new(x),
			TaggedValue::Bool(x) => Box::new(x),
			TaggedValue::UVec2(x) => Box::new(x),
			TaggedValue::DVec2(x) => Box::new(x),
			TaggedValue::OptionalDVec2(x) => Box::new(x),
			TaggedValue::DAffine2(x) => Box::new(x),
			TaggedValue::Image(x) => Box::new(x),
			TaggedValue::ImaginateCache(x) => Box::new(x),
			TaggedValue::ImageFrame(x) => Box::new(x),
			TaggedValue::Color(x) => Box::new(x),
			TaggedValue::Subpaths(x) => Box::new(x),
			TaggedValue::RcSubpath(x) => Box::new(x),
			TaggedValue::BlendMode(x) => Box::new(x),
			TaggedValue::LuminanceCalculation(x) => Box::new(x),
			TaggedValue::ImaginateSamplingMethod(x) => Box::new(x),
			TaggedValue::ImaginateMaskStartingFill(x) => Box::new(x),
			TaggedValue::ImaginateController(x) => Box::new(x),
			TaggedValue::VectorData(x) => Box::new(x),
			TaggedValue::Fill(x) => Box::new(x),
			TaggedValue::Stroke(x) => Box::new(x),
			TaggedValue::VecF64(x) => Box::new(x),
			TaggedValue::VecDVec2(x) => Box::new(x),
			TaggedValue::RedGreenBlue(x) => Box::new(x),
			TaggedValue::NoiseType(x) => Box::new(x),
			TaggedValue::FractalType(x) => Box::new(x),
			TaggedValue::CellularDistanceFunction(x) => Box::new(x),
			TaggedValue::CellularReturnType(x) => Box::new(x),
			TaggedValue::DomainWarpType(x) => Box::new(x),
			TaggedValue::RelativeAbsolute(x) => Box::new(x),
			TaggedValue::SelectiveColorChoice(x) => Box::new(x),
			TaggedValue::LineCap(x) => Box::new(x),
			TaggedValue::LineJoin(x) => Box::new(x),
			TaggedValue::FillType(x) => Box::new(x),
			TaggedValue::GradientType(x) => Box::new(x),
			TaggedValue::GradientPositions(x) => Box::new(x),
			TaggedValue::Quantization(x) => Box::new(x),
			TaggedValue::OptionalColor(x) => Box::new(x),
			TaggedValue::ManipulatorGroupIds(x) => Box::new(x),
			TaggedValue::Font(x) => Box::new(x),
			TaggedValue::BrushStrokes(x) => Box::new(x),
			TaggedValue::BrushCache(x) => Box::new(x),
			TaggedValue::Segments(x) => Box::new(x),
			TaggedValue::DocumentNode(x) => Box::new(x),
			TaggedValue::GraphicGroup(x) => Box::new(x),
			TaggedValue::Artboard(x) => Box::new(x),
			TaggedValue::Curve(x) => Box::new(x),
			TaggedValue::IVec2(x) => Box::new(x),
			TaggedValue::SurfaceFrame(x) => Box::new(x),
			TaggedValue::Footprint(x) => Box::new(x),
			TaggedValue::RenderOutput(x) => Box::new(x),
			TaggedValue::Palette(x) => Box::new(x),
		}
	}

	pub fn to_primitive_string(&self) -> String {
		match self {
			TaggedValue::None => "()".to_string(),
			TaggedValue::String(x) => format!("\"{x}\""),
			TaggedValue::U32(x) => x.to_string() + "_u32",
			TaggedValue::U64(x) => x.to_string() + "_u64",
			TaggedValue::F32(x) => x.to_string() + "_f32",
			TaggedValue::F64(x) => x.to_string() + "_f64",
			TaggedValue::Bool(x) => x.to_string(),
			TaggedValue::BlendMode(x) => "BlendMode::".to_string() + &x.to_string(),
			TaggedValue::Color(x) => format!("Color {x:?}"),
			_ => panic!("Cannot convert to primitive string"),
		}
	}

	pub fn ty(&self) -> Type {
		match self {
			TaggedValue::None => concrete!(()),
			TaggedValue::String(_) => concrete!(String),
			TaggedValue::U32(_) => concrete!(u32),
			TaggedValue::U64(_) => concrete!(u64),
			TaggedValue::F32(_) => concrete!(f32),
			TaggedValue::F64(_) => concrete!(f64),
			TaggedValue::Bool(_) => concrete!(bool),
			TaggedValue::UVec2(_) => concrete!(UVec2),
			TaggedValue::DVec2(_) => concrete!(DVec2),
			TaggedValue::OptionalDVec2(_) => concrete!(Option<DVec2>),
			TaggedValue::Image(_) => concrete!(graphene_core::raster::Image<Color>),
			TaggedValue::ImaginateCache(_) => concrete!(ImaginateCache),
			TaggedValue::ImageFrame(_) => concrete!(graphene_core::raster::ImageFrame<Color>),
			TaggedValue::Color(_) => concrete!(graphene_core::raster::Color),
			TaggedValue::Subpaths(_) => concrete!(Vec<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
			TaggedValue::RcSubpath(_) => concrete!(Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
			TaggedValue::BlendMode(_) => concrete!(BlendMode),
			TaggedValue::ImaginateSamplingMethod(_) => concrete!(ImaginateSamplingMethod),
			TaggedValue::ImaginateMaskStartingFill(_) => concrete!(ImaginateMaskStartingFill),
			TaggedValue::ImaginateController(_) => concrete!(ImaginateController),
			TaggedValue::DAffine2(_) => concrete!(DAffine2),
			TaggedValue::LuminanceCalculation(_) => concrete!(LuminanceCalculation),
			TaggedValue::VectorData(_) => concrete!(graphene_core::vector::VectorData),
			TaggedValue::Fill(_) => concrete!(graphene_core::vector::style::Fill),
			TaggedValue::Stroke(_) => concrete!(graphene_core::vector::style::Stroke),
			TaggedValue::VecF64(_) => concrete!(Vec<f64>),
			TaggedValue::VecDVec2(_) => concrete!(Vec<DVec2>),
			TaggedValue::RedGreenBlue(_) => concrete!(graphene_core::raster::RedGreenBlue),
			TaggedValue::NoiseType(_) => concrete!(graphene_core::raster::NoiseType),
			TaggedValue::FractalType(_) => concrete!(graphene_core::raster::FractalType),
			TaggedValue::CellularDistanceFunction(_) => concrete!(graphene_core::raster::CellularDistanceFunction),
			TaggedValue::CellularReturnType(_) => concrete!(graphene_core::raster::CellularReturnType),
			TaggedValue::DomainWarpType(_) => concrete!(graphene_core::raster::DomainWarpType),
			TaggedValue::RelativeAbsolute(_) => concrete!(graphene_core::raster::RelativeAbsolute),
			TaggedValue::SelectiveColorChoice(_) => concrete!(graphene_core::raster::SelectiveColorChoice),
			TaggedValue::LineCap(_) => concrete!(graphene_core::vector::style::LineCap),
			TaggedValue::LineJoin(_) => concrete!(graphene_core::vector::style::LineJoin),
			TaggedValue::FillType(_) => concrete!(graphene_core::vector::style::FillType),
			TaggedValue::GradientType(_) => concrete!(graphene_core::vector::style::GradientType),
			TaggedValue::GradientPositions(_) => concrete!(Vec<(f64, graphene_core::Color)>),
			TaggedValue::Quantization(_) => concrete!(graphene_core::quantization::QuantizationChannels),
			TaggedValue::OptionalColor(_) => concrete!(Option<graphene_core::Color>),
			TaggedValue::ManipulatorGroupIds(_) => concrete!(Vec<graphene_core::uuid::ManipulatorGroupId>),
			TaggedValue::Font(_) => concrete!(graphene_core::text::Font),
			TaggedValue::BrushStrokes(_) => concrete!(Vec<graphene_core::vector::brush_stroke::BrushStroke>),
			TaggedValue::BrushCache(_) => concrete!(BrushCache),
			TaggedValue::Segments(_) => concrete!(graphene_core::raster::IndexNode<Vec<graphene_core::raster::ImageFrame<Color>>>),
			TaggedValue::DocumentNode(_) => concrete!(crate::document::DocumentNode),
			TaggedValue::GraphicGroup(_) => concrete!(graphene_core::GraphicGroup),
			TaggedValue::Artboard(_) => concrete!(graphene_core::Artboard),
			TaggedValue::Curve(_) => concrete!(graphene_core::raster::curve::Curve),
			TaggedValue::IVec2(_) => concrete!(glam::IVec2),
			TaggedValue::SurfaceFrame(_) => concrete!(graphene_core::SurfaceFrame),
			TaggedValue::Footprint(_) => concrete!(graphene_core::transform::Footprint),
			TaggedValue::RenderOutput(_) => concrete!(RenderOutput),
			TaggedValue::Palette(_) => concrete!(Vec<Color>),
		}
	}

	pub fn try_from_any(input: Box<dyn DynAny<'a> + 'a>) -> Result<Self, String> {
		use dyn_any::downcast;
		use std::any::TypeId;

		match DynAny::type_id(input.as_ref()) {
			x if x == TypeId::of::<()>() => Ok(TaggedValue::None),
			x if x == TypeId::of::<String>() => Ok(TaggedValue::String(*downcast(input).unwrap())),
			x if x == TypeId::of::<u32>() => Ok(TaggedValue::U32(*downcast(input).unwrap())),
			x if x == TypeId::of::<u64>() => Ok(TaggedValue::U64(*downcast(input).unwrap())),
			x if x == TypeId::of::<f32>() => Ok(TaggedValue::F32(*downcast(input).unwrap())),
			x if x == TypeId::of::<f64>() => Ok(TaggedValue::F64(*downcast(input).unwrap())),
			x if x == TypeId::of::<bool>() => Ok(TaggedValue::Bool(*downcast(input).unwrap())),
			x if x == TypeId::of::<UVec2>() => Ok(TaggedValue::UVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<DVec2>() => Ok(TaggedValue::DVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<Option<DVec2>>() => Ok(TaggedValue::OptionalDVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::Image<Color>>() => Ok(TaggedValue::Image(*downcast(input).unwrap())),
			x if x == TypeId::of::<ImaginateCache>() => Ok(TaggedValue::ImaginateCache(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::ImageFrame<Color>>() => Ok(TaggedValue::ImageFrame(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::Color>() => Ok(TaggedValue::Color(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>>() => Ok(TaggedValue::Subpaths(*downcast(input).unwrap())),
			x if x == TypeId::of::<Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>>() => Ok(TaggedValue::RcSubpath(*downcast(input).unwrap())),
			x if x == TypeId::of::<BlendMode>() => Ok(TaggedValue::BlendMode(*downcast(input).unwrap())),
			x if x == TypeId::of::<ImaginateSamplingMethod>() => Ok(TaggedValue::ImaginateSamplingMethod(*downcast(input).unwrap())),
			x if x == TypeId::of::<ImaginateMaskStartingFill>() => Ok(TaggedValue::ImaginateMaskStartingFill(*downcast(input).unwrap())),
			x if x == TypeId::of::<ImaginateController>() => Ok(TaggedValue::ImaginateController(*downcast(input).unwrap())),
			x if x == TypeId::of::<DAffine2>() => Ok(TaggedValue::DAffine2(*downcast(input).unwrap())),
			x if x == TypeId::of::<LuminanceCalculation>() => Ok(TaggedValue::LuminanceCalculation(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::VectorData>() => Ok(TaggedValue::VectorData(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::Fill>() => Ok(TaggedValue::Fill(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::Stroke>() => Ok(TaggedValue::Stroke(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<f64>>() => Ok(TaggedValue::VecF64(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<DVec2>>() => Ok(TaggedValue::VecDVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::RedGreenBlue>() => Ok(TaggedValue::RedGreenBlue(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::NoiseType>() => Ok(TaggedValue::NoiseType(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::FractalType>() => Ok(TaggedValue::FractalType(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::CellularDistanceFunction>() => Ok(TaggedValue::CellularDistanceFunction(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::CellularReturnType>() => Ok(TaggedValue::CellularReturnType(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::DomainWarpType>() => Ok(TaggedValue::DomainWarpType(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::RelativeAbsolute>() => Ok(TaggedValue::RelativeAbsolute(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::SelectiveColorChoice>() => Ok(TaggedValue::SelectiveColorChoice(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::LineCap>() => Ok(TaggedValue::LineCap(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::LineJoin>() => Ok(TaggedValue::LineJoin(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::FillType>() => Ok(TaggedValue::FillType(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::GradientType>() => Ok(TaggedValue::GradientType(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<(f64, graphene_core::Color)>>() => Ok(TaggedValue::GradientPositions(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::quantization::QuantizationChannels>() => Ok(TaggedValue::Quantization(*downcast(input).unwrap())),
			x if x == TypeId::of::<Option<graphene_core::Color>>() => Ok(TaggedValue::OptionalColor(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<graphene_core::uuid::ManipulatorGroupId>>() => Ok(TaggedValue::ManipulatorGroupIds(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::text::Font>() => Ok(TaggedValue::Font(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<graphene_core::vector::brush_stroke::BrushStroke>>() => Ok(TaggedValue::BrushStrokes(*downcast(input).unwrap())),
			x if x == TypeId::of::<BrushCache>() => Ok(TaggedValue::BrushCache(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::IndexNode<Vec<graphene_core::raster::ImageFrame<Color>>>>() => Ok(TaggedValue::Segments(*downcast(input).unwrap())),
			x if x == TypeId::of::<crate::document::DocumentNode>() => Ok(TaggedValue::DocumentNode(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::GraphicGroup>() => Ok(TaggedValue::GraphicGroup(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::Artboard>() => Ok(TaggedValue::Artboard(*downcast(input).unwrap())),
			x if x == TypeId::of::<glam::IVec2>() => Ok(TaggedValue::IVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::SurfaceFrame>() => Ok(TaggedValue::SurfaceFrame(*downcast(input).unwrap())),
			x if x == TypeId::of::<RenderOutput>() => Ok(TaggedValue::RenderOutput(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::WasmSurfaceHandleFrame>() => {
				let frame = *downcast::<graphene_core::WasmSurfaceHandleFrame>(input).unwrap();
				Ok(TaggedValue::SurfaceFrame(frame.into()))
			}
			x if x == TypeId::of::<graphene_core::transform::Footprint>() => Ok(TaggedValue::Footprint(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<Color>>() => Ok(TaggedValue::Palette(*downcast(input).unwrap())),
			_ => Err(format!("Cannot convert {:?} to TaggedValue", DynAny::type_name(input.as_ref()))),
		}
	}
}

pub struct UpcastNode {
	value: TaggedValue,
}
impl<'input> Node<'input, DAny<'input>> for UpcastNode {
	type Output = FutureAny<'input>;

	fn eval(&'input self, _: DAny<'input>) -> Self::Output {
		Box::pin(async move { self.value.clone().to_any() })
	}
}
impl UpcastNode {
	pub fn new(value: TaggedValue) -> Self {
		Self { value }
	}
}

#[derive(Debug, Clone, PartialEq, dyn_any::DynAny, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderOutput {
	CanvasFrame(graphene_core::SurfaceFrame),
	Svg(String),
	Image(Vec<u8>),
}
