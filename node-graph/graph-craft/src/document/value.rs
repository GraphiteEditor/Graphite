use super::DocumentNode;
pub use crate::imaginate_input::{ImaginateCache, ImaginateController, ImaginateMaskStartingFill, ImaginateSamplingMethod};
use crate::proto::{Any as DAny, FutureAny};
use crate::wasm_application_io::WasmEditorApi;

use dyn_any::DynAny;
pub use dyn_any::StaticType;
use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::{BlendMode, LuminanceCalculation};
use graphene_core::renderer::RenderMetadata;
use graphene_core::uuid::NodeId;
use graphene_core::vector::style::Fill;
use graphene_core::{Color, MemoHash, Node, Type};

pub use glam::{DAffine2, DVec2, IVec2, UVec2};
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::str::FromStr;
pub use std::sync::Arc;

/// Macro to generate the tagged value enum.
macro_rules! tagged_value {
	($ ($( #[$meta:meta] )* $identifier:ident ($ty:ty) ),* $(,)?) => {
		/// A type that is known, allowing serialization (serde::Deserialize is not object safe)
		#[derive(Clone, Debug, PartialEq)]
		#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
		#[allow(clippy::large_enum_variant)] // TODO(TrueDoctor): Properly solve this disparity between the size of the largest and next largest variants
		pub enum TaggedValue {
			None,
			$( $(#[$meta] ) *$identifier( $ty ), )*
			RenderOutput(RenderOutput),
			SurfaceFrame(graphene_core::SurfaceFrame),
			#[cfg_attr(feature = "serde", serde(skip))]
			EditorApi(Arc<WasmEditorApi>)
		}

		// We must manually implement hashing because some values are floats and so do not reproducibly hash (see FakeHash below)
		#[allow(clippy::derived_hash_with_manual_eq)]
		impl Hash for TaggedValue {
			fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
				core::mem::discriminant(self).hash(state);
				match self {
					Self::None => {}
					$( Self::$identifier(x) => {x.hash(state)}),*
					Self::RenderOutput(x) => x.hash(state),
					Self::SurfaceFrame(x) => x.hash(state),
					Self::EditorApi(x) => x.hash(state),
				}
			}
		}
		impl<'a> TaggedValue {
			/// Converts to a Box<dyn DynAny> - this isn't very neat but I'm not sure of a better approach
			pub fn to_any(self) -> DAny<'a> {
				match self {
					Self::None => Box::new(()),
					$( Self::$identifier(x) => Box::new(x), )*
					Self::RenderOutput(x) => Box::new(x),
					Self::SurfaceFrame(x) => Box::new(x),
					Self::EditorApi(x) => Box::new(x),
				}
			}
			/// Creates a graphene_core::Type::Concrete(TypeDescriptor { .. }) with the type of the value inside the tagged value
			pub fn ty(&self) -> Type {
				match self {
					Self::None => concrete!(()),
					$( Self::$identifier(_) => concrete!($ty), )*
					Self::RenderOutput(_) => concrete!(RenderOutput),
					Self::SurfaceFrame(_) => concrete!(graphene_core::SurfaceFrame),
					Self::EditorApi(_) => concrete!(&WasmEditorApi)
				}
			}
			/// Attempts to downcast the dynamic type to a tagged value
			pub fn try_from_any(input: Box<dyn DynAny<'a> + 'a>) -> Result<Self, String> {
				use dyn_any::downcast;
				use std::any::TypeId;

				match DynAny::type_id(input.as_ref()) {
					x if x == TypeId::of::<()>() => Ok(TaggedValue::None),
					$( x if x == TypeId::of::<$ty>() => Ok(TaggedValue::$identifier(*downcast(input).unwrap())), )*
					x if x == TypeId::of::<RenderOutput>() => Ok(TaggedValue::RenderOutput(*downcast(input).unwrap())),
					x if x == TypeId::of::<graphene_core::SurfaceFrame>() => Ok(TaggedValue::SurfaceFrame(*downcast(input).unwrap())),


					_ => Err(format!("Cannot convert {:?} to TaggedValue", DynAny::type_name(input.as_ref()))),
				}
			}
			pub fn from_type(input: &Type) -> Option<Self> {
				match input {
					Type::Generic(_) => {
						None
					}
					Type::Concrete(concrete_type) => {
						let internal_id = concrete_type.id?;
						use std::any::TypeId;
						// TODO: Add default implementations for types such as TaggedValue::Subpaths, and use the defaults here and in document_node_types
						// Tries using the default for the tagged value type. If it not implemented, then uses the default used in document_node_types. If it is not used there, then TaggedValue::None is returned.
						Some(match internal_id {
							x if x == TypeId::of::<()>() => TaggedValue::None,
							$( x if x == TypeId::of::<$ty>() => TaggedValue::$identifier(Default::default()), )*
							_ => return None,
						})
					}
					Type::Fn(_, output) => TaggedValue::from_type(output),
					Type::Future(_) => {
						None
					}
				}
			}
			pub fn from_type_or_none(input: &Type) -> Self {
				Self::from_type(input).unwrap_or(TaggedValue::None)
			}
		}
	};
}

tagged_value! {
	String(String),
	U32(u32),
	U64(u64),
	#[cfg_attr(feature = "serde", serde(alias = "F32"))] // TODO: Eventually remove this alias (probably starting late 2024)
	F64(f64),
	Bool(bool),
	UVec2(UVec2),
	IVec2(IVec2),
	DVec2(DVec2),
	OptionalDVec2(Option<DVec2>),
	DAffine2(DAffine2),
	Image(graphene_core::raster::Image<Color>),
	ImaginateCache(ImaginateCache),
	ImageFrame(graphene_core::raster::ImageFrame<Color>),
	Color(graphene_core::raster::color::Color),
	Subpaths(Vec<bezier_rs::Subpath<graphene_core::vector::PointId>>),
	BlendMode(BlendMode),
	LuminanceCalculation(LuminanceCalculation),
	ImaginateSamplingMethod(ImaginateSamplingMethod),
	ImaginateMaskStartingFill(ImaginateMaskStartingFill),
	ImaginateController(ImaginateController),
	VectorData(graphene_core::vector::VectorData),
	Fill(graphene_core::vector::style::Fill),
	Stroke(graphene_core::vector::style::Stroke),
	F64Array4([f64; 4]),
	#[cfg_attr(feature = "serde", serde(alias = "VecF32"))] // TODO: Eventually remove this alias (probably starting late 2024)
	VecF64(Vec<f64>),
	VecU64(Vec<u64>),
	NodePath(Vec<NodeId>),
	VecDVec2(Vec<DVec2>),
	RedGreenBlue(graphene_core::raster::RedGreenBlue),
	RedGreenBlueAlpha(graphene_core::raster::RedGreenBlueAlpha),
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
	FillChoice(graphene_core::vector::style::FillChoice),
	Gradient(graphene_core::vector::style::Gradient),
	GradientType(graphene_core::vector::style::GradientType),
	#[cfg_attr(feature = "serde", serde(alias = "GradientPositions"))] // TODO: Eventually remove this alias (probably starting late 2024)
	GradientStops(graphene_core::vector::style::GradientStops),
	OptionalColor(Option<graphene_core::raster::color::Color>),
	#[cfg_attr(feature = "serde", serde(alias = "ManipulatorGroupIds"))] // TODO: Eventually remove this alias (probably starting late 2024)
	PointIds(Vec<graphene_core::vector::PointId>),
	Font(graphene_core::text::Font),
	BrushStrokes(Vec<graphene_core::vector::brush_stroke::BrushStroke>),
	BrushCache(BrushCache),
	Segments(Vec<graphene_core::raster::ImageFrame<Color>>),
	DocumentNode(DocumentNode),
	GraphicGroup(graphene_core::GraphicGroup),
	GraphicElement(graphene_core::GraphicElement),
	ArtboardGroup(graphene_core::ArtboardGroup),
	Curve(graphene_core::raster::curve::Curve),
	Footprint(graphene_core::transform::Footprint),
	Palette(Vec<Color>),
	VectorModification(Box<graphene_core::vector::VectorModification>),
	CentroidType(graphene_core::vector::misc::CentroidType),
	BooleanOperation(graphene_core::vector::misc::BooleanOperation),
	FontCache(Arc<graphene_core::text::FontCache>),
}

impl TaggedValue {
	pub fn to_primitive_string(&self) -> String {
		match self {
			TaggedValue::None => "()".to_string(),
			TaggedValue::String(x) => format!("\"{x}\""),
			TaggedValue::U32(x) => x.to_string() + "_u32",
			TaggedValue::U64(x) => x.to_string() + "_u64",
			TaggedValue::F64(x) => x.to_string() + "_f64",
			TaggedValue::Bool(x) => x.to_string(),
			TaggedValue::BlendMode(x) => "BlendMode::".to_string() + &x.to_string(),
			TaggedValue::Color(x) => format!("Color {x:?}"),
			_ => panic!("Cannot convert to primitive string"),
		}
	}

	pub fn from_primitive_string(string: &str, ty: &Type) -> Option<Self> {
		fn to_dvec2(input: &str) -> Option<DVec2> {
			let mut split = input.split(',');
			let x = split.next()?.trim().parse().ok()?;
			let y = split.next()?.trim().parse().ok()?;
			Some(DVec2::new(x, y))
		}

		fn to_color(input: &str) -> Option<Color> {
			// String syntax (e.g. "000000ff")
			if input.starts_with('"') && input.ends_with('"') {
				let color = input.trim().trim_matches('"').trim().trim_start_matches('#');
				match color.len() {
					6 => return Color::from_rgb_str(color),
					8 => return Color::from_rgba_str(color),
					_ => {
						log::error!("Invalid default value color string: {}", input);
						return None;
					}
				}
			}

			// Color constant syntax (e.g. Color::BLACK)
			let mut choices = input.split("::");
			let (first, second) = (choices.next()?.trim(), choices.next()?.trim());
			if first == "Color" {
				return Some(match second {
					"BLACK" => Color::BLACK,
					"WHITE" => Color::WHITE,
					"RED" => Color::RED,
					"GREEN" => Color::GREEN,
					"BLUE" => Color::BLUE,
					"YELLOW" => Color::YELLOW,
					"CYAN" => Color::CYAN,
					"MAGENTA" => Color::MAGENTA,
					"TRANSPARENT" => Color::TRANSPARENT,
					_ => {
						log::error!("Invalid default value color constant: {}", input);
						return None;
					}
				});
			}

			log::error!("Invalid default value color: {}", input);
			None
		}

		match ty {
			Type::Generic(_) => None,
			Type::Concrete(concrete_type) => {
				let internal_id = concrete_type.id?;
				use std::any::TypeId;
				// TODO: Add default implementations for types such as TaggedValue::Subpaths, and use the defaults here and in document_node_types
				// Tries using the default for the tagged value type. If it not implemented, then uses the default used in document_node_types. If it is not used there, then TaggedValue::None is returned.
				let ty = match internal_id {
					x if x == TypeId::of::<()>() => TaggedValue::None,
					x if x == TypeId::of::<String>() => TaggedValue::String(string.into()),
					x if x == TypeId::of::<f64>() => FromStr::from_str(string).map(TaggedValue::F64).ok()?,
					x if x == TypeId::of::<u64>() => FromStr::from_str(string).map(TaggedValue::U64).ok()?,
					x if x == TypeId::of::<u32>() => FromStr::from_str(string).map(TaggedValue::U32).ok()?,
					x if x == TypeId::of::<DVec2>() => to_dvec2(string).map(TaggedValue::DVec2)?,
					x if x == TypeId::of::<bool>() => FromStr::from_str(string).map(TaggedValue::Bool).ok()?,
					x if x == TypeId::of::<Color>() => to_color(string).map(TaggedValue::Color)?,
					x if x == TypeId::of::<Option<Color>>() => to_color(string).map(|color| TaggedValue::OptionalColor(Some(color)))?,
					x if x == TypeId::of::<Fill>() => to_color(string).map(|color| TaggedValue::Fill(Fill::solid(color)))?,
					_ => return None,
				};
				Some(ty)
			}
			Type::Fn(_, output) => TaggedValue::from_primitive_string(string, output),
			Type::Future(_) => None,
		}
	}

	pub fn to_u32(&self) -> u32 {
		match self {
			TaggedValue::U32(x) => *x,
			_ => panic!("Passed value is not of type u32"),
		}
	}
}

impl Display for TaggedValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			TaggedValue::String(x) => f.write_str(x),
			TaggedValue::U32(x) => f.write_fmt(format_args!("{x}")),
			TaggedValue::U64(x) => f.write_fmt(format_args!("{x}")),
			TaggedValue::F64(x) => f.write_fmt(format_args!("{x}")),
			TaggedValue::Bool(x) => f.write_fmt(format_args!("{x}")),
			_ => panic!("Cannot convert to string"),
		}
	}
}

pub struct UpcastNode {
	value: MemoHash<TaggedValue>,
}
impl<'input> Node<'input, DAny<'input>> for UpcastNode {
	type Output = FutureAny<'input>;

	fn eval(&'input self, _: DAny<'input>) -> Self::Output {
		Box::pin(async move { self.value.clone().into_inner().to_any() })
	}
}
impl UpcastNode {
	pub fn new(value: MemoHash<TaggedValue>) -> Self {
		Self { value }
	}
}
#[derive(Default, Debug, Clone, Copy)]
pub struct UpcastAsRefNode<T: AsRef<U> + Sync + Send, U: Sync + Send>(pub T, PhantomData<U>);

impl<'i, T: 'i + AsRef<U> + Sync + Send, U: 'i + StaticType + Sync + Send> Node<'i, DAny<'i>> for UpcastAsRefNode<T, U> {
	type Output = FutureAny<'i>;
	#[inline(always)]
	fn eval(&'i self, _: DAny<'i>) -> Self::Output {
		Box::pin(async move { Box::new(self.0.as_ref()) as DAny<'i> })
	}
}

impl<T: AsRef<U> + Sync + Send, U: Sync + Send> UpcastAsRefNode<T, U> {
	pub const fn new(value: T) -> UpcastAsRefNode<T, U> {
		UpcastAsRefNode(value, PhantomData)
	}
}

#[derive(Debug, Clone, PartialEq, dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RenderOutput {
	pub data: RenderOutputType,
	pub metadata: RenderMetadata,
}

#[derive(Debug, Clone, PartialEq, dyn_any::DynAny, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderOutputType {
	CanvasFrame(graphene_core::SurfaceFrame),
	Svg(String),
	Image(Vec<u8>),
}

impl Hash for RenderOutput {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.data.hash(state)
	}
}

/// We hash the floats and so-forth despite it not being reproducible because all inputs to the node graph must be hashed otherwise the graph execution breaks (so sorry about this hack)
trait FakeHash {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H);
}
mod fake_hash {
	use super::*;
	impl FakeHash for f64 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_bits().hash(state)
		}
	}
	impl FakeHash for DVec2 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_array().iter().for_each(|x| x.to_bits().hash(state))
		}
	}
	impl FakeHash for DAffine2 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_cols_array().iter().for_each(|x| x.to_bits().hash(state))
		}
	}
	impl<X: FakeHash> FakeHash for Option<X> {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			if let Some(x) = self {
				1.hash(state);
				x.hash(state);
			} else {
				0.hash(state);
			}
		}
	}
	impl<X: FakeHash> FakeHash for Vec<X> {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.len().hash(state);
			self.iter().for_each(|x| x.hash(state))
		}
	}
	impl<T: FakeHash, const N: usize> FakeHash for [T; N] {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.iter().for_each(|x| x.hash(state))
		}
	}
	impl FakeHash for (f64, Color) {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.0.to_bits().hash(state);
			self.1.hash(state)
		}
	}
}
