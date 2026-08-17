use super::DocumentNode;
use crate::application_io::PlatformEditorApi;
use crate::application_io::resource::Resource;
use crate::proto::{Any as DAny, FutureAny};
use brush_nodes::brush_stroke::{BrushStroke, BrushTrace};
use core_types::color::SRGBA8;
use core_types::list::{Item, List, NodeIdPath};
use core_types::transform::Footprint;
use core_types::{CacheHash, Color, ContextFeatures, MemoHash, Node, Type, TypeDescriptor};
use dyn_any::DynAny;
pub use dyn_any::StaticType;
pub use glam::{DAffine2, DVec2, IVec2, UVec2};
use graphene_application_io::resource::ResourceHash;
use graphene_application_io::resource::ResourceId;
use graphic_types::raster_types::{CPU, Image, Raster};
use graphic_types::vector_types::vector::misc::BoxCorners;
use graphic_types::vector_types::vector::style::DashPattern;
use graphic_types::vector_types::vector::style::{Gradient, GradientRamp, MeshGradient, MeshGradientSurface};
use graphic_types::vector_types::vector::{self, ReferencePoint};
use graphic_types::{Artboard, Graphic, Vector};
use rendering::RenderMetadata;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::str::FromStr;
pub use std::sync::Arc;
use text_nodes::Font;
use vector::VectorModification;

pub struct TaggedValueTypeError;

/// Item-cell element types routed through [`TaggedValue::TypeDefault`] instead of another dedicated variant, stored as the concrete `Item<T>` wire type.
/// Consumed by [`TaggedValue::from_type`] (which creates `TypeDefault` values) and [`TaggedValue::to_dynany`]/[`TaggedValue::to_any`] (which unwrap them into real default values).
macro_rules! for_each_item_type_default {
	($action:ident) => {
		$action!(Vector);
		$action!(f64);
		$action!(Raster<CPU>);
		$action!(Graphic);
		$action!(Color);
		$action!(Gradient);
		$action!(Artboard);
		$action!(String);
	};
}

/// List element types routed through [`TaggedValue::TypeDefault`], stored as the structural [`Type::List`] form.
/// `List<f64>` is absent because it stores as `TaggedValue::F64Array`.
macro_rules! for_each_list_type_default {
	($action:ident) => {
		$action!(Graphic);
		$action!(Artboard);
		$action!(Raster<CPU>);
		$action!(Vector);
		$action!(String);
		$action!(Color);
		$action!(Gradient);
	};
}

/// Unranked types routed through [`TaggedValue::TypeDefault`], stored as their concrete type.
macro_rules! for_each_bare_type_default {
	($action:ident) => {
		$action!(DocumentNode);
		$action!(Resource);
	};
}

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
			/// Example: `TaggedValue::TypeDefault(concrete!(String))` stores the type `String` but no specific string value.
			/// (Old documents stored a bare `TypeDescriptor` payload, routed to this shape by `deserialize_tagged_value_with_legacy_migration`.)
			TypeDefault(Type),
			/// Stored compactly as a `Vec<f64>`, materializes as `List<f64>` at runtime via `to_dynany`/`to_any`. Aliases recover legacy on-disk shapes.
			#[serde(deserialize_with = "core_types::misc::migrate_to_f64_array")] // TODO: Eventually remove this document upgrade code
			#[serde(alias = "F64Table", alias = "VecF64", alias = "VecF32", alias = "F64Array4")]
			F64Array(Vec<f64>),
			/// Stored compactly as a `Vec<f64>` of dash lengths, materializes as an `Item<DashPattern>` at runtime via `to_dynany`/`to_any`.
			DashPattern(Vec<f64>),
			/// Stored compactly as a `Vec<f64>` of corner values, materializes as an `Item<BoxCorners>` at runtime via `to_dynany`/`to_any`.
			BoxCorners(Vec<f64>),
			/// Stored as the `GradientRamp` exchange struct (nested `{ stops: { color, position?, midpoint? } }`), materializing as an `Item<Gradient>` at runtime. Aliases recover legacy on-disk shapes.
			/// (Old documents stored flat stops, a tuple list, or the ancient full `Gradient` struct under the legacy `"Gradient"` tag, all routed by `deserialize_tagged_value_with_legacy_migration`.)
			#[serde(alias = "Gradient", alias = "GradientTable", alias = "GradientPositions", alias = "GradientStops")]
			GradientRamp(GradientRamp),
			/// Stored as the `MeshGradientSurface` exchange struct (nested `{ mesh: ... }`), materializing as an `Item<MeshGradient>` at runtime.
			MeshGradient(MeshGradientSurface),
			/// Stored compactly as a `Vec<BrushStroke>`, materializes as the single-value `Item<BrushTrace>` at runtime via `to_dynany`/`to_any`. Aliases recover legacy on-disk shapes.
			#[serde(deserialize_with = "brush_nodes::migrations::migrate_to_brush_strokes")] // TODO: Eventually remove this document upgrade code
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
			/// Path to the consumer of a `NodeInput::Reflection(DocumentNodePath)`. Materializes an `Item<NodeIdPath>` at runtime via `to_dynany`/`to_any` during graph flattening, matching the ranked connectors it feeds.
			#[serde(skip)]
			NodeIdPath(NodeIdPath),
			/// The `DocumentNode` value carried by an `Extract` proto node, populated at flatten time by `resolve_extract_nodes`. The on-disk placeholder uses `TypeDefault(concrete!(DocumentNode))`.
			#[serde(skip)]
			DocumentNode(DocumentNode),
			/// Carried by context nullification proto nodes constructed at proto node compilation time in `insert_context_nullification_nodes`.
			#[serde(skip)]
			ContextFeatures(ContextFeatures),
			#[serde(skip)]
			EditorApi(Arc<PlatformEditorApi>),
			/// Only used by the `resource` node, should never be serialized
			#[serde(skip)]
			ResourceHash(ResourceHash),
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
					Self::DashPattern(lengths) => lengths.cache_hash(state),
					Self::BoxCorners(values) => values.cache_hash(state),
					Self::GradientRamp(ramp) => ramp.cache_hash(state),
					Self::MeshGradient(surface) => surface.cache_hash(state),
					Self::BrushStrokes(strokes) => strokes.cache_hash(state),
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::NodeIdPath(path) => path.cache_hash(state),
					Self::DocumentNode(node) => node.cache_hash(state),
					Self::ContextFeatures(features) => features.cache_hash(state),
					Self::RenderOutput(x) => x.cache_hash(state),
					Self::EditorApi(x) => x.cache_hash(state),
					Self::ResourceHash(x) => x.cache_hash(state),
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
						// Construct the actual default for types without a `TaggedValue` variant directly.
						// Recursion through `from_type_or_none` below is safe only because the type-default lists
						// exhaustively cover every type that `from_type` would route back to `TypeDefault`.
						match &td {
							Type::List(element) => {
								macro_rules! check {
									($type_default:ty) => {
										if **element == concrete!($type_default) { return Box::new(List::<$type_default>::default()); }
									};
								}
								for_each_list_type_default!(check);
							}
							Type::Item(element) => {
								macro_rules! check {
									($type_default:ty) => {
										if **element == concrete!($type_default) { return Box::new(Item::<$type_default>::default()); }
									};
								}
								for_each_item_type_default!(check);
							}
							Type::Concrete(descriptor) => {
								let name = descriptor.name.as_ref();
								macro_rules! check_bare {
									($type_default:ty) => {
										if name == std::any::type_name::<$type_default>() { return Box::new(<$type_default>::default()); }
									};
								}
								for_each_bare_type_default!(check_bare);
							}
							_ => {}
						}
						Self::from_type_or_none(&td).to_dynany()
					}
					Self::F64Array(values) => {
						let list: List<f64> = values.into_iter().map(core_types::list::Item::new_from_element).collect();
						Box::new(list)
					}
					Self::DashPattern(lengths) => Box::new(Item::new_from_element(DashPattern::from(lengths))),
					Self::BoxCorners(values) => Box::new(Item::new_from_element(BoxCorners::from(values))),
					Self::GradientRamp(ramp) => Box::new(Item::<Gradient>::from(ramp)),
					Self::MeshGradient(surface) => Box::new(Item::<MeshGradient>::from(surface)),
					Self::BrushStrokes(strokes) => Box::new(core_types::list::Item::new_from_element(BrushTrace::from(strokes))),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(x) => Box::new(Item::new_from_element(x)), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::RenderOutput(x) => Box::new(Item::new_from_element(x)),
					Self::NodeIdPath(path) => Box::new(Item::new_from_element(path)),
					Self::DocumentNode(node) => Box::new(node),
					Self::ContextFeatures(features) => Box::new(Item::new_from_element(features)),
					Self::EditorApi(x) => Box::new(x),
					Self::ResourceHash(x) => Box::new(Item::new_from_element(x)),
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
						match &td {
							Type::List(element) => {
								macro_rules! check {
									($type_default:ty) => {
										if **element == concrete!($type_default) { return Arc::new(List::<$type_default>::default()); }
									};
								}
								for_each_list_type_default!(check);
							}
							Type::Item(element) => {
								macro_rules! check {
									($type_default:ty) => {
										if **element == concrete!($type_default) { return Arc::new(Item::<$type_default>::default()); }
									};
								}
								for_each_item_type_default!(check);
							}
							Type::Concrete(descriptor) => {
								let name = descriptor.name.as_ref();
								macro_rules! check_bare {
									($type_default:ty) => {
										if name == std::any::type_name::<$type_default>() { return Arc::new(<$type_default>::default()); }
									};
								}
								for_each_bare_type_default!(check_bare);
							}
							_ => {}
						}
						Self::from_type_or_none(&td).to_any()
					}
					Self::F64Array(values) => {
						let list: List<f64> = values.into_iter().map(core_types::list::Item::new_from_element).collect();
						Arc::new(list)
					}
					Self::DashPattern(lengths) => Arc::new(Item::new_from_element(DashPattern::from(lengths))),
					Self::BoxCorners(values) => Arc::new(Item::new_from_element(BoxCorners::from(values))),
					Self::GradientRamp(ramp) => Arc::new(Item::<Gradient>::from(ramp)),
					Self::MeshGradient(surface) => Arc::new(Item::<MeshGradient>::from(surface)),
					Self::BrushStrokes(strokes) => Arc::new(core_types::list::Item::new_from_element(BrushTrace::from(strokes))),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(x) => Arc::new(Item::new_from_element(x)), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::RenderOutput(x) => Arc::new(Item::new_from_element(x)),
					Self::NodeIdPath(path) => Arc::new(Item::new_from_element(path)),
					Self::DocumentNode(node) => Arc::new(node),
					Self::ContextFeatures(features) => Arc::new(Item::new_from_element(features)),
					Self::EditorApi(x) => Arc::new(x),
					Self::ResourceHash(x) => Arc::new(Item::new_from_element(x)),
				}
			}

			/// Creates the wire [`Type`] of the value inside the tagged value, with ranked types in their structural form.
			pub fn ty(&self) -> Type {
				let ty = match self {
					// ===============
					// MANUAL VARIANTS
					// ===============
					Self::None => concrete!(()),
					Self::TypeDefault(td) => td.clone(),
					Self::F64Array(_) => list!(f64),
					Self::DashPattern(_) => item!(DashPattern),
					Self::BoxCorners(_) => item!(BoxCorners),
					Self::GradientRamp(_) => item!(Gradient),
					Self::MeshGradient(_) => item!(MeshGradient),
					Self::BrushStrokes(_) => item!(BrushTrace),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( Self::$identifier(_) => item!($ty), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					Self::RenderOutput(_) => item!(RenderOutput),
					Self::NodeIdPath(_) => item!(NodeIdPath),
					Self::DocumentNode(_) => concrete!(DocumentNode),
					Self::ContextFeatures(_) => item!(ContextFeatures),
					Self::EditorApi(_) => item!(&PlatformEditorApi),
					Self::ResourceHash(_) => item!(ResourceHash),
				};

				// Defensively converges any remaining name-encoded ranked type (e.g. an opaque macro capture) to the structural form
				ty.normalize_rank()
			}

			/// Attempts to downcast the dynamic type to a tagged value
			pub fn try_from_any(input: Box<dyn DynAny<'a> + 'a>) -> Result<Self, String> {
				use dyn_any::downcast;
				use std::any::TypeId;

				match DynAny::type_id(input.as_ref()) {
					// ===============
					// MANUAL VARIANTS
					// ===============
					// The manual variants convert from both their payload and wire forms, with the newtypes flattening to their stored `Vec<f64>` form
					x if x == TypeId::of::<()>() => Ok(TaggedValue::None),
					x if x == TypeId::of::<Vec<f64>>() => Ok(TaggedValue::F64Array(*downcast(input).unwrap())),
					x if x == TypeId::of::<List<f64>>() => Ok(TaggedValue::F64Array(downcast::<List<f64>>(input).unwrap().iter_element_values().copied().collect())),
					x if x == TypeId::of::<DashPattern>() => Ok(TaggedValue::DashPattern(downcast::<DashPattern>(input).unwrap().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<Item<DashPattern>>() => Ok(TaggedValue::DashPattern(downcast::<Item<DashPattern>>(input).unwrap().into_element().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<BoxCorners>() => Ok(TaggedValue::BoxCorners(downcast::<BoxCorners>(input).unwrap().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<Item<BoxCorners>>() => Ok(TaggedValue::BoxCorners(downcast::<Item<BoxCorners>>(input).unwrap().into_element().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<Gradient>() => Ok(TaggedValue::GradientRamp(GradientRamp::from(*downcast::<Gradient>(input).unwrap()))),
					x if x == TypeId::of::<Item<Gradient>>() => Ok(TaggedValue::GradientRamp(GradientRamp::from(&*downcast::<Item<Gradient>>(input).unwrap()))),
					x if x == TypeId::of::<MeshGradient>() => Ok(TaggedValue::MeshGradient(MeshGradientSurface::from(*downcast::<MeshGradient>(input).unwrap()))),
					x if x == TypeId::of::<Item<MeshGradient>>() => Ok(TaggedValue::MeshGradient(MeshGradientSurface::from(&*downcast::<Item<MeshGradient>>(input).unwrap()))),
					x if x == TypeId::of::<Vec<BrushStroke>>() => Ok(TaggedValue::BrushStrokes(*downcast(input).unwrap())),
					x if x == TypeId::of::<Item<BrushTrace>>() => Ok(TaggedValue::BrushStrokes(downcast::<Item<BrushTrace>>(input).unwrap().into_element().0.iter_element_values().cloned().collect())),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( x if x == TypeId::of::<$ty>() => Ok(TaggedValue::$identifier(*downcast(input).unwrap())), )*
					$( x if x == TypeId::of::<Item<$ty>>() => Ok(TaggedValue::$identifier(downcast::<Item<$ty>>(input).unwrap().into_element())), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					x if x == TypeId::of::<Item<RenderOutput>>() => Ok(TaggedValue::RenderOutput(downcast::<Item<RenderOutput>>(input).unwrap().into_element())),

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
					// The manual variants convert from both their payload and wire forms, with the newtypes flattening to their stored `Vec<f64>` form
					x if x == TypeId::of::<()>() => Ok(TaggedValue::None),
					x if x == TypeId::of::<Vec<f64>>() => Ok(TaggedValue::F64Array(input.downcast_ref::<Vec<f64>>().unwrap().clone())),
					x if x == TypeId::of::<List<f64>>() => Ok(TaggedValue::F64Array(input.downcast_ref::<List<f64>>().unwrap().iter_element_values().copied().collect())),
					x if x == TypeId::of::<DashPattern>() => Ok(TaggedValue::DashPattern(input.downcast_ref::<DashPattern>().unwrap().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<Item<DashPattern>>() => Ok(TaggedValue::DashPattern(input.downcast_ref::<Item<DashPattern>>().unwrap().element().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<BoxCorners>() => Ok(TaggedValue::BoxCorners(input.downcast_ref::<BoxCorners>().unwrap().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<Item<BoxCorners>>() => Ok(TaggedValue::BoxCorners(input.downcast_ref::<Item<BoxCorners>>().unwrap().element().0.iter_element_values().copied().collect())),
					x if x == TypeId::of::<Gradient>() => Ok(TaggedValue::GradientRamp(GradientRamp::from(input.downcast_ref::<Gradient>().unwrap()))),
					x if x == TypeId::of::<Item<Gradient>>() => Ok(TaggedValue::GradientRamp(GradientRamp::from(input.downcast_ref::<Item<Gradient>>().unwrap()))),
					x if x == TypeId::of::<MeshGradient>() => Ok(TaggedValue::MeshGradient(MeshGradientSurface::from(input.downcast_ref::<MeshGradient>().unwrap().clone()))),
					x if x == TypeId::of::<Item<MeshGradient>>() => Ok(TaggedValue::MeshGradient(MeshGradientSurface::from(input.downcast_ref::<Item<MeshGradient>>().unwrap()))),
					x if x == TypeId::of::<Vec<BrushStroke>>() => Ok(TaggedValue::BrushStrokes(input.downcast_ref::<Vec<BrushStroke>>().unwrap().clone())),
					x if x == TypeId::of::<Item<BrushTrace>>() => Ok(TaggedValue::BrushStrokes(input.downcast_ref::<Item<BrushTrace>>().unwrap().element().0.iter_element_values().cloned().collect())),
					// =======================
					// AUTO-GENERATED VARIANTS
					// =======================
					$( x if x == TypeId::of::<$ty>() => Ok(TaggedValue::$identifier(<$ty as Clone>::clone(input.downcast_ref().unwrap()))), )*
					$( x if x == TypeId::of::<Item<$ty>>() => Ok(TaggedValue::$identifier(Item::<$ty>::clone(input.downcast_ref().unwrap()).into_element())), )*
					// =======================
					// NON-SERIALIZED VARIANTS
					// =======================
					x if x == TypeId::of::<Item<RenderOutput>>() => Ok(TaggedValue::RenderOutput(Item::<RenderOutput>::clone(input.downcast_ref().unwrap()).into_element())),
					_ => Err(format!("Cannot convert {:?} to TaggedValue", std::any::type_name_of_val(input))),
				}
			}

			/// Returns a TaggedValue from the type, where that value is its type's `Default::default()`.
			/// Dispatches by name for concrete types and structurally by element for ranked types, where the name
			/// field is what round-trips through serde so it works even for types deserialized from disk.
			pub fn from_type(input: &Type) -> Option<Self> {
				match input {
					Type::Generic(_) => None,
					Type::Concrete(concrete_type) => {
						let name = concrete_type.name.as_ref();
						// TODO: Add default implementations for types such as TaggedValue::Subpaths, and use the defaults here and in document_node_types
						// Tries using the default for the tagged value type. If it not implemented, then uses the default used in document_node_types. If it is not used there, then TaggedValue::None is returned.
						if name == std::any::type_name::<()>() { return Some(TaggedValue::None) }
						if name == std::any::type_name::<Gradient>() { return Some(TaggedValue::GradientRamp(GradientRamp::default())) }
						if name == std::any::type_name::<DashPattern>() { return Some(TaggedValue::DashPattern(Vec::new())) }
						if name == std::any::type_name::<BoxCorners>() { return Some(TaggedValue::BoxCorners(Vec::new())) }
						if name == std::any::type_name::<MeshGradient>() { return Some(TaggedValue::MeshGradient(MeshGradientSurface::default())) }
						$( if name == std::any::type_name::<$ty>() { return Some(TaggedValue::$identifier(Default::default())) } )*
						if name == std::any::type_name::<BrushTrace>() { return Some(TaggedValue::BrushStrokes(Vec::new())) }
						// Unranked types without a variant route through `TypeDefault`, with `to_dynany`/`to_any` constructing the actual default at execution time
						macro_rules! check_bare {
							($type_default:ty) => {
								if name == std::any::type_name::<$type_default>() { return Some(TaggedValue::TypeDefault(input.clone())); }
							};
						}
						for_each_bare_type_default!(check_bare);
						None
					}
					Type::Fn(_, output) => TaggedValue::from_type(output),
					Type::Future(output) => TaggedValue::from_type(output),
					// Element types with a dedicated variant use it directly (the variant's value is a rank-0 cell); the rest store the structural type
					Type::Item(element) => TaggedValue::from_type(element).or_else(|| {
						macro_rules! check {
							($type_default:ty) => {
								if **element == concrete!($type_default) { return Some(TaggedValue::TypeDefault(input.clone())); }
							};
						}
						for_each_item_type_default!(check);
						None
					}),
					// Structural lists match by element; `List<f64>` stays the dedicated `F64Array` variant
					Type::List(element) => {
						if **element == concrete!(f64) {
							return Some(TaggedValue::F64Array(Vec::new()));
						}
						macro_rules! check {
							($type_default:ty) => {
								if **element == concrete!($type_default) { return Some(TaggedValue::TypeDefault(input.clone())); }
							};
						}
						for_each_list_type_default!(check);
						None
					}
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
					Self::TypeDefault(td) => format!("TypeDefault({td})"),
					Self::F64Array(values) => format!("F64Array({values:?})"),
					Self::DashPattern(lengths) => format!("DashPattern({lengths:?})"),
					Self::BoxCorners(values) => format!("BoxCorners({values:?})"),
					Self::GradientRamp(ramp) => format!("GradientRamp({ramp:?})"),
					Self::MeshGradient(surface) => format!("MeshGradient({surface:?})"),
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
					Self::ContextFeatures(features) => format!("ContextFeatures({features:?})"),
					Self::EditorApi(_) => "PlatformEditorApi".to_string(),
					Self::ResourceHash(hash) => format!("ResourceHash({hash:?})"),
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
	/// A plain, always-present color. Aliases recover legacy on-disk shapes; a legacy `null` payload (the old "no color")
	/// is routed to [`TaggedValue::no_paint`] by `deserialize_tagged_value_with_legacy_migration`.
	#[serde(deserialize_with = "core_types::misc::migrate_to_color")] // TODO: Eventually remove this document upgrade code
	#[serde(alias = "ColorTable", alias = "OptionalColor", alias = "ColorNotInTable")]
	Color(Color),
	Font(Font),
	Footprint(Footprint),
	VectorModification(Box<VectorModification>),
	ImageData(Image<Color>),
	Resource(ResourceId),
	// Legacy
	#[serde(alias = "OptionalDAffine2")]
	LegacyOptionalDAffine2(Option<DAffine2>),
	#[serde(alias = "FillGradient")]
	LegacyGradient(graphic_types::migrations::legacy::LegacyGradient),
	// ==========
	// ENUM TYPES
	// ==========
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
	#[serde(alias = "GradientType")] // TODO: Eventually remove this document upgrade code
	GradientForm(vector::style::GradientForm),
	#[serde(alias = "GradientSpreadMethod")] // TODO: Eventually remove this document upgrade code
	GradientSpread(vector::style::GradientSpread),
	GradientSpace(vector::style::GradientSpace),
	GradientHueDirection(vector::style::GradientHueDirection),
	GradientInterpolation(vector::style::GradientInterpolation),
	ReferencePoint(vector::ReferencePoint),
	CentroidType(vector::misc::CentroidType),
	BooleanOperation(vector::misc::BooleanOperation),
	TextAlign(text_nodes::TextAlign),
	ScaleType(core_types::transform::ScaleType),
	// Legacy
	PaintOrder(vector::style::PaintOrder), // TODO: Eventually remove this document upgrade code
	#[serde(alias = "Fill")]
	LegacyFill(graphic_types::migrations::legacy::LegacyFill), // TODO: Eventually remove this document upgrade code
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
				let color = SRGBA8::from_hex_str(hex).map(Color::from);
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

		fn to_gradient(input: &str) -> Option<Gradient> {
			// String syntax: (e.g. "000000ff, ff0000ff")
			let stops = input.split(',').filter_map(|s| to_color(s.trim())).collect::<Vec<_>>();
			match stops.len() {
				0 => {
					log::error!("Invalid default value gradient string: {input}");
					None
				}
				1 => Some(Gradient::from(vec![stops[0], stops[0]])),
				_ => Some(Gradient::from(stops)),
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
					() if ty == TypeId::of::<Color>() => to_color(string).map(TaggedValue::Color)?,
					// The Fill/Stroke paint wires carry `Graphic` or `Gradient` elements, so a paint default parses through the element recursion as a color or gradient literal
					() if ty == TypeId::of::<Graphic>() => to_color(string).map(TaggedValue::Color)?,
					() if ty == TypeId::of::<Gradient>() => to_gradient(string).map(|gradient| TaggedValue::GradientRamp(gradient.into()))?,
					() if ty == TypeId::of::<ReferencePoint>() => to_reference_point(string).map(TaggedValue::ReferencePoint)?,
					() if ty == TypeId::of::<DashPattern>() => TaggedValue::DashPattern(core_types::misc::parse_f64_list(string)),
					() if ty == TypeId::of::<BoxCorners>() => TaggedValue::BoxCorners(core_types::misc::parse_f64_list(string)),
					_ => return None,
				};
				Some(ty)
			}
			Type::Fn(_, output) => TaggedValue::from_primitive_string(string, output),
			Type::Future(fut) => TaggedValue::from_primitive_string(string, fut),
			Type::Item(element) => TaggedValue::from_primitive_string(string, element),
			Type::List(element) => TaggedValue::from_primitive_string(string, element),
		}
	}

	pub fn to_u32(&self) -> u32 {
		match self {
			TaggedValue::U32(x) => *x,
			_ => panic!("Passed value is not of type u32"),
		}
	}

	/// The stored form of a paint input's red-slash "no paint" choice: the `Item<Graphic>` type default, materializing as a `Graphic::None` paint.
	pub fn no_paint() -> Self {
		TaggedValue::TypeDefault(item!(Graphic))
	}

	/// Whether this is the `Item<Graphic>` type default created by [`Self::no_paint`] (and by disconnecting a paint wire).
	pub fn is_no_paint(&self) -> bool {
		matches!(self, TaggedValue::TypeDefault(td) if *td == item!(Graphic))
	}
}

/// Custom deserializer hooked onto `NodeInput::Value::tagged_value` that intercepts removed-variant tags before delegating to `TaggedValue`'s standard derive.
///
/// Routes legacy variant names into modern variants, in typed Rust. Each legacy name is also matched against the historical `#[serde(alias = "...")]` spellings the deleted variant accepted, so old-shape inner payloads are caught:
///
/// - `BrushCache` → `TaggedValue::None` (purely runtime cache; no payload to preserve)
/// - `Graphic` (or alias `GraphicGroup`/`Group`) → `TaggedValue::TypeDefault(list!(Graphic))`
/// - `Artboard` (or alias `ArtboardGroup`) → `TaggedValue::TypeDefault(list!(Artboard))`
/// - `Raster` (or alias `ImageFrame`/`RasterData`/`Image`):
///     - non-empty (the legacy `image` proto's input 1, where the inner `Raster<CPU>` serializes as the embedded `Image<Color>`) → `TaggedValue::ImageData(<inner Image<Color>>)`
///     - empty → `TaggedValue::TypeDefault(list!(Raster<CPU>))`
/// - `Vector` (or alias `VectorData`):
///     - non-empty → `TaggedValue::VectorModification(<built from first element>)` (the document_migration's Path pass disambiguates this between SVG-import legacy and a discardable modern baked value via the input's `exposed` flag)
///     - empty → `TaggedValue::TypeDefault(list!(Vector))`
/// - `FillChoice` → `TaggedValue::Color` (solid), `TaggedValue::GradientRamp` (gradient), or `TaggedValue::no_paint()` (none)
/// - `Gradient` (or alias `GradientTable`/`GradientPositions`/`GradientStops`) → `TaggedValue::LegacyGradient` (ancient full struct) or `TaggedValue::GradientRamp` (ramp and legacy stops shapes, unwrapped from the legacy table form)
/// - `TypeDefault` with the old bare-`TypeDescriptor` payload → the same variant wrapping a `Type` (name-encoded `List` normalized to structural)
///
/// All other tags (including ones with the modern shape) fall through to the standard derived `Deserialize` for `TaggedValue`.
// TODO: Eventually remove this document upgrade code
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
			"Graphic" | "GraphicGroup" | "Group" => return Ok(MemoHash::new(TaggedValue::TypeDefault(list!(Graphic)))),
			"Artboard" | "ArtboardGroup" => return Ok(MemoHash::new(TaggedValue::TypeDefault(list!(Artboard)))),
			"Raster" | "ImageFrame" | "RasterData" | "Image" => {
				let first_element = content
					.as_object()
					.and_then(|c| c.get("element").or_else(|| c.get("instance")).or_else(|| c.get("instances")))
					.and_then(|e| e.as_array())
					.and_then(|arr| arr.first());
				if let Some(image_value) = first_element {
					let image: Image<Color> = serde_json::from_value(image_value.clone()).map_err(serde::de::Error::custom)?;
					return Ok(MemoHash::new(TaggedValue::ImageData(image)));
				}
				return Ok(MemoHash::new(TaggedValue::TypeDefault(list!(Raster<CPU>))));
			}
			"Vector" | "VectorData" => {
				let vector = graphic_types::migrations::migrate_to_optional_vector(content.clone()).map_err(serde::de::Error::custom)?;
				if let Some(vector) = vector {
					let modification = Box::new(VectorModification::create_from_vector(&vector));
					return Ok(MemoHash::new(TaggedValue::VectorModification(modification)));
				}
				return Ok(MemoHash::new(TaggedValue::TypeDefault(list!(Vector))));
			}
			// The `TypeDefault` payload used to be a bare `TypeDescriptor`; it now carries a `Type`
			"TypeDefault" if content.as_object().is_some_and(|c| c.contains_key("name")) => {
				let descriptor: TypeDescriptor = serde_json::from_value(content.clone()).map_err(serde::de::Error::custom)?;
				return Ok(MemoHash::new(TaggedValue::TypeDefault(Type::Concrete(descriptor).normalize_rank())));
			}
			// The `Color` tag used to carry `Option<Color>`, where a `null` payload (or an empty legacy color table) was the red-slash "no paint" choice
			"Color" | "ColorTable" | "OptionalColor" | "ColorNotInTable"
				if content.is_null()
					|| content
						.as_object()
						.and_then(|c| c.get("element").or_else(|| c.get("instance")).or_else(|| c.get("instances")))
						.and_then(|e| e.as_array())
						.is_some_and(|colors| colors.is_empty()) =>
			{
				return Ok(MemoHash::new(TaggedValue::no_paint()));
			}
			// The removed `FillChoice` variant decomposes into the plain paint values
			"FillChoice" => {
				if let Some(payload) = content.as_object() {
					if let Some(solid) = payload.get("Solid") {
						let color: Color = serde_json::from_value(solid.clone()).map_err(serde::de::Error::custom)?;
						return Ok(MemoHash::new(TaggedValue::Color(color)));
					}
					if let Some(gradient) = payload.get("Gradient") {
						let ramp = graphic_types::migrations::migrate_to_gradient_ramp(gradient.clone()).map_err(serde::de::Error::custom)?;
						return Ok(MemoHash::new(TaggedValue::GradientRamp(ramp)));
					}
				}
				return Ok(MemoHash::new(TaggedValue::no_paint()));
			}
			// The gradient tags carried several shapes over time, disambiguated here: the ancient full struct (`start`/`end` keys) becomes `LegacyGradient`,
			// while the current ramp, the flat stops struct, the old tuple list, and the legacy one-element table wrapper all parse as the ramp value directly
			"Gradient" | "GradientTable" | "GradientPositions" | "GradientStops" => {
				let table_element = content
					.as_object()
					.and_then(|c| c.get("element").or_else(|| c.get("instance")).or_else(|| c.get("instances")))
					.and_then(|element| element.as_array());

				// An empty legacy table wrapper carries no gradient, degrading to the default (in the era's gamma) rather than failing the document load
				if let Some(array) = table_element
					&& array.is_empty()
				{
					let ramp = GradientRamp {
						gradient_space: vector::style::GradientSpace::RgbGamma,
						..Default::default()
					};
					return Ok(MemoHash::new(TaggedValue::GradientRamp(ramp)));
				}

				let payload = table_element.and_then(|array| array.first()).unwrap_or(content);

				if payload.as_object().is_some_and(|c| c.contains_key("start") && c.contains_key("end")) {
					let gradient: graphic_types::migrations::legacy::LegacyGradient = serde_json::from_value(payload.clone()).map_err(serde::de::Error::custom)?;
					return Ok(MemoHash::new(TaggedValue::LegacyGradient(gradient)));
				}

				let ramp = graphic_types::migrations::migrate_to_gradient_ramp(payload.clone()).map_err(serde::de::Error::custom)?;
				return Ok(MemoHash::new(TaggedValue::GradientRamp(ramp)));
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
		Box::pin(async move { Box::new(Item::new_from_element(self.0.as_ref())) as DAny<'i> })
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
	Texture(graphene_application_io::Texture),
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

#[cfg(test)]
mod typedefault_dispatch {
	use super::*;
	use core_types::{concrete, item, list};

	/// Round-trips every type in the type-default lists through `TaggedValue::TypeDefault → to_dynany / to_any` and asserts the resulting concrete type matches the stored type.
	///
	/// This guards against the only way to break the recursion invariant in the unwrap functions: someone hand-rolling a `TypeDefault`-yielding case in `from_type` (or the macro's expansion in one of the unwrap sites silently failing to match a name). If it fails, the message points at the specific type and the structural reason.
	#[test]
	fn typedefault_dispatch_terminates() {
		macro_rules! check {
			($type_default:ty, $stored:expr) => {{
				let ty: Type = $stored;
				let expected_type_id = std::any::TypeId::of::<$type_default>();
				let dyn_value = TaggedValue::TypeDefault(ty.clone()).to_dynany();
				assert_eq!(
					DynAny::type_id(&*dyn_value),
					expected_type_id,
					"`to_dynany(TypeDefault({0}))` did not produce a `{0}` — the type-default lists cover this type but the unwrap site doesn't handle it. Without a match, `to_dynany` falls back to `from_type_or_none`, which returns `TypeDefault({0})` again and recurses forever.",
					std::any::type_name::<$type_default>(),
				);

				let arc_value = TaggedValue::TypeDefault(ty).to_any();
				assert_eq!(
					(*arc_value).type_id(),
					expected_type_id,
					"`to_any(TypeDefault({0}))` did not produce a `{0}` — same recursion hazard as above for the `to_any` path.",
					std::any::type_name::<$type_default>(),
				);
			}};
		}
		macro_rules! check_item {
			($element:ty) => {
				check!(Item<$element>, item!($element));
			};
		}
		macro_rules! check_list {
			($element:ty) => {
				check!(List<$element>, list!($element));
			};
		}
		macro_rules! check_bare {
			($type_default:ty) => {
				check!($type_default, concrete!($type_default));
			};
		}
		for_each_item_type_default!(check_item);
		for_each_list_type_default!(check_list);
		for_each_bare_type_default!(check_bare);
	}
}

#[cfg(test)]
mod paint_default_parsing {
	use super::*;
	use core_types::{item, list};

	/// A Fill/Stroke paint wire carries `Graphic` elements, so its `Color::BLACK` default must parse through the
	/// element recursion into a `Color` for a fresh Fill node's paint to resolve.
	#[test]
	fn paint_wire_parses_color_default_through_its_element() {
		let black = Some(TaggedValue::Color(Color::BLACK));
		assert_eq!(
			TaggedValue::from_primitive_string("Color::BLACK", &list!(Graphic)),
			black,
			"a `List<Graphic>` paint wire should resolve its color default"
		);
		assert_eq!(
			TaggedValue::from_primitive_string("Color::BLACK", &item!(Graphic)),
			black,
			"an `Item<Graphic>` paint wire should resolve its color default"
		);
	}

	/// Table-era documents stored the red-slash "no paint" fill as an empty color table, which must keep
	/// deserializing to [`TaggedValue::no_paint`] rather than collapsing to a transparent color.
	#[test]
	fn empty_legacy_color_table_deserializes_to_no_paint() {
		for payload in [r#"{"ColorTable": {"instances": []}}"#, r#"{"ColorTable": {"element": []}}"#, r#"{"Color": null}"#] {
			let mut deserializer = serde_json::Deserializer::from_str(payload);
			let value = deserialize_tagged_value_with_legacy_migration(&mut deserializer).expect("The legacy payload should deserialize");
			assert!(value.is_no_paint(), "The legacy payload `{payload}` should migrate to the no-paint choice");
		}
	}
}

#[cfg(test)]
mod gradient_shape_migration {
	use graphic_types::vector_types::{GradientSpace, GradientSpread};

	use super::*;

	fn load(payload: serde_json::Value) -> TaggedValue {
		deserialize_tagged_value_with_legacy_migration(payload)
			.expect("The gradient payload should deserialize")
			.into_inner()
			.as_ref()
			.clone()
	}

	fn white() -> serde_json::Value {
		serde_json::to_value(Color::WHITE).unwrap()
	}

	#[test]
	fn modern_ramp_payload_round_trips() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		gradient.set_positions(&[0.2, 0.9]);
		let value = TaggedValue::GradientRamp(GradientRamp {
			gradient_spread: GradientSpread::Reflect,
			..GradientRamp::from(gradient)
		});

		let json = serde_json::to_value(&value).unwrap();
		assert!(json.get("GradientRamp").and_then(|payload| payload.get("stops")).is_some(), "the payload should nest its stops: {json}");
		assert_eq!(
			json.get("GradientRamp").and_then(|payload| payload.get("gradient_space")),
			Some(&serde_json::json!("OkLab")),
			"the space should serialize even at its default, marking the ramp as post-legacy: {json}"
		);
		assert_eq!(load(json), value);
	}

	// TODO: Eventually remove this document upgrade code
	#[test]
	fn ramp_without_space_field_reads_as_legacy_gamma() {
		let json = serde_json::json!({ "GradientRamp": { "stops": { "color": [white(), white()] } } });
		let TaggedValue::GradientRamp(ramp) = load(json) else {
			panic!("the ramp payload should become a gradient ramp value")
		};

		assert_eq!(ramp.gradient_space, GradientSpace::RgbGamma, "a ramp saved before the field existed should read as gamma");
	}

	// TODO: Eventually remove this document upgrade code
	#[test]
	fn legacy_flat_stops_parse_faithfully() {
		let json = serde_json::json!({ "Gradient": { "color": [white(), white()], "position": [0., 0.25], "midpoint": [0.5, 0.5] } });
		let TaggedValue::GradientRamp(ramp) = load(json) else {
			panic!("the flat stops should become a gradient ramp value")
		};
		assert_eq!(ramp.gradient_space, GradientSpace::RgbGamma, "the pre-ramp flat form should carry the era's gamma");

		let gradient = Gradient::from(ramp);
		assert_eq!(gradient.positions(false), vec![0., 0.25]);
		assert!(gradient.has_midpoint_attribute(), "the flat form must parse faithfully");
	}

	// TODO: Eventually remove this document upgrade code
	#[test]
	fn legacy_tuple_stops_parse_with_defaults_elided() {
		let json = serde_json::json!({ "Gradient": [[0., white()], [1., white()]] });
		let TaggedValue::GradientRamp(ramp) = load(json) else {
			panic!("the tuple stops should become a gradient ramp value")
		};
		assert_eq!(ramp.gradient_space, GradientSpace::RgbGamma, "the pre-ramp tuple form should carry the era's gamma");

		let gradient = Gradient::from(ramp);
		assert_eq!(gradient.positions(false), vec![0., 1.]);
		assert!(!gradient.has_position_attribute(), "even legacy tuple positions should elide");
	}

	// TODO: Eventually remove this document upgrade code
	#[test]
	fn empty_legacy_gradient_table_degrades_to_the_default() {
		let json = serde_json::json!({ "GradientTable": { "element": [] } });
		let expected = GradientRamp {
			gradient_space: GradientSpace::RgbGamma,
			..Default::default()
		};
		assert_eq!(load(json), TaggedValue::GradientRamp(expected));
	}

	// TODO: Eventually remove this document upgrade code
	#[test]
	fn ancient_full_struct_routes_to_legacy_gradient() {
		let json = serde_json::json!({ "Gradient": { "stops": [[0., white()], [1., white()]], "gradient_type": "Linear", "start": [0., 0.], "end": [1., 0.] } });
		let TaggedValue::LegacyGradient(legacy) = load(json) else {
			panic!("the ancient full struct should become a legacy gradient value")
		};
		assert_eq!(
			Gradient::from(legacy.stops).positions(false),
			vec![0., 1.],
			"the nested tuple stops should parse through the field adapter"
		);
	}
}
