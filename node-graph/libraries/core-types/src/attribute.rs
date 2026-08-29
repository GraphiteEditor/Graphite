//! Attribute markers and their census. A marker declares an attribute name
//! once, fixing its value type and its name-specific default; the census
//! collects every declaration so name resolution, defaults, and diagnostics
//! run at graph compile time. One name belongs to one marker, so a name can
//! never mean two different types.
//!
//! Values are `Copy` and pack directly into record fields. Data with drop
//! glue rides the arena instead: the marker declares a reference value
//! (`&str`), the writing kernel parks the payload in the arena, and the
//! record field carries the eval-lifetime reference.

use crate::list::AnyAttributeValue;
use glam::{DAffine2, DVec2};
use std::any::TypeId;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{LazyLock, Mutex};

/// Declares an attribute name: one marker per name, fixing the value type and
/// the name-specific default. Declare markers through the [`attribute!`]
/// macro, which also registers them into the [`ATTRIBUTE_REGISTRY`].
pub trait Attribute: 'static {
	/// The name as it appears in documents and diagnostics.
	const NAME: &'static str;
	/// The value type every read and write of this name shares. The lifetime
	/// is the evaluation the value flows in; non-reference values ignore it.
	/// The value outlives that evaluation, so its `'static` instantiation is
	/// the one the census registers and layouts stamp their type id from.
	type Value<'e>: Copy + Default + std::fmt::Debug + 'e;
	/// The name-specific default, filled where an item lacks the attribute.
	/// Producing a value for any `'e` from no inputs, reference defaults can
	/// only point at `'static` data, which is what lets the census fill them
	/// as plain bytes.
	fn default<'e>() -> Self::Value<'e> {
		Default::default()
	}

	/// Borrows the value out of legacy list storage, whose stored form is the
	/// owned clone [`Self::read_erased`] produces. `None` where the column is
	/// absent or holds another type.
	fn from_stored<'a>(stored: &'a dyn std::any::Any) -> Option<Self::Value<'a>>;

	/// # Safety
	/// `ptr` must point at a live field of this marker's value type.
	unsafe fn read_erased(ptr: *const u8) -> Box<dyn AnyAttributeValue>;

	/// Re-parks the owned clone [`Self::read_erased`] produced into fresh
	/// field storage; `None` for plain values, which ride the byte copy.
	const REPARK: Option<unsafe fn(&dyn AnyAttributeValue, *mut u8, &crate::arena::Arena) -> Option<()>> = None;
}

/// A kernel-facing attribute value. A parameter `Attr<A>` is a read of `A`
/// (yielding the declared default where the attribute is absent upstream), an
/// `Attr<A>` in the return tuple is a write, and the same marker on both
/// sides is a modify.
pub struct Attr<'e, A: Attribute>(pub A::Value<'e>);

impl<'e, A: Attribute> Deref for Attr<'e, A> {
	type Target = A::Value<'e>;

	fn deref(&self) -> &A::Value<'e> {
		&self.0
	}
}

impl<'e, A: Attribute> Clone for Attr<'e, A> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'e, A: Attribute> Copy for Attr<'e, A> {}

impl<'e, A: Attribute> std::fmt::Debug for Attr<'e, A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple(A::NAME).field(&self.0).finish()
	}
}

/// A deletion of `A` in a node's return tuple: the name leaves the output
/// layout, so downstream reads yield the declared default again. Functionally
/// a write of the default; the value carries nothing.
pub struct RemoveAttr<A: Attribute>(PhantomData<A>);

impl<A: Attribute> RemoveAttr<A> {
	pub const fn new() -> Self {
		RemoveAttr(PhantomData)
	}
}

impl<A: Attribute> Default for RemoveAttr<A> {
	fn default() -> Self {
		Self::new()
	}
}

/// A census row: what is known about one declared attribute name.
#[derive(Clone, Copy, Debug)]
pub struct AttributeInfo {
	pub name: &'static str,
	pub value_type: TypeId,
	pub value_type_name: &'static str,
	pub default: fn() -> Box<dyn AnyAttributeValue>,
	pub size: usize,
	pub align: usize,
	/// Writes the declared default's bytes into a `size`-long slice.
	pub write_default_bytes: fn(&mut [u8]),
	/// The marker's field form at the given level, for layouts built at runtime.
	pub field_write_at: fn(u8) -> crate::record::FieldWrite,
	/// Writes a legacy stored value into a field of this marker, parking
	/// droppable payloads. A wrong-typed stored value leaves the field
	/// untouched; `None` reports arena exhaustion.
	pub write_stored: unsafe fn(&dyn AnyAttributeValue, *mut u8, &crate::arena::Arena) -> Option<()>,
}

fn write_default_bytes<A: Attribute>(out: &mut [u8]) {
	assert_eq!(out.len(), size_of::<A::Value<'static>>());
	let value: A::Value<'static> = A::default();
	unsafe { std::ptr::copy_nonoverlapping((&raw const value).cast::<u8>(), out.as_mut_ptr(), size_of::<A::Value<'static>>()) };
}

fn field_write_at<A: Attribute>(level: u8) -> crate::record::FieldWrite
where
	A::Value<'static>: graphene_hash::CacheHash + PartialEq,
{
	crate::record::FieldWrite::of::<A>(level)
}

unsafe fn write_stored<A: Attribute>(stored: &dyn AnyAttributeValue, dst: *mut u8, arena: &crate::arena::Arena) -> Option<()> {
	if A::from_stored(stored.as_any()).is_none() {
		// A wrong-typed stored value reads as absent, so the field keeps its default.
		return Some(());
	}
	match A::REPARK {
		Some(repark) => unsafe { repark(stored, dst, arena) },
		None => {
			let value = A::from_stored(stored.as_any()).expect("checked above");
			// SAFETY: a marker without re-park glue stores a plain value, so the bytes carry no borrowed data.
			unsafe { dst.cast::<A::Value<'_>>().write(value) };
			Some(())
		}
	}
}

/// All declared attribute names, keyed by name.
pub static ATTRIBUTE_REGISTRY: LazyLock<Mutex<HashMap<&'static str, AttributeInfo>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Registers `A` into the census. Called by the [`attribute!`] expansion at
/// startup (ctor natively, a `__node_registry_attribute_*` export on wasm).
/// Re-registration at the same value type is idempotent; a second marker
/// claiming the name at a different value type panics.
pub fn register<A: Attribute>()
where
	A::Value<'static>: AnyAttributeValue + graphene_hash::CacheHash + PartialEq,
{
	let info = AttributeInfo {
		name: A::NAME,
		value_type: TypeId::of::<A::Value<'static>>(),
		value_type_name: std::any::type_name::<A::Value<'static>>(),
		default: || Box::new(A::default()),
		size: size_of::<A::Value<'static>>(),
		align: align_of::<A::Value<'static>>(),
		write_default_bytes: write_default_bytes::<A>,
		field_write_at: field_write_at::<A>,
		write_stored: write_stored::<A>,
	};
	let conflict = match ATTRIBUTE_REGISTRY.lock().unwrap().entry(A::NAME) {
		Entry::Vacant(vacant) => {
			vacant.insert(info);
			None
		}
		Entry::Occupied(occupied) => (occupied.get().value_type != info.value_type).then(|| occupied.get().value_type_name),
	};
	if let Some(existing) = conflict {
		panic!("attribute `{}` is declared at two value types: {existing} and {}", A::NAME, info.value_type_name);
	}
}

/// Looks up a declared name.
pub fn info(name: &str) -> Option<AttributeInfo> {
	ATTRIBUTE_REGISTRY.lock().unwrap().get(name).copied()
}

/// The name-specific default for `name`, if the name is declared.
pub fn default_value(name: &str) -> Option<Box<dyn AnyAttributeValue>> {
	info(name).map(|info| (info.default)())
}

/// Declares attribute markers: for each entry, the marker struct, its
/// [`Attribute`] impl, and the census registration.
///
/// ```
/// core_types::attribute! {
///     /// How visible the content is.
///     pub Opacity("opacity"): f64 = 1.;
///     /// The item's label, parked in the arena by the writer.
///     pub Label("label"): &str;
/// }
/// ```
///
/// The trailing `= expr` is the name-specific default; without it the value
/// type's `Default` applies. A `&T` value carries the eval lifetime, so its
/// default must be `'static` data. An `Option<&T>` value is an optional
/// parked reference whose default is `None`, for attributes whose absence
/// means something a present value cannot.
#[macro_export]
macro_rules! attribute {
	() => {};
	($(#[$meta:meta])* $vis:vis $marker:ident($name:literal): Option<&$value:ty>; $($rest:tt)*) => {
		$(#[$meta])*
		$vis struct $marker;

		impl $crate::attribute::Attribute for $marker {
			const NAME: &'static str = $name;
			type Value<'e> = ::core::option::Option<&'e $value>;

			fn from_stored<'a>(stored: &'a dyn ::std::any::Any) -> ::core::option::Option<Self::Value<'a>> {
				stored
					.downcast_ref::<::core::option::Option<<$value as ::std::borrow::ToOwned>::Owned>>()
					.map(|owned| owned.as_ref().map(::std::borrow::Borrow::borrow))
			}

			unsafe fn read_erased(ptr: *const u8) -> ::std::boxed::Box<dyn $crate::list::AnyAttributeValue> {
				::std::boxed::Box::new(unsafe { ptr.cast::<::core::option::Option<&$value>>().read() }.map(|value| <$value as ::std::borrow::ToOwned>::to_owned(value)))
			}

			const REPARK: ::core::option::Option<unsafe fn(&dyn $crate::list::AnyAttributeValue, *mut u8, &$crate::arena::Arena) -> ::core::option::Option<()>> = {
				unsafe fn repark(value: &dyn $crate::list::AnyAttributeValue, dst: *mut u8, arena: &$crate::arena::Arena) -> ::core::option::Option<()> {
					let owned: &::core::option::Option<<$value as ::std::borrow::ToOwned>::Owned> =
						value.as_any().downcast_ref().expect("an optional reference attribute replays its owned clone");
					let parked = match owned {
						::core::option::Option::Some(owned) => {
							let (parked, _) = arena.alloc(<$value as ::std::borrow::ToOwned>::to_owned(::std::borrow::Borrow::borrow(owned)))?;
							::core::option::Option::Some(::std::borrow::Borrow::borrow(parked))
						}
						::core::option::Option::None => ::core::option::Option::None,
					};
					unsafe { dst.cast::<::core::option::Option<&$value>>().write(parked) };
					::core::option::Option::Some(())
				}
				::core::option::Option::Some(repark)
			};
		}

		$crate::attribute!(@register $marker);
		$crate::attribute!($($rest)*);
	};
	($(#[$meta:meta])* $vis:vis $marker:ident($name:literal): &$value:ty $(= $default:expr)?; $($rest:tt)*) => {
		$(#[$meta])*
		$vis struct $marker;

		impl $crate::attribute::Attribute for $marker {
			const NAME: &'static str = $name;
			type Value<'e> = &'e $value;
			$(
				fn default<'e>() -> Self::Value<'e> {
					$default
				}
			)?

			fn from_stored<'a>(stored: &'a dyn ::std::any::Any) -> ::core::option::Option<Self::Value<'a>> {
				stored.downcast_ref::<<$value as ::std::borrow::ToOwned>::Owned>().map(::std::borrow::Borrow::borrow)
			}

			unsafe fn read_erased(ptr: *const u8) -> ::std::boxed::Box<dyn $crate::list::AnyAttributeValue> {
				::std::boxed::Box::new(unsafe { ptr.cast::<&$value>().read() }.to_owned())
			}

			const REPARK: ::core::option::Option<unsafe fn(&dyn $crate::list::AnyAttributeValue, *mut u8, &$crate::arena::Arena) -> ::core::option::Option<()>> = {
				unsafe fn repark(value: &dyn $crate::list::AnyAttributeValue, dst: *mut u8, arena: &$crate::arena::Arena) -> ::core::option::Option<()> {
					let owned: &<$value as ::std::borrow::ToOwned>::Owned = value.as_any().downcast_ref().expect("a reference attribute replays its owned clone");
					let (parked, _) = arena.alloc(<$value as ::std::borrow::ToOwned>::to_owned(::std::borrow::Borrow::borrow(owned)))?;
					unsafe { dst.cast::<&$value>().write(::std::borrow::Borrow::borrow(parked)) };
					::core::option::Option::Some(())
				}
				::core::option::Option::Some(repark)
			};
		}

		$crate::attribute!(@register $marker);
		$crate::attribute!($($rest)*);
	};
	($(#[$meta:meta])* $vis:vis $marker:ident($name:literal): $value:ty $(= $default:expr)?; $($rest:tt)*) => {
		$(#[$meta])*
		$vis struct $marker;

		impl $crate::attribute::Attribute for $marker {
			const NAME: &'static str = $name;
			type Value<'e> = $value;
			$(
				fn default<'e>() -> Self::Value<'e> {
					$default
				}
			)?

			fn from_stored<'a>(stored: &'a dyn ::std::any::Any) -> ::core::option::Option<Self::Value<'a>> {
				stored.downcast_ref::<$value>().copied()
			}

			unsafe fn read_erased(ptr: *const u8) -> ::std::boxed::Box<dyn $crate::list::AnyAttributeValue> {
				::std::boxed::Box::new(unsafe { ptr.cast::<$value>().read() })
			}
		}

		$crate::attribute!(@register $marker);
		$crate::attribute!($($rest)*);
	};
	(@register $marker:ident) => {
		const _: () = {
			#[cfg(not(target_family = "wasm"))]
			#[$crate::ctor::ctor]
			fn register() {
				$crate::attribute::register::<$marker>();
			}

			#[cfg(target_family = "wasm")]
			#[unsafe(export_name = concat!("__node_registry_attribute_", stringify!($marker)))]
			extern "C" fn register() {
				$crate::attribute::register::<$marker>();
			}
		};
	};
}

attribute! {
	/// Item's `DAffine2` transformation, composed multiplicatively through nested groups.
	pub Transform("transform"): DAffine2;
	/// Item's `BlendMode`, controlling how it composites with content beneath it.
	pub BlendMode("blend_mode"): crate::blending::BlendMode;
	/// Item's opacity multiplier, composed multiplicatively through nested groups.
	/// Affects content clipped to the item.
	pub Opacity("opacity"): f64 = 1.;
	/// Item's fill opacity multiplier. Like opacity but does not affect content clipped to the item.
	pub OpacityFill("opacity_fill"): f64 = 1.;
	/// Whether an item inherits the alpha of the content beneath it (clipping mask).
	pub ClippingMask("clipping_mask"): bool;
	/// The document node path of the editor layer owning the item.
	/// Editor tools read it to route clicks and selection back to the originating layer.
	pub EditorLayerPath("editor:layer_path"): &[crate::uuid::NodeId];
	/// Maps the unit square `[(0, 0), (1, 1)]` (top-left convention) onto the 'Text' node's
	/// text frame in this item's local space. Each item carries the frame relative to its own
	/// glyph origin so it survives 'Index Elements' filtering. The Text tool reads this to
	/// position its drag cage. Stored as an affine to allow non-axis-aligned frames in the future.
	pub EditorTextFrame("editor:text_frame"): DAffine2;
	/// Byte offset where a regex match begins ('Regex Find All' and 'Regex Capture' text nodes).
	pub Start("start"): u64;
	/// Byte offset where a regex match ends ('Regex Find All' and 'Regex Capture' text nodes).
	pub End("end"): u64;
	/// A regex named-capture-group's name, or empty for unnamed groups.
	pub Name("name"): &str;
	/// A JSON value's type (`"string"`, `"number"`, `"object"`, etc.) from 'JSON Query All'.
	pub Type("type"): &str;
	/// Artboard's top-left corner in document coordinates.
	pub Location("location"): DVec2;
	/// Artboard's width and height.
	pub Dimensions("dimensions"): DVec2;
	/// Artboard's background fill.
	pub Background("background"): crate::Color;
	/// Whether an artboard clips content to its bounds.
	pub Clip("clip"): bool;
	/// Text item's font size in document-space units.
	pub FontSize("font_size"): f64 = 24.;
	/// Text item's line height as a ratio of the font size.
	pub LineHeight("line_height"): f64 = 1.2;
	/// Text item's extra spacing between letters in document-space units.
	pub LetterSpacing("letter_spacing"): f64;
	/// Text item's maximum line-wrap width in document-space units.
	pub MaxWidth("max_width"): Option<f64>;
	/// Text item's maximum block height in document-space units, past which lines are not drawn.
	pub MaxHeight("max_height"): Option<f64>;
	/// Text item's faux-italic letter tilt angle in degrees.
	pub LetterTilt("letter_tilt"): f64;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn census_carries_declared_names() {
		let row = info("opacity").unwrap();
		assert_eq!(row.value_type, TypeId::of::<f64>());
		assert_eq!(info("transform").unwrap().value_type, TypeId::of::<DAffine2>());
		assert_eq!(info("max_width").unwrap().value_type, TypeId::of::<Option<f64>>());
		assert_eq!(info("background").unwrap().value_type, TypeId::of::<crate::Color>());
		assert!(info("never_declared").is_none());
	}

	#[test]
	fn name_specific_default_overrides_the_type_default() {
		assert_eq!(<Opacity as Attribute>::default(), 1.);
		assert_eq!(<Name as Attribute>::default(), "");
		assert_eq!(<FontSize as Attribute>::default(), 24.);
		assert_eq!(<LineHeight as Attribute>::default(), 1.2);
		assert_eq!(<MaxWidth as Attribute>::default(), None);
	}

	#[test]
	fn erased_default_downcasts_to_the_declared_type() {
		let value = default_value("opacity_fill").unwrap();
		assert_eq!(*value.as_any().downcast_ref::<f64>().unwrap(), 1.);
	}

	#[test]
	fn reference_values_register_at_the_static_instantiation() {
		let row = info("name").unwrap();
		assert_eq!(row.value_type, TypeId::of::<&'static str>());
		assert_eq!(row.size, size_of::<&str>());
	}

	#[test]
	fn reregistration_at_the_same_type_is_idempotent() {
		register::<Opacity>();
		register::<Opacity>();
		assert_eq!(info("opacity").unwrap().value_type, TypeId::of::<f64>());
	}

	#[test]
	#[should_panic(expected = "two value types")]
	fn a_second_marker_at_a_different_type_panics() {
		struct Conflict;
		impl Attribute for Conflict {
			const NAME: &'static str = "opacity";
			type Value<'e> = bool;

			fn from_stored<'a>(stored: &'a dyn std::any::Any) -> Option<Self::Value<'a>> {
				stored.downcast_ref::<bool>().copied()
			}

			unsafe fn read_erased(ptr: *const u8) -> Box<dyn AnyAttributeValue> {
				Box::new(unsafe { ptr.cast::<bool>().read() })
			}
		}
		register::<Conflict>();
	}
}
