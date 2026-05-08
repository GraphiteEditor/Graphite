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
			None,
			/// Stores a type, from which its `Default::default()` value can be obtained, rather than storing an actual type's value.
			/// Example: `TaggedValue::TypeDefault(descriptor!(String))` stores the type `String` but no specific string value.
			TypeDefault(TypeDescriptor),
			$( $(#[$meta] ) *$identifier( $ty ), )*
			RenderOutput(RenderOutput),
			#[serde(skip)]
			EditorApi(Arc<PlatformEditorApi>),
		}

		impl CacheHash for TaggedValue {
			fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
				core::mem::discriminant(self).hash(state);
				match self {
					Self::None => {}
					Self::TypeDefault(td) => td.cache_hash(state),
					$( Self::$identifier(x) => { x.cache_hash(state) }),*
					Self::RenderOutput(x) => x.cache_hash(state),
					Self::EditorApi(x) => x.cache_hash(state),
				}
			}
		}

		impl<'a> TaggedValue {
			/// Converts to a Box<dyn DynAny>
			pub fn to_dynany(self) -> DAny<'a> {
				match self {
					Self::None => Box::new(()),
					Self::TypeDefault(td) => {
						// Construct the actual default for types without a `TaggedValue` variant directly, instead of going through
						// `from_type_or_none` (which would just return `TypeDefault` again and recurse forever).
						let name = td.name.as_ref();
						if name == std::any::type_name::<Table<Graphic>>() { return Box::new(Table::<Graphic>::default()); }
						if name == std::any::type_name::<Table<Artboard>>() { return Box::new(Table::<Artboard>::default()); }
						if name == std::any::type_name::<Table<Raster<CPU>>>() { return Box::new(Table::<Raster<CPU>>::default()); }
						Self::from_type_or_none(&Type::Concrete(td)).to_dynany()
					}
					$( Self::$identifier(x) => Box::new(x), )*
					Self::RenderOutput(x) => Box::new(x),
					Self::EditorApi(x) => Box::new(x),
				}
			}

			/// Converts to a Arc<dyn Any + Send + Sync + 'static>
			pub fn to_any(self) -> Arc<dyn std::any::Any + Send + Sync + 'static> {
				match self {
					Self::None => Arc::new(()),
					Self::TypeDefault(td) => {
						// Same direct-construction path as `to_dynany` for the same reason as in `to_dynany`.
						let name = td.name.as_ref();
						if name == std::any::type_name::<Table<Graphic>>() { return Arc::new(Table::<Graphic>::default()); }
						if name == std::any::type_name::<Table<Artboard>>() { return Arc::new(Table::<Artboard>::default()); }
						if name == std::any::type_name::<Table<Raster<CPU>>>() { return Arc::new(Table::<Raster<CPU>>::default()); }
						Self::from_type_or_none(&Type::Concrete(td)).to_any()
					}
					$( Self::$identifier(x) => Arc::new(x), )*
					Self::RenderOutput(x) => Arc::new(x),
					Self::EditorApi(x) => Arc::new(x),
				}
			}

			/// Creates a core_types::Type::Concrete(TypeDescriptor { .. }) with the type of the value inside the tagged value
			pub fn ty(&self) -> Type {
				match self {
					Self::None => concrete!(()),
					Self::TypeDefault(td) => Type::Concrete(td.clone()),
					$( Self::$identifier(_) => concrete!($ty), )*
					Self::RenderOutput(_) => concrete!(RenderOutput),
					Self::EditorApi(_) => concrete!(&PlatformEditorApi),
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
						if name == std::any::type_name::<Table<Color>>() { return Some(TaggedValue::Color(Table::new_from_element(Color::default()))) }
						if name == std::any::type_name::<Table<GradientStops>>() { return Some(TaggedValue::GradientTable(Table::new_from_element(GradientStops::default()))) }
						$( if name == std::any::type_name::<$ty>() { return Some(TaggedValue::$identifier(Default::default())) } )*
						// Types whose `TaggedValue` variant has been removed. They route through `TypeDefault` instead, with `to_dynany`/`to_any` constructing the actual default at execution time.
						if name == std::any::type_name::<Table<Graphic>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						if name == std::any::type_name::<Table<Artboard>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						if name == std::any::type_name::<Table<Raster<CPU>>>() { return Some(TaggedValue::TypeDefault(concrete_type.clone())) }
						None
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

			pub fn to_debug_string(&self) -> String {
				match self {
					Self::None => "()".to_string(),
					Self::TypeDefault(td) => format!("TypeDefault({})", td.name),
					$( Self::$identifier(x) => format!("{:?}", x), )*
					Self::RenderOutput(_) => "RenderOutput".to_string(),
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
	// ===========
	// TABLE TYPES
	// ===========
	StringTable(Table<String>),
	#[serde(deserialize_with = "core_types::misc::migrate_vec_f64_to_table")] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "VecF64", alias = "VecF32", alias = "F64Array4")]
	F64Table(Table<f64>),
	NodeIdTable(Table<NodeId>),
	#[serde(deserialize_with = "graphic_types::migrations::migrate_vector")] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "VectorData")]
	Vector(Table<Vector>),
	#[serde(deserialize_with = "core_types::misc::migrate_color")] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "ColorTable", alias = "OptionalColor", alias = "ColorNotInTable")]
	Color(Table<Color>),
	#[serde(deserialize_with = "graphic_types::vector_types::gradient::migrate_gradient_stops")] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "GradientPositions", alias = "GradientStops")]
	GradientTable(Table<GradientStops>),
	#[serde(deserialize_with = "brush_nodes::migrations::migrate_brush_strokes_to_table")] // TODO: Eventually remove this migration document upgrade code
	#[serde(alias = "BrushStrokes")]
	BrushStrokeTable(Table<BrushStroke>),
	// ============
	// SCALAR TYPES
	// ============
	F32(f32),
	F64(f64),
	U32(u32),
	U64(u64),
	Bool(bool),
	String(String),
	#[serde(alias = "IVec2", alias = "UVec2")]
	DVec2(DVec2),
	DAffine2(DAffine2),
	Gradient(Gradient),
	Font(Font),
	DocumentNode(DocumentNode),
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
					() if ty == TypeId::of::<Color>() => to_color(string).map(|color| TaggedValue::Color(Table::new_from_element(color)))?,
					() if ty == TypeId::of::<Table<Color>>() => to_color(string).map(|color| TaggedValue::Color(Table::new_from_element(color)))?,
					() if ty == TypeId::of::<Table<GradientStops>>() => to_gradient(string).map(|color| TaggedValue::GradientTable(Table::new_from_element(color)))?,
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

	/// Walks a JSON document tree and rewrites any externally-tagged `TaggedValue` whose discriminant has been removed to allow documents to continue deserialization.
	///
	/// `REMOVED_VARIANTS` discriminants get rewritten to the unit variant `"None"`; the document migration step then removes any orphan node inputs that result.
	/// `REMOVED_PLACEHOLDER_CONTAINERS` discriminants are rewritten to a `TypeDefault` carrying the corresponding
	/// type name, since they were only ever used as empty `Table<...>` placeholders that should now route through the `TypeDefault` mechanism.
	#[cfg(feature = "loading")]
	pub fn scrub_removed_variants_from_json(value: &mut serde_json::Value) {
		// Names of `TaggedValue` variants that have been removed since being released and carried no useful payload.
		// Any object of the form `{"<name>": <payload>}` is rewritten to `"None"` on load.
		const REMOVED_VARIANTS: &[&str] = &["BrushCache"];

		// Names of `TaggedValue` variants that have been removed since being released and were only used as empty `Table<...>` placeholders.
		// Any object of the form `{"<name>": <payload>}` is rewritten to a `TypeDefault` carrying the corresponding type name.
		// Includes the historical `#[serde(alias = "...")]` spellings the deleted variants accepted, so old-shape inner payloads are also caught.
		const REMOVED_PLACEHOLDER_CONTAINERS: &[(&str, &str)] = &[
			("Graphic", "core_types::table::Table<graphic_types::graphic::Graphic>"),
			("GraphicGroup", "core_types::table::Table<graphic_types::graphic::Graphic>"),
			("Group", "core_types::table::Table<graphic_types::graphic::Graphic>"),
			("Artboard", "core_types::table::Table<graphic_types::artboard::Artboard>"),
			("ArtboardGroup", "core_types::table::Table<graphic_types::artboard::Artboard>"),
			("Raster", "core_types::table::Table<graphic_types::raster_types::Raster<graphic_types::raster_types::CPU>>"),
			("ImageFrame", "core_types::table::Table<graphic_types::raster_types::Raster<graphic_types::raster_types::CPU>>"),
			("RasterData", "core_types::table::Table<graphic_types::raster_types::Raster<graphic_types::raster_types::CPU>>"),
			("Image", "core_types::table::Table<graphic_types::raster_types::Raster<graphic_types::raster_types::CPU>>"),
		];

		match value {
			serde_json::Value::Object(map) => {
				if map.len() == 1
					&& let Some(key) = map.keys().next()
				{
					if REMOVED_VARIANTS.contains(&key.as_str()) {
						*value = serde_json::Value::String("None".to_string());
						return;
					}
					if let Some((_, type_name)) = REMOVED_PLACEHOLDER_CONTAINERS.iter().find(|(name, _)| *name == key.as_str()) {
						*value = serde_json::json!({ "TypeDefault": { "name": *type_name } });
						return;
					}
				}
				for child in map.values_mut() {
					Self::scrub_removed_variants_from_json(child);
				}
			}
			serde_json::Value::Array(array) => {
				for child in array {
					Self::scrub_removed_variants_from_json(child);
				}
			}
			_ => {}
		}
	}

	// TODO: Eventually remove this migration document upgrade code
	/// Walks a JSON document tree, finds nodes whose `implementation.ProtoNode.name` is the legacy `image` proto nodde (current name or any of its renamed predecessors), and rescues an embedded image stored at input 1 as a `Table<Raster<CPU>>` into the modern `TaggedValue::ImageData` shape.
	///
	/// Without this, the subsequent `scrub_removed_variants_from_json` would discard the table payload along with the variant tag, losing the user's image data. Must run before the scrub.
	#[cfg(feature = "loading")]
	pub fn rescue_legacy_image_proto_inputs_in_json(value: &mut serde_json::Value) {
		// Identifiers for the `image` proto node — current and historical aliases (mirroring the entry under `NODE_REPLACEMENTS` in `document_migration.rs`).
		// This walk runs before `NODE_REPLACEMENTS` rewrites old proto names to the current one, so we have to recognize all spellings.
		const IMAGE_PROTO_NAMES: &[&str] = &[
			"raster_nodes::std_nodes::ImageNode",
			"raster_nodes::std_nodes::ImageValueNode",
			"graphene_raster_nodes::std_nodes::ImageValueNode",
			"graphene_std::raster::ImageValueNode",
			"graphene_std::raster::ImageNode",
		];

		// Variant tags meaning "this is a `Table<Raster<CPU>>`" — the canonical name plus the historical `#[serde(alias = "...")]` spellings the deleted variant accepted.
		const RASTER_VARIANT_TAGS: &[&str] = &["Raster", "ImageFrame", "RasterData", "Image"];

		match value {
			serde_json::Value::Object(map) => {
				let is_image_proto = map
					.get("implementation")
					.and_then(|i| i.as_object())
					.and_then(|m| m.get("ProtoNode"))
					.and_then(|p| p.as_object())
					.and_then(|m| m.get("name"))
					.and_then(|n| n.as_str())
					.is_some_and(|name| IMAGE_PROTO_NAMES.contains(&name));

				if is_image_proto
					&& let Some(input_1) = map.get_mut("inputs").and_then(|i| i.as_array_mut()).and_then(|a| a.get_mut(1))
					&& let Some(input_obj) = input_1.as_object_mut()
					&& let Some(value_obj) = input_obj.get_mut("Value").and_then(|v| v.as_object_mut())
					&& let Some(tagged_value) = value_obj.get_mut("tagged_value")
				{
					// `tagged_value` should be `{"Raster": {"element": [<image>, ...]}}` (or one of the historical aliases). Each element of `Table<Raster<CPU>>` serializes as the inner `Image<Color>` directly because `Raster<CPU>::serialize` delegates to `Image<Color>`.
					let rescued = tagged_value
						.as_object()
						.filter(|tv| tv.len() == 1)
						.and_then(|tv| tv.iter().next())
						.filter(|(tag, _)| RASTER_VARIANT_TAGS.contains(&tag.as_str()))
						.and_then(|(_, content)| content.as_object())
						.and_then(|c| c.get("element"))
						.and_then(|e| e.as_array())
						.and_then(|arr| arr.first())
						.cloned();

					if let Some(image) = rescued {
						*tagged_value = serde_json::json!({ "ImageData": image });
						value_obj.insert("exposed".to_string(), serde_json::Value::Bool(false));
					}
				}

				for child in map.values_mut() {
					Self::rescue_legacy_image_proto_inputs_in_json(child);
				}
			}
			serde_json::Value::Array(array) => {
				for child in array {
					Self::rescue_legacy_image_proto_inputs_in_json(child);
				}
			}
			_ => {}
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

#[cfg(all(test, feature = "loading"))]
mod tests {
	use super::*;

	#[test]
	fn scrub_replaces_removed_variant_with_none_unit() {
		let mut value = serde_json::json!({
			"Value": {
				"tagged_value": { "BrushCache": { "unique_id": 1, "prev_input": [] } },
				"exposed": false
			}
		});
		TaggedValue::scrub_removed_variants_from_json(&mut value);
		assert_eq!(value, serde_json::json!({ "Value": { "tagged_value": "None", "exposed": false } }));
	}

	#[test]
	fn scrub_leaves_live_variants_unchanged() {
		let mut value = serde_json::json!({ "Value": { "tagged_value": { "F64": 1.5 }, "exposed": false } });
		let original = value.clone();
		TaggedValue::scrub_removed_variants_from_json(&mut value);
		assert_eq!(value, original);
	}

	#[test]
	fn scrub_recurses_through_arrays_and_nested_objects() {
		let mut value = serde_json::json!([{ "BrushCache": { "any": "payload" } }, { "F32": 0.5 }]);
		TaggedValue::scrub_removed_variants_from_json(&mut value);
		assert_eq!(value, serde_json::json!(["None", { "F32": 0.5 }]));
	}

	#[test]
	fn scrub_rewrites_removed_placeholder_container_to_type_default() {
		let mut value = serde_json::json!({
			"Value": {
				"tagged_value": { "Graphic": { "element": [] } },
				"exposed": true
			}
		});
		TaggedValue::scrub_removed_variants_from_json(&mut value);
		assert_eq!(
			value,
			serde_json::json!({
				"Value": {
					"tagged_value": { "TypeDefault": { "name": "core_types::table::Table<graphic_types::graphic::Graphic>" } },
					"exposed": true
				}
			})
		);
	}

	#[test]
	fn scrub_rewrites_legacy_alias_with_old_shape_payload() {
		// Old documents stored `Artboard` under the alias `ArtboardGroup` with a shape preceding the `Table<Artboard>` rewrite (`{id, instance, transform, alpha_blending, source_node_id}` parallel arrays). The scrub must catch the alias and discard the legacy payload.
		let mut value = serde_json::json!({
			"ArtboardGroup": { "id": [], "instance": [], "transform": [], "alpha_blending": [], "source_node_id": [] }
		});
		TaggedValue::scrub_removed_variants_from_json(&mut value);
		assert_eq!(value, serde_json::json!({ "TypeDefault": { "name": "core_types::table::Table<graphic_types::artboard::Artboard>" } }));
	}
}
