use super::DocumentNode;
use crate::executor::Any;
pub use crate::imaginate_input::{ImaginateMaskStartingFill, ImaginateSamplingMethod, ImaginateStatus};

use graphene_core::raster::{BlendMode, LuminanceCalculation};
use graphene_core::{Color, Node, Type};

pub use dyn_any::StaticType;
use dyn_any::{DynAny, Upcast};
use dyn_clone::DynClone;
pub use glam::{DAffine2, DVec2};
use std::hash::Hash;
pub use std::sync::Arc;

/// A type that is known, allowing serialization (serde::Deserialize is not object safe)
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TaggedValue {
	None,
	String(String),
	U32(u32),
	F32(f32),
	F64(f64),
	Bool(bool),
	DVec2(DVec2),
	OptionalDVec2(Option<DVec2>),
	DAffine2(DAffine2),
	Image(graphene_core::raster::Image<Color>),
	RcImage(Option<Arc<graphene_core::raster::Image<Color>>>),
	ImageFrame(graphene_core::raster::ImageFrame<Color>),
	Color(graphene_core::raster::color::Color),
	Subpaths(Vec<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
	RcSubpath(Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
	BlendMode(BlendMode),
	LuminanceCalculation(LuminanceCalculation),
	ImaginateSamplingMethod(ImaginateSamplingMethod),
	ImaginateMaskStartingFill(ImaginateMaskStartingFill),
	ImaginateStatus(ImaginateStatus),
	LayerPath(Option<Vec<u64>>),
	VectorData(graphene_core::vector::VectorData),
	Fill(graphene_core::vector::style::Fill),
	Stroke(graphene_core::vector::style::Stroke),
	VecF32(Vec<f32>),
	RedGreenBlue(graphene_core::raster::RedGreenBlue),
	RelativeAbsolute(graphene_core::raster::RelativeAbsolute),
	SelectiveColorChoice(graphene_core::raster::SelectiveColorChoice),
	LineCap(graphene_core::vector::style::LineCap),
	LineJoin(graphene_core::vector::style::LineJoin),
	FillType(graphene_core::vector::style::FillType),
	GradientType(graphene_core::vector::style::GradientType),
	GradientPositions(Vec<(f64, Option<graphene_core::Color>)>),
	Quantization(graphene_core::quantization::QuantizationChannels),
	OptionalColor(Option<graphene_core::raster::color::Color>),
	ManipulatorGroupIds(Vec<graphene_core::uuid::ManipulatorGroupId>),
	Font(graphene_core::text::Font),
	VecDVec2(Vec<DVec2>),
	Segments(Vec<graphene_core::raster::ImageFrame<Color>>),
	EditorApi(graphene_core::EditorApi<'static>),
	DocumentNode(DocumentNode),
	GraphicGroup(graphene_core::GraphicGroup),
	Artboard(graphene_core::Artboard),
	Optional2IVec2(Option<[glam::IVec2; 2]>),
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for TaggedValue {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		core::mem::discriminant(self).hash(state);
		match self {
			Self::None => {}
			Self::String(s) => s.hash(state),
			Self::U32(u) => u.hash(state),
			Self::F32(f) => f.to_bits().hash(state),
			Self::F64(f) => f.to_bits().hash(state),
			Self::Bool(b) => b.hash(state),
			Self::DVec2(v) => v.to_array().iter().for_each(|x| x.to_bits().hash(state)),
			Self::OptionalDVec2(None) => 0.hash(state),
			Self::OptionalDVec2(Some(v)) => {
				1.hash(state);
				Self::DVec2(*v).hash(state)
			}
			Self::DAffine2(m) => m.to_cols_array().iter().for_each(|x| x.to_bits().hash(state)),
			Self::Image(i) => i.hash(state),
			Self::RcImage(i) => i.hash(state),
			Self::Color(c) => c.hash(state),
			Self::Subpaths(s) => s.iter().for_each(|subpath| subpath.hash(state)),
			Self::RcSubpath(s) => s.hash(state),
			Self::BlendMode(b) => b.hash(state),
			Self::LuminanceCalculation(l) => l.hash(state),
			Self::ImaginateSamplingMethod(m) => m.hash(state),
			Self::ImaginateMaskStartingFill(f) => f.hash(state),
			Self::ImaginateStatus(s) => s.hash(state),
			Self::LayerPath(p) => p.hash(state),
			Self::ImageFrame(i) => i.hash(state),
			Self::VectorData(vector_data) => vector_data.hash(state),
			Self::Fill(fill) => fill.hash(state),
			Self::Stroke(stroke) => stroke.hash(state),
			Self::VecF32(vec_f32) => vec_f32.iter().for_each(|val| val.to_bits().hash(state)),
			Self::RedGreenBlue(red_green_blue) => red_green_blue.hash(state),
			Self::RelativeAbsolute(relative_absolute) => relative_absolute.hash(state),
			Self::SelectiveColorChoice(selective_color_choice) => selective_color_choice.hash(state),
			Self::LineCap(line_cap) => line_cap.hash(state),
			Self::LineJoin(line_join) => line_join.hash(state),
			Self::FillType(fill_type) => fill_type.hash(state),
			Self::GradientType(gradient_type) => gradient_type.hash(state),
			Self::GradientPositions(gradient_positions) => {
				gradient_positions.len().hash(state);
				for (position, color) in gradient_positions {
					position.to_bits().hash(state);
					color.hash(state);
				}
			}
			Self::Quantization(quantized_image) => quantized_image.hash(state),
			Self::OptionalColor(color) => color.hash(state),
			Self::ManipulatorGroupIds(mirror) => mirror.hash(state),
			Self::Font(font) => font.hash(state),
			Self::VecDVec2(vec_dvec2) => {
				vec_dvec2.len().hash(state);
				for dvec2 in vec_dvec2 {
					dvec2.to_array().iter().for_each(|x| x.to_bits().hash(state));
				}
			}
			Self::Segments(segments) => {
				for segment in segments {
					segment.hash(state)
				}
			}
			Self::EditorApi(editor_api) => editor_api.hash(state),
			Self::DocumentNode(document_node) => document_node.hash(state),
			Self::GraphicGroup(graphic_group) => graphic_group.hash(state),
			Self::Artboard(artboard) => artboard.hash(state),
			Self::Optional2IVec2(v) => v.hash(state),
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
			TaggedValue::F32(x) => Box::new(x),
			TaggedValue::F64(x) => Box::new(x),
			TaggedValue::Bool(x) => Box::new(x),
			TaggedValue::DVec2(x) => Box::new(x),
			TaggedValue::OptionalDVec2(x) => Box::new(x),
			TaggedValue::DAffine2(x) => Box::new(x),
			TaggedValue::Image(x) => Box::new(x),
			TaggedValue::RcImage(x) => Box::new(x),
			TaggedValue::ImageFrame(x) => Box::new(x),
			TaggedValue::Color(x) => Box::new(x),
			TaggedValue::Subpaths(x) => Box::new(x),
			TaggedValue::RcSubpath(x) => Box::new(x),
			TaggedValue::BlendMode(x) => Box::new(x),
			TaggedValue::LuminanceCalculation(x) => Box::new(x),
			TaggedValue::ImaginateSamplingMethod(x) => Box::new(x),
			TaggedValue::ImaginateMaskStartingFill(x) => Box::new(x),
			TaggedValue::ImaginateStatus(x) => Box::new(x),
			TaggedValue::LayerPath(x) => Box::new(x),
			TaggedValue::VectorData(x) => Box::new(x),
			TaggedValue::Fill(x) => Box::new(x),
			TaggedValue::Stroke(x) => Box::new(x),
			TaggedValue::VecF32(x) => Box::new(x),
			TaggedValue::RedGreenBlue(x) => Box::new(x),
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
			TaggedValue::VecDVec2(x) => Box::new(x),
			TaggedValue::Segments(x) => Box::new(x),
			TaggedValue::EditorApi(x) => Box::new(x),
			TaggedValue::DocumentNode(x) => Box::new(x),
			TaggedValue::GraphicGroup(x) => Box::new(x),
			TaggedValue::Artboard(x) => Box::new(x),
			TaggedValue::Optional2IVec2(x) => Box::new(x),
		}
	}

	pub fn to_primitive_string(&self) -> String {
		match self {
			TaggedValue::None => "()".to_string(),
			TaggedValue::String(x) => x.clone(),
			TaggedValue::U32(x) => x.to_string(),
			TaggedValue::F32(x) => x.to_string(),
			TaggedValue::F64(x) => x.to_string(),
			TaggedValue::Bool(x) => x.to_string(),
			_ => panic!("Cannot convert to primitive string"),
		}
	}

	pub fn ty(&self) -> Type {
		use graphene_core::TypeDescriptor;
		use std::borrow::Cow;
		match self {
			TaggedValue::None => concrete!(()),
			TaggedValue::String(_) => concrete!(String),
			TaggedValue::U32(_) => concrete!(u32),
			TaggedValue::F32(_) => concrete!(f32),
			TaggedValue::F64(_) => concrete!(f64),
			TaggedValue::Bool(_) => concrete!(bool),
			TaggedValue::DVec2(_) => concrete!(DVec2),
			TaggedValue::OptionalDVec2(_) => concrete!(Option<DVec2>),
			TaggedValue::Image(_) => concrete!(graphene_core::raster::Image<Color>),
			TaggedValue::RcImage(_) => concrete!(Option<Arc<graphene_core::raster::Image<Color>>>),
			TaggedValue::ImageFrame(_) => concrete!(graphene_core::raster::ImageFrame<Color>),
			TaggedValue::Color(_) => concrete!(graphene_core::raster::Color),
			TaggedValue::Subpaths(_) => concrete!(Vec<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
			TaggedValue::RcSubpath(_) => concrete!(Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
			TaggedValue::BlendMode(_) => concrete!(BlendMode),
			TaggedValue::ImaginateSamplingMethod(_) => concrete!(ImaginateSamplingMethod),
			TaggedValue::ImaginateMaskStartingFill(_) => concrete!(ImaginateMaskStartingFill),
			TaggedValue::ImaginateStatus(_) => concrete!(ImaginateStatus),
			TaggedValue::LayerPath(_) => concrete!(Option<Vec<u64>>),
			TaggedValue::DAffine2(_) => concrete!(DAffine2),
			TaggedValue::LuminanceCalculation(_) => concrete!(LuminanceCalculation),
			TaggedValue::VectorData(_) => concrete!(graphene_core::vector::VectorData),
			TaggedValue::Fill(_) => concrete!(graphene_core::vector::style::Fill),
			TaggedValue::Stroke(_) => concrete!(graphene_core::vector::style::Stroke),
			TaggedValue::VecF32(_) => concrete!(Vec<f32>),
			TaggedValue::RedGreenBlue(_) => concrete!(graphene_core::raster::RedGreenBlue),
			TaggedValue::RelativeAbsolute(_) => concrete!(graphene_core::raster::RelativeAbsolute),
			TaggedValue::SelectiveColorChoice(_) => concrete!(graphene_core::raster::SelectiveColorChoice),
			TaggedValue::LineCap(_) => concrete!(graphene_core::vector::style::LineCap),
			TaggedValue::LineJoin(_) => concrete!(graphene_core::vector::style::LineJoin),
			TaggedValue::FillType(_) => concrete!(graphene_core::vector::style::FillType),
			TaggedValue::GradientType(_) => concrete!(graphene_core::vector::style::GradientType),
			TaggedValue::GradientPositions(_) => concrete!(Vec<(f64, Option<graphene_core::Color>)>),
			TaggedValue::Quantization(_) => concrete!(graphene_core::quantization::QuantizationChannels),
			TaggedValue::OptionalColor(_) => concrete!(Option<graphene_core::Color>),
			TaggedValue::ManipulatorGroupIds(_) => concrete!(Vec<graphene_core::uuid::ManipulatorGroupId>),
			TaggedValue::Font(_) => concrete!(graphene_core::text::Font),
			TaggedValue::VecDVec2(_) => concrete!(Vec<DVec2>),
			TaggedValue::Segments(_) => concrete!(graphene_core::raster::IndexNode<Vec<graphene_core::raster::ImageFrame<Color>>>),
			TaggedValue::EditorApi(_) => concrete!(graphene_core::EditorApi),
			TaggedValue::DocumentNode(_) => concrete!(crate::document::DocumentNode),
			TaggedValue::GraphicGroup(_) => concrete!(graphene_core::GraphicGroup),
			TaggedValue::Artboard(_) => concrete!(graphene_core::Artboard),
			TaggedValue::Optional2IVec2(_) => concrete!(Option<[glam::IVec2; 2]>),
		}
	}

	pub fn try_from_any(input: Box<dyn DynAny<'a> + 'a>) -> Option<Self> {
		use dyn_any::downcast;
		use std::any::TypeId;

		match DynAny::type_id(input.as_ref()) {
			x if x == TypeId::of::<()>() => Some(TaggedValue::None),
			x if x == TypeId::of::<String>() => Some(TaggedValue::String(*downcast(input).unwrap())),
			x if x == TypeId::of::<u32>() => Some(TaggedValue::U32(*downcast(input).unwrap())),
			x if x == TypeId::of::<f32>() => Some(TaggedValue::F32(*downcast(input).unwrap())),
			x if x == TypeId::of::<f64>() => Some(TaggedValue::F64(*downcast(input).unwrap())),
			x if x == TypeId::of::<bool>() => Some(TaggedValue::Bool(*downcast(input).unwrap())),
			x if x == TypeId::of::<DVec2>() => Some(TaggedValue::DVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<Option<DVec2>>() => Some(TaggedValue::OptionalDVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::Image<Color>>() => Some(TaggedValue::Image(*downcast(input).unwrap())),
			x if x == TypeId::of::<Option<Arc<graphene_core::raster::Image<Color>>>>() => Some(TaggedValue::RcImage(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::ImageFrame<Color>>() => Some(TaggedValue::ImageFrame(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::Color>() => Some(TaggedValue::Color(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>>() => Some(TaggedValue::Subpaths(*downcast(input).unwrap())),
			x if x == TypeId::of::<Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>>() => Some(TaggedValue::RcSubpath(*downcast(input).unwrap())),
			x if x == TypeId::of::<BlendMode>() => Some(TaggedValue::BlendMode(*downcast(input).unwrap())),
			x if x == TypeId::of::<ImaginateSamplingMethod>() => Some(TaggedValue::ImaginateSamplingMethod(*downcast(input).unwrap())),
			x if x == TypeId::of::<ImaginateMaskStartingFill>() => Some(TaggedValue::ImaginateMaskStartingFill(*downcast(input).unwrap())),
			x if x == TypeId::of::<ImaginateStatus>() => Some(TaggedValue::ImaginateStatus(*downcast(input).unwrap())),
			x if x == TypeId::of::<Option<Vec<u64>>>() => Some(TaggedValue::LayerPath(*downcast(input).unwrap())),
			x if x == TypeId::of::<DAffine2>() => Some(TaggedValue::DAffine2(*downcast(input).unwrap())),
			x if x == TypeId::of::<LuminanceCalculation>() => Some(TaggedValue::LuminanceCalculation(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::VectorData>() => Some(TaggedValue::VectorData(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::Fill>() => Some(TaggedValue::Fill(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::Stroke>() => Some(TaggedValue::Stroke(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<f32>>() => Some(TaggedValue::VecF32(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::RedGreenBlue>() => Some(TaggedValue::RedGreenBlue(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::RelativeAbsolute>() => Some(TaggedValue::RelativeAbsolute(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::SelectiveColorChoice>() => Some(TaggedValue::SelectiveColorChoice(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::LineCap>() => Some(TaggedValue::LineCap(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::LineJoin>() => Some(TaggedValue::LineJoin(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::FillType>() => Some(TaggedValue::FillType(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::vector::style::GradientType>() => Some(TaggedValue::GradientType(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<(f64, Option<graphene_core::Color>)>>() => Some(TaggedValue::GradientPositions(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::quantization::QuantizationChannels>() => Some(TaggedValue::Quantization(*downcast(input).unwrap())),
			x if x == TypeId::of::<Option<graphene_core::Color>>() => Some(TaggedValue::OptionalColor(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<graphene_core::uuid::ManipulatorGroupId>>() => Some(TaggedValue::ManipulatorGroupIds(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::text::Font>() => Some(TaggedValue::Font(*downcast(input).unwrap())),
			x if x == TypeId::of::<Vec<DVec2>>() => Some(TaggedValue::VecDVec2(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::raster::IndexNode<Vec<graphene_core::raster::ImageFrame<Color>>>>() => Some(TaggedValue::Segments(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::EditorApi>() => Some(TaggedValue::EditorApi(*downcast(input).unwrap())),
			x if x == TypeId::of::<crate::document::DocumentNode>() => Some(TaggedValue::DocumentNode(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::GraphicGroup>() => Some(TaggedValue::GraphicGroup(*downcast(input).unwrap())),
			x if x == TypeId::of::<graphene_core::Artboard>() => Some(TaggedValue::Artboard(*downcast(input).unwrap())),
			x if x == TypeId::of::<Option<[glam::IVec2; 2]>>() => Some(TaggedValue::Optional2IVec2(*downcast(input).unwrap())),
			_ => None,
		}
	}
}

pub struct UpcastNode {
	value: TaggedValue,
}
impl<'input> Node<'input, Box<dyn DynAny<'input> + 'input>> for UpcastNode {
	type Output = Box<dyn DynAny<'input> + 'input>;

	fn eval(&'input self, _: Box<dyn DynAny<'input> + 'input>) -> Self::Output {
		self.value.clone().to_any()
	}
}
impl UpcastNode {
	pub fn new(value: TaggedValue) -> Self {
		Self { value }
	}
}

pub type Value<'a> = Box<dyn for<'i> ValueTrait<'i> + 'a>;

pub trait ValueTrait<'a>: DynAny<'a> + Upcast<dyn DynAny<'a> + 'a> + std::fmt::Debug + DynClone + Sync + Send + 'a {}

pub trait IntoValue<'a>: Sized + for<'i> ValueTrait<'i> + 'a {
	fn into_any(self) -> Value<'a> {
		Box::new(self)
	}
}

impl<'a, T: 'a + StaticType + Upcast<dyn DynAny<'a> + 'a> + std::fmt::Debug + PartialEq + Clone + Sync + Send + 'a> ValueTrait<'a> for T {}

impl<'a, T: for<'i> ValueTrait<'i> + 'a> IntoValue<'a> for T {}

#[repr(C)]
pub(crate) struct Vtable {
	pub(crate) destructor: unsafe fn(*mut ()),
	pub(crate) size: usize,
	pub(crate) align: usize,
}

#[repr(C)]
pub(crate) struct TraitObject {
	pub(crate) self_ptr: *mut u8,
	pub(crate) vtable: &'static Vtable,
}

impl<'a> PartialEq for Box<dyn for<'i> ValueTrait<'i> + 'a> {
	#[cfg_attr(miri, ignore)]
	fn eq(&self, other: &Self) -> bool {
		if self.type_id() != other.type_id() {
			return false;
		}
		let self_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(self.as_ref()) };
		let other_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(other.as_ref()) };
		let size = self_trait_object.vtable.size;
		let self_mem = unsafe { std::slice::from_raw_parts(self_trait_object.self_ptr, size) };
		let other_mem = unsafe { std::slice::from_raw_parts(other_trait_object.self_ptr, size) };
		self_mem == other_mem
	}
}

impl<'a> Hash for Value<'a> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		let self_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(self.as_ref()) };
		let size = self_trait_object.vtable.size;
		let self_mem = unsafe { std::slice::from_raw_parts(self_trait_object.self_ptr, size) };
		self_mem.hash(state);
	}
}

impl<'a> Clone for Value<'a> {
	fn clone(&self) -> Self {
		let self_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(self.as_ref()) };
		let size = self_trait_object.vtable.size;
		let self_mem = unsafe { std::slice::from_raw_parts(self_trait_object.self_ptr, size) }.to_owned();
		let ptr = Vec::leak(self_mem);
		unsafe {
			std::mem::transmute(TraitObject {
				self_ptr: ptr as *mut [u8] as *mut u8,
				vtable: self_trait_object.vtable,
			})
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	#[cfg_attr(miri, ignore)]
	fn test_any_src() {
		assert!(2_u32.into_any() == 2_u32.into_any());
		assert!(2_u32.into_any() != 3_u32.into_any());
		assert!(2_u32.into_any() != 3_i32.into_any());
	}
}
