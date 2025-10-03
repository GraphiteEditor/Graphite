use super::DocumentNode;
use crate::proto::{Any as DAny, FutureAny};
use crate::wasm_application_io::WasmEditorApi;
use dyn_any::DynAny;
pub use dyn_any::StaticType;
use glam::{Affine2, Vec2};
pub use glam::{DAffine2, DVec2, IVec2, UVec2};
use graphene_application_io::{ImageTexture, SurfaceFrame};
use graphene_brush::brush_cache::BrushCache;
use graphene_brush::brush_stroke::BrushStroke;
use graphene_core::raster::Image;
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::table::Table;
use graphene_core::transform::ReferencePoint;
use graphene_core::uuid::NodeId;
use graphene_core::vector::Vector;
use graphene_core::vector::style::Fill;
use graphene_core::vector::style::GradientStops;
use graphene_core::{Artboard, Color, ContextFeatures, Graphic, MemoHash, Node, Type};
use graphene_svg_renderer::RenderMetadata;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::str::FromStr;
pub use std::sync::Arc;

pub struct TaggedValueTypeError;

/// Macro to generate the tagged value enum.
macro_rules! tagged_value {
	($ ($( #[$meta:meta] )* $identifier:ident ($ty:ty) ),* $(,)?) => {
		/// A type that is known, allowing serialization (serde::Deserialize is not object safe)
		#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
		#[allow(clippy::large_enum_variant)] // TODO(TrueDoctor): Properly solve this disparity between the size of the largest and next largest variants
		pub enum TaggedValue {
			None,
			$( $(#[$meta] ) *$identifier( $ty ), )*
			RenderOutput(RenderOutput),
			SurfaceFrame(SurfaceFrame),
			#[serde(skip)]
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
			/// Converts to a Box<dyn DynAny>
			pub fn to_dynany(self) -> DAny<'a> {
				match self {
					Self::None => Box::new(()),
					$( Self::$identifier(x) => Box::new(x), )*
					Self::RenderOutput(x) => Box::new(x),
					Self::SurfaceFrame(x) => Box::new(x),
					Self::EditorApi(x) => Box::new(x),
				}
			}
			/// Converts to a Arc<dyn Any + Send + Sync + 'static>
			pub fn to_any(self) -> Arc<dyn std::any::Any + Send + Sync + 'static> {
				match self {
					Self::None => Arc::new(()),
					$( Self::$identifier(x) => Arc::new(x), )*
					Self::RenderOutput(x) => Arc::new(x),
					Self::SurfaceFrame(x) => Arc::new(x),
					Self::EditorApi(x) => Arc::new(x),
				}
			}
			/// Creates a graphene_core::Type::Concrete(TypeDescriptor { .. }) with the type of the value inside the tagged value
			pub fn ty(&self) -> Type {
				match self {
					Self::None => concrete!(()),
					$( Self::$identifier(_) => concrete!($ty), )*
					Self::RenderOutput(_) => concrete!(RenderOutput),
					Self::SurfaceFrame(_) => concrete!(SurfaceFrame),
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
					x if x == TypeId::of::<SurfaceFrame>() => Ok(TaggedValue::SurfaceFrame(*downcast(input).unwrap())),


					_ => Err(format!("Cannot convert {:?} to TaggedValue", DynAny::type_name(input.as_ref()))),
				}
			}
			/// Attempts to downcast the dynamic type to a tagged value
			pub fn try_from_std_any_ref(input: &dyn std::any::Any) -> Result<Self, String> {
				use std::any::TypeId;

				match input.type_id() {
					x if x == TypeId::of::<()>() => Ok(TaggedValue::None),
					$( x if x == TypeId::of::<$ty>() => Ok(TaggedValue::$identifier(<$ty as Clone>::clone(input.downcast_ref().unwrap()))), )*
					x if x == TypeId::of::<RenderOutput>() => Ok(TaggedValue::RenderOutput(RenderOutput::clone(input.downcast_ref().unwrap()))),
					x if x == TypeId::of::<SurfaceFrame>() => Ok(TaggedValue::SurfaceFrame(SurfaceFrame::clone(input.downcast_ref().unwrap()))),
					_ => Err(format!("Cannot convert {:?} to TaggedValue",std::any::type_name_of_val(input))),
				}
			}
			pub fn from_type(input: &Type) -> Option<Self> {
				match input {
					Type::Generic(_) => None,
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
					Type::Future(output) => {
						TaggedValue::from_type(output)
					}
				}
			}
			pub fn from_type_or_none(input: &Type) -> Self {
				Self::from_type(input).unwrap_or(TaggedValue::None)
			}
		}

		$(
			impl From<$ty> for TaggedValue {
				fn from(value: $ty) -> Self {
					Self::$identifier(value)
				}
			}
		)*

		$(
			impl<'a> TryFrom<&'a TaggedValue> for &'a $ty {
				type Error = TaggedValueTypeError;
				fn try_from(value: &'a TaggedValue) -> Result<Self, Self::Error> {
					match value{
						TaggedValue::$identifier(value) => Ok(value),
						_ => Err(TaggedValueTypeError),
					}
				}
			}
		)*
	};
}

tagged_value! {
	// ===============
	// PRIMITIVE TYPES
	// ===============
	F32(f32),
	F64(f64),
	U32(u32),
	U64(u64),
	Bool(bool),
	String(String),
	OptionalF64(Option<f64>),
	ColorNotInTable(Color),
	OptionalColorNotInTable(Option<Color>),
	// ========================
	// LISTS OF PRIMITIVE TYPES
	// ========================
	#[serde(alias = "VecF32")] // TODO: Eventually remove this alias document upgrade code
	VecF64(Vec<f64>),
	VecDVec2(Vec<DVec2>),
	F64Array4([f64; 4]),
	NodePath(Vec<NodeId>),
	// ===========
	// TABLE TYPES
	// ===========
	GraphicUnused(Graphic), // TODO: This is unused but removing it causes `cargo test` to infinitely recurse its type solving; figure out why and then remove this
	#[cfg_attr(target_family = "wasm", serde(deserialize_with = "graphene_core::vector::migrate_vector"))] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "VectorData")]
	Vector(Table<Vector>),
	#[cfg_attr(target_family = "wasm", serde(deserialize_with = "graphene_core::raster::image::migrate_image_frame"))] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "ImageFrame", alias = "RasterData", alias = "Image")]
	Raster(Table<Raster<CPU>>),
	#[cfg_attr(target_family = "wasm", serde(deserialize_with = "graphene_core::graphic::migrate_graphic"))] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "GraphicGroup", alias = "Group")]
	Graphic(Table<Graphic>),
	#[cfg_attr(target_family = "wasm", serde(deserialize_with = "graphene_core::artboard::migrate_artboard"))] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "ArtboardGroup")]
	Artboard(Table<Artboard>),
	#[cfg_attr(target_family = "wasm", serde(deserialize_with = "graphene_core::misc::migrate_color"))] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "ColorTable", alias = "OptionalColor")]
	Color(Table<Color>),
	GradientTable(Table<GradientStops>),
	// ============
	// STRUCT TYPES
	// ============
	FVec2(Vec2),
	FAffine2(Affine2),
	#[serde(alias = "IVec2", alias = "UVec2")]
	DVec2(DVec2),
	DAffine2(DAffine2),
	Stroke(graphene_core::vector::style::Stroke),
	Gradient(graphene_core::vector::style::Gradient),
	#[serde(alias = "GradientPositions")] // TODO: Eventually remove this alias document upgrade code
	GradientStops(GradientStops),
	Font(graphene_core::text::Font),
	BrushStrokes(Vec<BrushStroke>),
	BrushCache(BrushCache),
	DocumentNode(DocumentNode),
	ContextFeatures(ContextFeatures),
	Curve(graphene_raster_nodes::curve::Curve),
	Footprint(graphene_core::transform::Footprint),
	VectorModification(Box<graphene_core::vector::VectorModification>),
	// ==========
	// ENUM TYPES
	// ==========
	Fill(graphene_core::vector::style::Fill),
	BlendMode(graphene_core::blending::BlendMode),
	LuminanceCalculation(graphene_raster_nodes::adjustments::LuminanceCalculation),
	XY(graphene_core::extract_xy::XY),
	RedGreenBlue(graphene_raster_nodes::adjustments::RedGreenBlue),
	RedGreenBlueAlpha(graphene_raster_nodes::adjustments::RedGreenBlueAlpha),
	RealTimeMode(graphene_core::animation::RealTimeMode),
	NoiseType(graphene_raster_nodes::adjustments::NoiseType),
	FractalType(graphene_raster_nodes::adjustments::FractalType),
	CellularDistanceFunction(graphene_raster_nodes::adjustments::CellularDistanceFunction),
	CellularReturnType(graphene_raster_nodes::adjustments::CellularReturnType),
	DomainWarpType(graphene_raster_nodes::adjustments::DomainWarpType),
	RelativeAbsolute(graphene_raster_nodes::adjustments::RelativeAbsolute),
	SelectiveColorChoice(graphene_raster_nodes::adjustments::SelectiveColorChoice),
	GridType(graphene_core::vector::misc::GridType),
	ArcType(graphene_core::vector::misc::ArcType),
	MergeByDistanceAlgorithm(graphene_core::vector::misc::MergeByDistanceAlgorithm),
	PointSpacingType(graphene_core::vector::misc::PointSpacingType),
	SpiralType(graphene_core::vector::misc::SpiralType),
	#[serde(alias = "LineCap")]
	StrokeCap(graphene_core::vector::style::StrokeCap),
	#[serde(alias = "LineJoin")]
	StrokeJoin(graphene_core::vector::style::StrokeJoin),
	StrokeAlign(graphene_core::vector::style::StrokeAlign),
	PaintOrder(graphene_core::vector::style::PaintOrder),
	FillType(graphene_core::vector::style::FillType),
	GradientType(graphene_core::vector::style::GradientType),
	ReferencePoint(graphene_core::transform::ReferencePoint),
	CentroidType(graphene_core::vector::misc::CentroidType),
	BooleanOperation(graphene_path_bool::BooleanOperation),
	TextAlign(graphene_core::text::TextAlign),
}

impl TaggedValue {
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
						log::error!("Invalid default value color string: {input}");
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
						log::error!("Invalid default value color constant: {input}");
						return None;
					}
				});
			}

			log::error!("Invalid default value color: {input}");
			None
		}

		fn to_reference_point(input: &str) -> Option<ReferencePoint> {
			let mut choices = input.split("::");
			let (first, second) = (choices.next()?.trim(), choices.next()?.trim());
			if first == "ReferencePoint" {
				return Some(match second {
					"None" => ReferencePoint::None,
					"TopLeft" => ReferencePoint::TopLeft,
					"TopCenter" => ReferencePoint::TopCenter,
					"TopRight" => ReferencePoint::TopRight,
					"CenterLeft" => ReferencePoint::CenterLeft,
					"Center" => ReferencePoint::Center,
					"CenterRight" => ReferencePoint::CenterRight,
					"BottomLeft" => ReferencePoint::BottomLeft,
					"BottomCenter" => ReferencePoint::BottomCenter,
					"BottomRight" => ReferencePoint::BottomRight,
					_ => {
						log::error!("Invalid ReferencePoint default type variant: {input}");
						return None;
					}
				});
			}

			log::error!("Invalid ReferencePoint default type: {input}");
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
					x if x == TypeId::of::<f32>() => FromStr::from_str(string).map(TaggedValue::F32).ok()?,
					x if x == TypeId::of::<u64>() => FromStr::from_str(string).map(TaggedValue::U64).ok()?,
					x if x == TypeId::of::<u32>() => FromStr::from_str(string).map(TaggedValue::U32).ok()?,
					x if x == TypeId::of::<DVec2>() => to_dvec2(string).map(TaggedValue::DVec2)?,
					x if x == TypeId::of::<bool>() => FromStr::from_str(string).map(TaggedValue::Bool).ok()?,
					x if x == TypeId::of::<Table<Color>>() => to_color(string).map(|color| TaggedValue::Color(Table::new_from_element(color)))?,
					x if x == TypeId::of::<Color>() => to_color(string).map(TaggedValue::ColorNotInTable)?,
					x if x == TypeId::of::<Option<Color>>() => TaggedValue::ColorNotInTable(to_color(string)?),
					x if x == TypeId::of::<Fill>() => to_color(string).map(|color| TaggedValue::Fill(Fill::solid(color)))?,
					x if x == TypeId::of::<ReferencePoint>() => to_reference_point(string).map(TaggedValue::ReferencePoint)?,
					_ => return None,
				};
				Some(ty)
			}
			Type::Fn(_, output) => TaggedValue::from_primitive_string(string, output),
			Type::Future(fut) => TaggedValue::from_primitive_string(string, fut),
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
			TaggedValue::F32(x) => f.write_fmt(format_args!("{x}")),
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
		let memo_clone = MemoHash::clone(&self.value);
		Box::pin(async move { memo_clone.into_inner().as_ref().clone().to_dynany() })
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

#[derive(Debug, Clone, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize)]
pub struct RenderOutput {
	pub data: RenderOutputType,
	pub metadata: RenderMetadata,
}

#[derive(Debug, Clone, Hash, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize)]
pub enum RenderOutputType {
	CanvasFrame(SurfaceFrame),
	#[serde(skip)]
	Texture(ImageTexture),
	#[serde(skip)]
	Buffer {
		data: Vec<u8>,
		width: u32,
		height: u32,
	},
	Svg {
		svg: String,
		image_data: Vec<(u64, Image<Color>)>,
	},
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
	impl FakeHash for f32 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_bits().hash(state)
		}
	}
	impl FakeHash for DVec2 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_array().iter().for_each(|x| x.to_bits().hash(state))
		}
	}
	impl FakeHash for Vec2 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_array().iter().for_each(|x| x.to_bits().hash(state))
		}
	}
	impl FakeHash for DAffine2 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_cols_array().iter().for_each(|x| x.to_bits().hash(state))
		}
	}
	impl FakeHash for Affine2 {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			self.to_cols_array().iter().for_each(|x| x.to_bits().hash(state))
		}
	}
	impl<T: FakeHash> FakeHash for Option<T> {
		fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
			if let Some(x) = self {
				1.hash(state);
				x.hash(state);
			} else {
				0.hash(state);
			}
		}
	}
	impl<T: FakeHash> FakeHash for Vec<T> {
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

#[test]
fn can_construct_color() {
	assert_eq!(TaggedValue::from_type(&concrete!(Color)).unwrap(), TaggedValue::ColorNotInTable(Color::default()));
}
