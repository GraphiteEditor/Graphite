use super::DocumentNode;
use crate::application_io::PlatformEditorApi;
use crate::proto::{Any as DAny, FutureAny};
use brush_nodes::brush_stroke::BrushStroke;
use core_types::table::Table;
use core_types::transform::Footprint;
use core_types::uuid::NodeId;
use core_types::{CacheHash, Color, ContextFeatures, MemoHash, Node, Type, TypeDescriptor};
use dyn_any::DynAny;
pub use dyn_any::StaticType;
pub use glam::{DAffine2, DVec2, IVec2, UVec2};
use graphic_types::raster_types::{CPU, Image, Raster};
use graphic_types::vector_types::vector::style::{Fill, Gradient, GradientStops};
use graphic_types::vector_types::vector::{self, ReferencePoint};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::RenderMetadata;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::str::FromStr;
pub use std::sync::Arc;
use text_nodes::Font;
use text_nodes::vector_types::GradientStop;
use vector::VectorModification;

pub struct TaggedValueTypeError;

/// Macro to generate the tagged value enum.
macro_rules! tagged_value {
	($ ($( #[$meta:meta] )* $identifier:ident ($ty:ty) ),* $(,)?) => {
		/// A type that is known, allowing serialization (serde::Deserialize is not object safe)
		#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
		#[allow(clippy::large_enum_variant)] // TODO(TrueDoctor): Properly solve this disparity between the size of the largest and next largest variants
		pub enum TaggedValue {
			// ===============
			// MANUAL VARIANTS
			// ===============
			None,
			/// Stores a type, from which its `Default::default()` value can be obtained, rather than storing an actual type's value.
			/// Example: `TaggedValue::TypeDefault(descriptor!(String))` stores the type `String` but no specific string value.
			TypeDefault(TypeDescriptor),
			/// Stored compactly as a `Vec<f64>`, materializes as `Table<f64>` at runtime via `to_dynany`/`to_any`. Aliases recover legacy on-disk shapes.
			#[serde(deserialize_with = "core_types::misc::migrate_to_f64_array")] // TODO: Eventually remove this migration document upgrade code
			#[serde(alias = "F64Table", alias = "VecF64", alias = "VecF32", alias = "F64Array4")]
			F64Array(Vec<f64>),
			/// Stored compactly as an `Option<Color>`, materializes as `Table<Color>` at runtime via `to_dynany`/`to_any`. Aliases recover legacy on-disk shapes.
			#[serde(deserialize_with = "core_types::misc::migrate_to_optional_color")] // TODO: Eventually remove this migration document upgrade code
			#[serde(alias = "ColorTable", alias = "OptionalColor", alias = "ColorNotInTable")]
			Color(Option<Color>),
			/// Stored compactly as a `GradientStops`, materializes as a single-row `Table<GradientStops>` at runtime via `to_dynany`/`to_any`. Aliases recover legacy on-disk shapes.
			/// (Old documents that stored a full `Gradient` struct under this same `"Gradient"` tag are routed to `FillGradient` by `deserialize_tagged_value_with_legacy_migration`.)
			#[serde(deserialize_with = "graphic_types::vector_types::gradient::migrate_to_gradient_stops")] // TODO: Eventually remove this migration document upgrade code
			#[serde(alias = "GradientTable", alias = "GradientPositions")]
			Gradient(GradientStops),
			/// Stored compactly as a `Vec<BrushStroke>`, materializes as `Table<BrushStroke>` at runtime via `to_dynany`/`to_any`. Aliases recover legacy on-disk shapes.
			#[serde(deserialize_with = "brush_nodes::migrations::migrate_to_brush_strokes")] // TODO: Eventually remove this migration document upgrade code
			#[serde(alias = "BrushStrokeTable")]
			BrushStrokes(Vec<BrushStroke>),
			// =======================
			// AUTO-GENERATED VARIANTS
			// =======================
			$( $(#[$meta] ) *$identifier( $ty ), )*
			// =======================
			// NON-SERIALIZED VARIANTS
			// =======================
			#[serde(skip)]
			RenderOutput(RenderOutput),
			/// Path to the consumer of a `NodeInput::Reflection(DocumentNodePath)`. Materializes a `Table<NodeId>` at runtime via `to_dynany`/`to_any` during graph flattening.
			#[serde(skip)]
			NodeIdPath(Vec<NodeId>),
			/// The `DocumentNode` value carried by an `Extract` proto node, populated at flatten time by `resolve_extract_nodes`. The on-disk placeholder uses `TypeDefault(descriptor!(DocumentNode))`.
			#[serde(skip)]
			DocumentNode(DocumentNode),
			#[serde(skip)]
			EditorApi(Arc<PlatformEditorApi>),
		}

		impl CacheHash for TaggedValue {
			fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
				core::mem::discriminant(self).hash(state);
				match self {
					// ===============
					// MANUAL VARIANTS
					// ===============
					Self::None => {}
					Self::TypeDefault(td) => td.cache_hash(state),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(x) => { x.cache_hash(state) }),*
					Self::F64Array(values) => values.cache_hash(state),
					Self::Color(color) => color.cache_hash(state),
					Self::Gradient(stops) => stops.cache_hash(state),
					Self::BrushStrokes(strokes) => strokes.cache_hash(state),
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::NodeIdPath(path) => path.hash(state),
					Self::DocumentNode(node) => node.cache_hash(state),
					Self::RenderOutput(x) => x.cache_hash(state),
					Self::EditorApi(x) => x.cache_hash(state),
				}
			}
		}

		impl<'a> TaggedValue {
			/// Converts to a Box<dyn DynAny>
			pub fn to_dynany(self) -> DAny<'a> {
				match self {
					// ===============
					// MANUAL VARIANTS
					// ===============
					Self::None => Box::new(()),
					Self::TypeDefault(td) => {
						// Construct the actual default for types without a `TaggedValue` variant directly, instead of going through
						// `from_type_or_none` (which would just return `TypeDefault` again and recurse forever).
						let name = td.name.as_ref();
						if name == std::any::type_name::<Table<Graphic>>() { return Box::new(Table::<Graphic>::default()); }
						if name == std::any::type_name::<Table<Artboard>>() { return Box::new(Table::<Artboard>::default()); }
						if name == std::any::type_name::<Table<Raster<CPU>>>() { return Box::new(Table::<Raster<CPU>>::default()); }
						if name == std::any::type_name::<Table<Vector>>() { return Box::new(Table::<Vector>::default()); }
						if name == std::any::type_name::<Table<String>>() { return Box::new(Table::<String>::default()); }
						if name == std::any::type_name::<DocumentNode>() { return Box::new(DocumentNode::default()); }
						Self::from_type_or_none(&Type::Concrete(td)).to_dynany()
					}
					Self::F64Array(values) => {
						let table: Table<f64> = values.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Box::new(table)
					}
					Self::Color(color) => {
						let table: Table<Color> = color.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Box::new(table)
					}
					Self::Gradient(stops) => Box::new(Table::<GradientStops>::new_from_element(stops)),
					Self::BrushStrokes(strokes) => {
						let table: Table<BrushStroke> = strokes.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Box::new(table)
					}
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(x) => Box::new(x), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::RenderOutput(x) => Box::new(x),
					Self::NodeIdPath(path) => {
						let table: Table<NodeId> = path.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Box::new(table)
					}
					Self::DocumentNode(node) => Box::new(node),
					Self::EditorApi(x) => Box::new(x),
				}
			}

			/// Converts to a Arc<dyn Any + Send + Sync + 'static>
			pub fn to_any(self) -> Arc<dyn std::any::Any + Send + Sync + 'static> {
				match self {
					// ===============
					// MANUAL VARIANTS
					// ===============
					Self::None => Arc::new(()),
					Self::TypeDefault(td) => {
						// Same direct-construction path as `to_dynany` for the same reason as in `to_dynany`.
						let name = td.name.as_ref();
						if name == std::any::type_name::<Table<Graphic>>() { return Arc::new(Table::<Graphic>::default()); }
						if name == std::any::type_name::<Table<Artboard>>() { return Arc::new(Table::<Artboard>::default()); }
						if name == std::any::type_name::<Table<Raster<CPU>>>() { return Arc::new(Table::<Raster<CPU>>::default()); }
						if name == std::any::type_name::<Table<Vector>>() { return Arc::new(Table::<Vector>::default()); }
						if name == std::any::type_name::<Table<String>>() { return Arc::new(Table::<String>::default()); }
						if name == std::any::type_name::<DocumentNode>() { return Arc::new(DocumentNode::default()); }
						Self::from_type_or_none(&Type::Concrete(td)).to_any()
					}
					Self::F64Array(values) => {
						let table: Table<f64> = values.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Arc::new(table)
					}
					Self::Color(color) => {
						let table: Table<Color> = color.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Arc::new(table)
					}
					Self::Gradient(stops) => Arc::new(Table::<GradientStops>::new_from_element(stops)),
					Self::BrushStrokes(strokes) => {
						let table: Table<BrushStroke> = strokes.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Arc::new(table)
					}
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(x) => Arc::new(x), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::RenderOutput(x) => Arc::new(x),
					Self::NodeIdPath(path) => {
						let table: Table<NodeId> = path.into_iter().map(core_types::table::TableRow::new_from_element).collect();
						Arc::new(table)
					}
					Self::DocumentNode(node) => Arc::new(node),
					Self::EditorApi(x) => Arc::new(x),
				}
			}

			/// Creates a core_types::Type::Concrete(TypeDescriptor { .. }) with the type of the value inside the tagged value
			pub fn ty(&self) -> Type {
				match self {
					// ===============
					// MANUAL VARIANTS
					// ===============
					Self::None => concrete!(()),
					Self::TypeDefault(td) => Type::Concrete(td.clone()),
					Self::F64Array(_) => concrete!(Table<f64>),
					Self::Color(_) => concrete!(Table<Color>),
					Self::Gradient(_) => concrete!(Table<GradientStops>),
					Self::BrushStrokes(_) => concrete!(Table<BrushStroke>),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(_) => concrete!($ty), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::RenderOutput(_) => concrete!(RenderOutput),
					Self::NodeIdPath(_) => concrete!(Table<NodeId>),
					Self::DocumentNode(_) => concrete!(DocumentNode),
					Self::EditorApi(_) => concrete!(&PlatformEditorApi),
				}
			}

			/// Attempts to downcast the dynamic type to a tagged value
			pub fn try_from_any(input: Box<dyn DynAny<'a> + 'a>) -> Result<Self, String> {
				use dyn_any::downcast;
				use std::any::TypeId;

				match DynAny::type_id(input.as_ref()) {
					// ===============
					// MANUAL VARIANTS
					// ===============
					x if x == TypeId::of::<()>() => Ok(TaggedValue::None),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( x if x == TypeId::of::<$ty>() => Ok(TaggedValue::$identifier(*downcast(input).unwrap())), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					x if x == TypeId::of::<RenderOutput>() => Ok(TaggedValue::RenderOutput(*downcast(input).unwrap())),

					_ => Err(format!("Cannot convert {:?} to TaggedValue", DynAny::type_name(input.as_ref()))),
				}
			}

			/// Attempts to downcast the dynamic type to a tagged value
			pub fn try_from_std_any_ref(input: &dyn std::any::Any) -> Result<Self, String> {
				use std::any::TypeId;

				match input.type_id() {
					// ===============
					// MANUAL VARIANTS
					// ===============
					x if x == TypeId::of::<()>() => Ok(TaggedValue::None),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( x if x == TypeId::of::<$ty>() => Ok(TaggedValue::$identifier(<$ty as Clone>::clone(input.downcast_ref().unwrap()))), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					x if x == TypeId::of::<RenderOutput>() => Ok(TaggedValue::RenderOutput(RenderOutput::clone(input.downcast_ref().unwrap()))),
					_ => Err(format!("Cannot convert {:?} to TaggedValue", std::any::type_name_of_val(input))),
				}
			}

			/// Returns a TaggedValue from the type, where that value is its type's `Default::default()`.
			/// Dispatches by the type's name (the field that round-trips through serde) so it works for both
			/// freshly constructed types and types deserialized from disk where the runtime `TypeId` is unavailable.
			pub fn from_type(input: &Type) -> Option<Self> {
				match input {
					Type::Generic(_) => None,
					Type::Concrete(concrete_type) => {
						let name = concrete_type.name.as_ref();
						// TODO: Add default implementations for types such as TaggedValue::Subpaths, and use the defaults here and in document_node_types
						// Tries using the default for the tagged value type. If it not implemented, then uses the default used in document_node_types. If it is not used there, then TaggedValue::None is returned.
						if name == std::any::type_name::<()>() { return Some(TaggedValue::None) }
						// Table-wrapped types need a single-item default with the element's default, not an empty table
						if name == std::any::type_name::<Table<Color>>() { return Some(TaggedValue::Color(Some(Color::default()))) }
						if name == std::any::type_name::<Table<GradientStops>>() { return Some(TaggedValue::Gradient(GradientStops::default())) }
						$( if name == std::any::type_name::<$ty>() { return Some(TaggedValue::$identifier(Default::default())) } )*
						if name == std::any::type_name::<Table<f64>>() { return Some(TaggedValue::F64Array(Vec::new())) }
						if name == std::any::type_name::<Table<BrushStroke>>() { return Some(TaggedValue::BrushStrokes(Vec::new())) }
						// Types whose `TaggedValue` variant has been removed. They route through `TypeDefault` instead, with `to_dynany`/`to_any` constructing the actual default at execution time.
						if name == std::any::type_name::<Table<Graphic>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						if name == std::any::type_name::<Table<Artboard>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						if name == std::any::type_name::<Table<Raster<CPU>>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						if name == std::any::type_name::<Table<Vector>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						if name == std::any::type_name::<Table<String>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						if name == std::any::type_name::<DocumentNode>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						None
					}
					Type::Fn(_, output) => TaggedValue::from_type(output),
					Type::Future(output) => TaggedValue::from_type(output),
				}
			}

			pub fn from_type_or_none(input: &Type) -> Self {
				Self::from_type(input).unwrap_or(TaggedValue::None)
			}

			pub fn to_debug_string(&self) -> String {
				match self {
					// ===============
					// MANUAL VARIANTS
					// ===============
					Self::None => "()".to_string(),
					Self::TypeDefault(td) => format!("TypeDefault({})", td.name),
					Self::F64Array(values) => format!("F64Array({values:?})"),
					Self::Color(color) => format!("Color({color:?})"),
					Self::Gradient(stops) => format!("Gradient({stops:?})"),
					Self::BrushStrokes(strokes) => format!("BrushStrokes({strokes:?})"),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(x) => format!("{:?}", x), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::RenderOutput(_) => "RenderOutput".to_string(),
					Self::NodeIdPath(path) => format!("NodeIdPath({path:?})"),
					Self::DocumentNode(node) => format!("DocumentNode({node:?})"),
					Self::EditorApi(_) => "PlatformEditorApi".to_string(),
				}
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
	#[serde(alias = "IVec2", alias = "UVec2", alias = "Vec2")]
	DVec2(DVec2),
	#[serde(alias = "Affine2")]
	DAffine2(DAffine2),
	FillGradient(Gradient),
	Font(Font),
	ContextFeatures(ContextFeatures),
	Footprint(Footprint),
	VectorModification(Box<VectorModification>),
	ImageData(Image<Color>),
	// ==========
	// ENUM TYPES
	// ==========
	Fill(vector::style::Fill),
	BlendMode(core_types::blending::BlendMode),
	LuminanceCalculation(raster_nodes::adjustments::LuminanceCalculation),
	QRCodeErrorCorrectionLevel(vector_nodes::generator_nodes::QRCodeErrorCorrectionLevel),
	XY(graphene_core::extract_xy::XY),
	StringCapitalization(text_nodes::StringCapitalization),
	RedGreenBlue(raster_nodes::adjustments::RedGreenBlue),
	RedGreenBlueAlpha(raster_nodes::adjustments::RedGreenBlueAlpha),
	RealTimeMode(graphene_core::animation::RealTimeMode),
	NoiseType(raster_nodes::adjustments::NoiseType),
	FractalType(raster_nodes::adjustments::FractalType),
	CellularDistanceFunction(raster_nodes::adjustments::CellularDistanceFunction),
	CellularReturnType(raster_nodes::adjustments::CellularReturnType),
	DomainWarpType(raster_nodes::adjustments::DomainWarpType),
	RelativeAbsolute(raster_nodes::adjustments::RelativeAbsolute),
	SelectiveColorChoice(raster_nodes::adjustments::SelectiveColorChoice),
	GridType(vector::misc::GridType),
	ArcType(vector::misc::ArcType),
	RowsOrColumns(vector::misc::RowsOrColumns),
	MergeByDistanceAlgorithm(vector::misc::MergeByDistanceAlgorithm),
	ExtrudeJoiningAlgorithm(vector::misc::ExtrudeJoiningAlgorithm),
	PointSpacingType(vector::misc::PointSpacingType),
	SpiralType(vector::misc::SpiralType),
	InterpolationDistribution(vector::misc::InterpolationDistribution),
	#[serde(alias = "LineCap")]
	StrokeCap(vector::style::StrokeCap),
	#[serde(alias = "LineJoin")]
	StrokeJoin(vector::style::StrokeJoin),
	StrokeAlign(vector::style::StrokeAlign),
	PaintOrder(vector::style::PaintOrder),
	GradientType(vector::style::GradientType),
	GradientSpreadMethod(vector::style::GradientSpreadMethod),
	ReferencePoint(vector::ReferencePoint),
	CentroidType(vector::misc::CentroidType),
	BooleanOperation(vector::misc::BooleanOperation),
	TextAlign(text_nodes::TextAlign),
	ScaleType(core_types::transform::ScaleType),
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
				let hex = input.trim().trim_matches('"').trim().trim_start_matches('#');
				let color = Color::from_hex_str(hex);
				if color.is_none() {
					log::error!("Invalid default value color string: {input}");
				}
				return color;
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

		fn to_gradient(input: &str) -> Option<GradientStops> {
			// String syntax: (e.g. "000000ff, ff0000ff")
			let stops = input.split(',').filter_map(|s| to_color(s.trim())).collect::<Vec<_>>();
			if stops.len() == 1 {
				Some(GradientStops::new(vec![
					GradientStop {
						position: 0.,
						midpoint: 0.5,
						color: stops[0],
					},
					GradientStop {
						position: 1.,
						midpoint: 0.5,
						color: stops[0],
					},
				]))
			} else if stops.len() >= 2 {
				let step = 1. / (stops.len() - 1) as f64;
				Some(GradientStops::new(stops.into_iter().enumerate().map(|(i, color)| GradientStop {
					position: i as f64 * step,
					midpoint: 0.5,
					color,
				})))
			} else {
				log::error!("Invalid default value gradient string: {input}");
				None
			}
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
				let ty = concrete_type.id?;
				use std::any::TypeId;
				// TODO: Add default implementations for types such as TaggedValue::Subpaths, and use the defaults here and in document_node_types
				// Tries using the default for the tagged value type. If it not implemented, then uses the default used in document_node_types. If it is not used there, then TaggedValue::None is returned.
				let ty = match () {
					() if ty == TypeId::of::<()>() => TaggedValue::None,
					() if ty == TypeId::of::<String>() => TaggedValue::String(string.into()),
					() if ty == TypeId::of::<f64>() => FromStr::from_str(string).map(TaggedValue::F64).ok()?,
					() if ty == TypeId::of::<f32>() => FromStr::from_str(string).map(TaggedValue::F32).ok()?,
					() if ty == TypeId::of::<u64>() => FromStr::from_str(string).map(TaggedValue::U64).ok()?,
					() if ty == TypeId::of::<u32>() => FromStr::from_str(string).map(TaggedValue::U32).ok()?,
					() if ty == TypeId::of::<DVec2>() => to_dvec2(string).map(TaggedValue::DVec2)?,
					() if ty == TypeId::of::<bool>() => FromStr::from_str(string).map(TaggedValue::Bool).ok()?,
					// `Color` (not in a table) is still currently needed by `BlackAndWhiteNode` and `ColorOverlayNode` GPU `shader_node(PerPixelAdjust)` variants
					() if ty == TypeId::of::<Color>() => to_color(string).map(|color| TaggedValue::Color(Some(color)))?,
					() if ty == TypeId::of::<Table<Color>>() => to_color(string).map(|color| TaggedValue::Color(Some(color)))?,
					() if ty == TypeId::of::<Table<GradientStops>>() => to_gradient(string).map(TaggedValue::Gradient)?,
					() if ty == TypeId::of::<Fill>() => to_color(string).map(|color| TaggedValue::Fill(Fill::solid(color)))?,
					() if ty == TypeId::of::<ReferencePoint>() => to_reference_point(string).map(TaggedValue::ReferencePoint)?,
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

/// Custom deserializer hooked onto `NodeInput::Value::tagged_value` that intercepts removed-variant tags before delegating to `TaggedValue`'s standard derive.
///
/// Routes legacy variant names into modern variants, in typed Rust. Each legacy name is also matched against the historical `#[serde(alias = "...")]` spellings the deleted variant accepted, so old-shape inner payloads are caught:
///
/// - `BrushCache` → `TaggedValue::None` (purely runtime cache; no payload to preserve)
/// - `Graphic` (or alias `GraphicGroup`/`Group`) → `TaggedValue::TypeDefault(descriptor!(Table<Graphic>))`
/// - `Artboard` (or alias `ArtboardGroup`) → `TaggedValue::TypeDefault(descriptor!(Table<Artboard>))`
/// - `Raster` (or alias `ImageFrame`/`RasterData`/`Image`):
///     - non-empty (the legacy `image` proto's input 1, where the inner `Raster<CPU>` serializes as the embedded `Image<Color>`) → `TaggedValue::ImageData(<inner Image<Color>>)`
///     - empty → `TaggedValue::TypeDefault(descriptor!(Table<Raster<CPU>>))`
/// - `Vector` (or alias `VectorData`):
///     - non-empty → `TaggedValue::VectorModification(<built from first element>)` (the document_migration's Path pass disambiguates this between SVG-import legacy and a discardable modern baked value via the input's `exposed` flag)
///     - empty → `TaggedValue::TypeDefault(descriptor!(Table<Vector>))`
///
/// All other tags (including ones with the modern shape) fall through to the standard derived `Deserialize` for `TaggedValue`.
// TODO: Eventually remove this migration document upgrade code
#[cfg(feature = "loading")]
pub fn deserialize_tagged_value_with_legacy_migration<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<MemoHash<TaggedValue>, D::Error> {
	use serde::Deserialize;
	let value = serde_json::Value::deserialize(deserializer)?;

	if let Some(map) = value.as_object()
		&& map.len() == 1
		&& let Some((tag, content)) = map.iter().next()
	{
		match tag.as_str() {
			"BrushCache" => return Ok(MemoHash::new(TaggedValue::None)),
			"Graphic" | "GraphicGroup" | "Group" => return Ok(MemoHash::new(TaggedValue::TypeDefault(descriptor!(Table<Graphic>)))),
			"Artboard" | "ArtboardGroup" => return Ok(MemoHash::new(TaggedValue::TypeDefault(descriptor!(Table<Artboard>)))),
			"Raster" | "ImageFrame" | "RasterData" | "Image" => {
				let first_element = content.as_object().and_then(|c| c.get("element")).and_then(|e| e.as_array()).and_then(|arr| arr.first());
				if let Some(image_value) = first_element {
					let image: Image<Color> = serde_json::from_value(image_value.clone()).map_err(serde::de::Error::custom)?;
					return Ok(MemoHash::new(TaggedValue::ImageData(image)));
				}
				return Ok(MemoHash::new(TaggedValue::TypeDefault(descriptor!(Table<Raster<CPU>>))));
			}
			"Vector" | "VectorData" => {
				let table = graphic_types::migrations::migrate_vector(content.clone()).map_err(serde::de::Error::custom)?;
				if let Some(vector) = table.element(0) {
					let modification = Box::new(VectorModification::create_from_vector(vector));
					return Ok(MemoHash::new(TaggedValue::VectorModification(modification)));
				}
				return Ok(MemoHash::new(TaggedValue::TypeDefault(descriptor!(Table<Vector>))));
			}
			// The `Gradient` tag was reused: it used to carry a full `Gradient` struct (now `FillGradient`), and now carries an `Option<GradientStops>`.
			// Disambiguate by payload shape: a Gradient struct has `start`/`end` keys; a `GradientStops` has none of those (it has `position`/`midpoint`/`color`).
			"Gradient" if content.as_object().is_some_and(|c| c.contains_key("start") && c.contains_key("end")) => {
				let gradient: Gradient = serde_json::from_value(content.clone()).map_err(serde::de::Error::custom)?;
				return Ok(MemoHash::new(TaggedValue::FillGradient(gradient)));
			}
			_ => {}
		}
	}

	let tagged_value: TaggedValue = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
	Ok(MemoHash::new(tagged_value))
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

#[derive(Debug, Clone, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize)]
pub enum RenderOutputType {
	#[serde(skip)]
	Texture(graphene_application_io::ImageTexture),
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
	#[cfg(target_family = "wasm")]
	CanvasFrame {
		canvas_id: u64,
		resolution: DVec2,
	},
}

impl CacheHash for RenderOutputType {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::mem::discriminant(self).hash(state);
		match self {
			Self::Texture(texture) => texture.hash(state),
			Self::Buffer { data, width, height } => {
				data.cache_hash(state);
				width.cache_hash(state);
				height.cache_hash(state);
			}
			Self::Svg { svg, image_data } => {
				svg.cache_hash(state);
				image_data.cache_hash(state);
			}
			#[cfg(target_family = "wasm")]
			Self::CanvasFrame { canvas_id, resolution } => {
				canvas_id.cache_hash(state);
				resolution.cache_hash(state);
			}
		}
	}
}

// Metadata is excluded because it's editor-side auxiliary data (click targets, transforms)
// that shouldn't affect render cache invalidation, and it contains HashMaps with non-deterministic iteration order
impl CacheHash for RenderOutput {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.data.cache_hash(state);
	}
}
