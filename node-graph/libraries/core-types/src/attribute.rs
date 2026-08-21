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
	type Value<'e>: Copy + Default + std::fmt::Debug;
	/// The name-specific default, filled where an item lacks the attribute.
	/// Producing a value for any `'e` from no inputs, reference defaults can
	/// only point at `'static` data, which is what lets the census fill them
	/// as plain bytes.
	fn default<'e>() -> Self::Value<'e> {
		Default::default()
	}

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
}

fn write_default_bytes<A: Attribute>(out: &mut [u8]) {
	assert_eq!(out.len(), size_of::<A::Value<'static>>());
	let value: A::Value<'static> = A::default();
	unsafe { std::ptr::copy_nonoverlapping((&raw const value).cast::<u8>(), out.as_mut_ptr(), size_of::<A::Value<'static>>()) };
}

/// All declared attribute names, keyed by name.
pub static ATTRIBUTE_REGISTRY: LazyLock<Mutex<HashMap<&'static str, AttributeInfo>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Registers `A` into the census. Called by the [`attribute!`] expansion at
/// startup (ctor natively, a `__node_registry_attribute_*` export on wasm).
/// Re-registration at the same value type is idempotent; a second marker
/// claiming the name at a different value type panics.
pub fn register<A: Attribute>()
where
	A::Value<'static>: AnyAttributeValue,
{
	let info = AttributeInfo {
		name: A::NAME,
		value_type: TypeId::of::<A::Value<'static>>(),
		value_type_name: std::any::type_name::<A::Value<'static>>(),
		default: || Box::new(A::default()),
		size: size_of::<A::Value<'static>>(),
		align: align_of::<A::Value<'static>>(),
		write_default_bytes: write_default_bytes::<A>,
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
/// default must be `'static` data.
#[macro_export]
macro_rules! attribute {
	() => {};
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
	/// Artboard's top-left corner in document coordinates.
	pub Location("location"): DVec2;
	/// A regex named-capture-group's name, or empty for unnamed groups.
	pub Name("name"): &str;
	/// The document node path of the editor layer owning the item.
	pub EditorLayerPath("editor:layer_path"): &[crate::uuid::NodeId];
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn census_carries_declared_names() {
		let row = info("opacity").unwrap();
		assert_eq!(row.value_type, TypeId::of::<f64>());
		assert_eq!(info("transform").unwrap().value_type, TypeId::of::<DAffine2>());
		assert!(info("never_declared").is_none());
	}

	#[test]
	fn name_specific_default_overrides_the_type_default() {
		assert_eq!(<Opacity as Attribute>::default(), 1.);
		assert_eq!(<Name as Attribute>::default(), "");
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

			unsafe fn read_erased(ptr: *const u8) -> Box<dyn AnyAttributeValue> {
				Box::new(unsafe { ptr.cast::<bool>().read() })
			}
		}
		register::<Conflict>();
	}
}
