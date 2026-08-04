//! Attribute markers and their census. A marker declares an attribute name
//! once, fixing its value type and its name-specific default; the census
//! collects every declaration so name resolution, defaults, and diagnostics
//! run at graph compile time. One name belongs to one marker, so a name can
//! never mean two different types.

use crate::list::AnyAttributeValue;
use glam::{DAffine2, DVec2};
use std::any::TypeId;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Deref;
use std::sync::{LazyLock, Mutex};

/// Declares an attribute name: one marker per name, fixing the value type and
/// the name-specific default. Declare markers through the [`attribute!`]
/// macro, which also registers them into the [`ATTRIBUTE_REGISTRY`].
pub trait Attribute: 'static {
	/// The name as it appears in documents and diagnostics.
	const NAME: &'static str;
	/// The value type every read and write of this name shares.
	type Value: AnyAttributeValue + Clone + Default + std::fmt::Debug;
	/// The name-specific default, filled where an item lacks the attribute.
	fn default() -> Self::Value {
		Self::Value::default()
	}
}

/// A kernel-facing attribute value. A parameter `Attr<A>` is a read of `A`
/// (yielding the declared default where the attribute is absent upstream), an
/// `Attr<A>` in the return tuple is a write, and the same marker on both
/// sides is a modify.
pub struct Attr<A: Attribute>(pub A::Value);

impl<A: Attribute> Deref for Attr<A> {
	type Target = A::Value;

	fn deref(&self) -> &A::Value {
		&self.0
	}
}

impl<A: Attribute> Clone for Attr<A> {
	fn clone(&self) -> Self {
		Attr(self.0.clone())
	}
}

impl<A: Attribute> std::fmt::Debug for Attr<A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple(A::NAME).field(&self.0).finish()
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
	/// Whether the value is eligible for packed-record fields: no drop glue,
	/// so its bytes copy freely. Droppable values stay on the erased path
	/// until the per-type clone/drop glue lands.
	pub packable: bool,
	/// Writes the declared default's bytes into a `size`-long slice.
	/// Meaningful only when `packable`.
	pub write_default_bytes: fn(&mut [u8]),
}

fn write_default_bytes<A: Attribute>(out: &mut [u8]) {
	assert!(!std::mem::needs_drop::<A::Value>(), "default bytes exist only for packable values");
	assert_eq!(out.len(), size_of::<A::Value>());
	let value = A::default();
	unsafe { std::ptr::copy_nonoverlapping((&raw const value).cast::<u8>(), out.as_mut_ptr(), size_of::<A::Value>()) };
}

/// All declared attribute names, keyed by name.
pub static ATTRIBUTE_REGISTRY: LazyLock<Mutex<HashMap<&'static str, AttributeInfo>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Registers `A` into the census. Called by the [`attribute!`] expansion at
/// startup (ctor natively, a `__node_registry_attribute_*` export on wasm).
/// Re-registration at the same value type is idempotent; a second marker
/// claiming the name at a different value type panics.
pub fn register<A: Attribute>() {
	let info = AttributeInfo {
		name: A::NAME,
		value_type: TypeId::of::<A::Value>(),
		value_type_name: std::any::type_name::<A::Value>(),
		default: || Box::new(A::default()),
		size: size_of::<A::Value>(),
		align: align_of::<A::Value>(),
		packable: !std::mem::needs_drop::<A::Value>(),
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
///     /// The item's transformation.
///     pub Transform("transform"): glam::DAffine2;
/// }
/// ```
///
/// The trailing `= expr` is the name-specific default; without it the value
/// type's `Default` applies.
#[macro_export]
macro_rules! attribute {
	($($(#[$meta:meta])* $vis:vis $marker:ident($name:literal): $value:ty $(= $default:expr)?;)+) => {
		$(
			$(#[$meta])*
			$vis struct $marker;

			impl $crate::attribute::Attribute for $marker {
				const NAME: &'static str = $name;
				type Value = $value;
				$(
					fn default() -> $value {
						$default
					}
				)?
			}

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
		)+
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
	pub Name("name"): String;
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
		assert_eq!(<Name as Attribute>::default(), String::new());
	}

	#[test]
	fn erased_default_downcasts_to_the_declared_type() {
		let value = default_value("opacity_fill").unwrap();
		assert_eq!(*value.as_any().downcast_ref::<f64>().unwrap(), 1.);
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
			type Value = bool;
		}
		register::<Conflict>();
	}
}
