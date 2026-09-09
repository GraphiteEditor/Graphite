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
/// macro, which also registers them into the [`ATTRIBUTE_REGISTRY`] and emits
/// an impl meeting the obligations below.
///
/// # Safety
///
/// [`REPARK`](Self::REPARK) must be `Some` for every marker whose
/// [`Value<'e>`](Self::Value) can carry a borrow shorter than `'static`. The
/// plain-value arm of the census writer retypes the value [`from_stored`] hands
/// it from the stored form's lifetime to the field's, which is only a relabel
/// where the value borrows nothing; a re-parking marker instead writes a
/// reference into the arena it is given.
///
/// [`from_stored`](Self::from_stored) must accept exactly the erased form
/// [`read_erased`](Self::read_erased) produces, and [`read_erased`] must read
/// its `ptr` as this marker's own `Value`. The two are each other's inverse
/// across every persistence seam, so a marker that reads one type and stores
/// another writes a value of the wrong type into the field.
pub unsafe trait Attribute: 'static {
	/// The name as it appears in documents and diagnostics.
	const NAME: &'static str;
	/// The value type every read and write of this name shares. The lifetime
	/// is the evaluation the value flows in; non-reference values ignore it.
	/// The value outlives that evaluation, so its `'static` instantiation is
	/// the one the census registers and layouts stamp their type id from.
	///
	/// `Send + Sync` because a record's bytes are these values: `RecordValue`
	/// is `Send + Sync` over whatever the field writes put there, and the write
	/// path never consults the census, so the bound has to sit here.
	type Value<'e>: Copy + Default + std::fmt::Debug + Send + Sync + 'e;
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
	const REPARK: Option<crate::list::ReparkFn> = None;
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

/// An attribute write that outlives the evaluation producing it: an async
/// source's slot persists across generations, so a reference value cannot
/// cross as itself. The value crosses deep-copied through the field glue and
/// parks into the serving arena at every lift, which is why the copy is paid
/// once per invocation rather than once per evaluation.
pub struct OwnedAttr<A: Attribute>(Box<dyn AnyAttributeValue>, PhantomData<fn() -> A>);

impl<A: Attribute> OwnedAttr<A> {
	/// Deep-copies `value` out of the evaluation that produced it.
	pub fn new(value: A::Value<'_>) -> Self {
		// SAFETY: the read addresses a live local of the marker's value type.
		let erased = unsafe { A::read_erased((&raw const value).cast()) };
		OwnedAttr(crate::record::deepen_field_value(erased), PhantomData)
	}

	/// Parks the copy into `arena` for one evaluation; `None` reports arena
	/// exhaustion.
	pub fn park<'e>(&self, arena: &'e crate::arena::Arena) -> Option<A::Value<'e>> {
		let resident = crate::record::replay_field_value(&*self.0, arena)?;
		let mut value = A::default();
		// SAFETY: the slot is a live field of the marker's value type, and the
		// stored value is the copy `new` took at that same type.
		unsafe { write_stored::<A>(resident.as_deref().unwrap_or(&*self.0), (&raw mut value).cast(), arena) }?;
		Some(value)
	}
}

impl<A: Attribute> Clone for OwnedAttr<A> {
	fn clone(&self) -> Self {
		OwnedAttr(self.0.clone(), PhantomData)
	}
}

impl<A: Attribute> std::fmt::Debug for OwnedAttr<A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple(A::NAME).field(&self.0.display_string()).finish()
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

/// The value-type half of an attribute: everything [`Attribute`] declares
/// except the name and its name-specific default. A name-generic write pairs
/// one of these with a name taken from the graph, so the value type stays
/// concrete in the signature while the name varies per instance.
///
/// # Safety
///
/// The obligations are [`Attribute`]'s, at this trait's value type:
/// [`REPARK`](Self::REPARK) must be `Some` for every row whose
/// [`Value<'e>`](Self::Value) can carry a borrow shorter than `'static`, and
/// [`from_stored`](Self::from_stored) and [`read_erased`](Self::read_erased)
/// must be each other's inverse.
pub unsafe trait AttrValue: 'static {
	/// The value type every write of this row shares, as [`Attribute::Value`].
	type Value<'e>: Copy + Default + std::fmt::Debug + Send + Sync + 'e;

	/// Borrows the value out of legacy list storage, as [`Attribute::from_stored`].
	fn from_stored<'a>(stored: &'a dyn std::any::Any) -> Option<Self::Value<'a>>;

	/// # Safety
	/// `ptr` must point at a live field of this row's value type.
	unsafe fn read_erased(ptr: *const u8) -> Box<dyn AnyAttributeValue>;

	/// Re-parks an owned clone into fresh field storage, as [`Attribute::REPARK`].
	const REPARK: Option<crate::list::ReparkFn> = None;
}

/// The [`Attribute::NAME`] a [`Named`] marker carries before the compiler
/// fills it. A layout never holds it: the fold replaces it with the
/// instance's constant, and a node whose name input is not constant is
/// refused at that same point.
pub const NAMED_PLACEHOLDER: &str = "";

/// A name-generic attribute write. `X` is a placeholder that distinguishes
/// name-generic attributes within one signature, so a node writing two of
/// them takes two names; `V` fixes the value type. The name is absent by
/// construction: it comes from the instance's constant text input, folded
/// into the layout at graph compile time, so a computed name cannot exist.
///
/// Written `Named<X>` in parameter position, it declares where `X`'s name is
/// wired: the macro gives that input constant text, and the kernel receives
/// only the placeholder, since a folded name is a layout fact rather than a
/// value the kernel needs.
pub struct Named<X, V = Text>(PhantomData<fn() -> (X, V)>);

impl<X, V> Default for Named<X, V> {
	fn default() -> Self {
		Named(PhantomData)
	}
}

/// The placeholders a signature distinguishes its name-generic attributes by.
/// A node writing one name uses [`Name0`]; a second name on the same node
/// takes [`Name1`], and so on, which is all the placeholder has to do.
pub struct Name0;
/// The second name-generic attribute in one signature. See [`Name0`].
pub struct Name1;
/// The third name-generic attribute in one signature. See [`Name0`].
pub struct Name2;

// SAFETY: every obligation is discharged by `V`, which carries the same
// contract at the same value type; only the name differs, and the compiler
// fold replaces the placeholder before a layout sees it.
unsafe impl<X: 'static, V: AttrValue> Attribute for Named<X, V> {
	const NAME: &'static str = NAMED_PLACEHOLDER;
	type Value<'e> = V::Value<'e>;

	fn from_stored<'a>(stored: &'a dyn std::any::Any) -> Option<Self::Value<'a>> {
		V::from_stored(stored)
	}

	unsafe fn read_erased(ptr: *const u8) -> Box<dyn AnyAttributeValue> {
		// SAFETY: the caller's contract, at `V`'s own value type.
		unsafe { V::read_erased(ptr) }
	}

	const REPARK: Option<crate::list::ReparkFn> = V::REPARK;
}

/// The value a name-generic write takes off the wire, and the row it lands
/// in. A plain row is its own wire form; a reference row's wire form is the
/// owned payload the kernel parks in the arena, so the field can carry a
/// borrow of it for the evaluation.
pub trait WireValue: 'static {
	/// The row this value is written at, fixing the field's value type.
	type Row: AttrValue;

	/// Moves the value into `arena` where the row borrows it, or hands it back
	/// unchanged where the row stores it plainly. `None` reports exhaustion.
	fn park<'e>(self, arena: &'e crate::arena::Arena) -> Option<<Self::Row as AttrValue>::Value<'e>>;
}

/// Declares [`WireValue`] rows. `for T` is a plain value, carried and stored
/// as itself; `Wire => Row` parks `Wire`'s payload in the arena and stores the
/// borrow `Row` names.
#[macro_export]
macro_rules! wire_value {
	() => {};
	(for $value:ty; $($rest:tt)*) => {
		impl $crate::attribute::WireValue for $value {
			type Row = $value;

			fn park<'e>(self, _: &'e $crate::arena::Arena) -> ::core::option::Option<$value> {
				::core::option::Option::Some(self)
			}
		}

		$crate::wire_value!($($rest)*);
	};
	($wire:ty => $row:ty; $($rest:tt)*) => {
		impl $crate::attribute::WireValue for $wire {
			type Row = $row;

			fn park<'e>(self, arena: &'e $crate::arena::Arena) -> ::core::option::Option<<$row as $crate::attribute::AttrValue>::Value<'e>> {
				let (parked, _) = arena.alloc(self)?;
				::core::option::Option::Some(::std::borrow::Borrow::borrow(parked))
			}
		}

		$crate::wire_value!($($rest)*);
	};
}

wire_value! {
	for f64;
	for u32;
	for u64;
	for bool;
	for DVec2;
	for DAffine2;
	for crate::Color;
	for crate::blending::BlendMode;
	::std::vec::Vec<crate::uuid::NodeId> => NodeIdPath;
	::std::string::String => Text;
}

/// Interns a folded attribute name for the `&'static str` a layout field
/// holds. Census names never reach here; a document's novel names are finite
/// and repeat across instances, so the leak is one allocation per name.
pub fn intern_name(name: &str) -> &'static str {
	static NAMES: LazyLock<Mutex<std::collections::HashSet<&'static str>>> = LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));
	let mut names = NAMES.lock().unwrap();
	if let Some(interned) = names.get(name) {
		return interned;
	}
	let interned: &'static str = Box::leak(name.to_owned().into_boxed_str());
	names.insert(interned);
	interned
}

/// Declares [`AttrValue`] rows, the value types a name-generic attribute can
/// be written at. `for T` implements the row on an existing plain value type;
/// `Row: &T` and `Row: Option<&T>` declare a token naming a reference value,
/// whose payload the writing kernel parks in the arena.
///
/// ```
/// core_types::named_value! {
///     /// Plain rows, implemented on the value type itself.
///     for f64;
///     /// A reference row, whose token names the borrowed value.
///     pub Text: &str;
/// }
/// ```
#[macro_export]
macro_rules! named_value {
	() => {};
	($(#[$meta:meta])* for $value:ty; $($rest:tt)*) => {
		// SAFETY: a plain value type cannot name `'e`, so no re-park is owed,
		// and the two glue fns below are each other's inverse at `$value`.
		unsafe impl $crate::attribute::AttrValue for $value {
			type Value<'e> = $value;

			fn from_stored<'a>(stored: &'a dyn ::std::any::Any) -> ::core::option::Option<Self::Value<'a>> {
				stored.downcast_ref::<$value>().copied()
			}

			unsafe fn read_erased(ptr: *const u8) -> ::std::boxed::Box<dyn $crate::list::AnyAttributeValue> {
				::std::boxed::Box::new(unsafe { ptr.cast::<$value>().read() })
			}
		}

		$crate::named_value!($($rest)*);
	};
	($(#[$meta:meta])* $vis:vis $row:ident: Option<&$value:ty>; $($rest:tt)*) => {
		$(#[$meta])*
		$vis struct $row;

		// SAFETY: the reference arm emits `REPARK`, and the two glue fns below
		// are each other's inverse at `Option<&$value>`.
		unsafe impl $crate::attribute::AttrValue for $row {
			type Value<'e> = ::core::option::Option<&'e $value>;

			fn from_stored<'a>(stored: &'a dyn ::std::any::Any) -> ::core::option::Option<Self::Value<'a>> {
				stored
					.downcast_ref::<::core::option::Option<<$value as ::std::borrow::ToOwned>::Owned>>()
					.map(|owned| owned.as_ref().map(::std::borrow::Borrow::borrow))
			}

			unsafe fn read_erased(ptr: *const u8) -> ::std::boxed::Box<dyn $crate::list::AnyAttributeValue> {
				::std::boxed::Box::new(unsafe { ptr.cast::<::core::option::Option<&$value>>().read() }.map(|value| <$value as ::std::borrow::ToOwned>::to_owned(value)))
			}

			const REPARK: ::core::option::Option<$crate::list::ReparkFn> = {
				unsafe fn repark(value: &dyn $crate::list::AnyAttributeValue, dst: *mut u8, arena: &$crate::arena::Arena) -> ::core::option::Option<()> {
					let owned: &::core::option::Option<<$value as ::std::borrow::ToOwned>::Owned> =
						value.as_any().downcast_ref().expect("an optional reference row replays its owned clone");
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

		$crate::named_value!($($rest)*);
	};
	($(#[$meta:meta])* $vis:vis $row:ident: &$value:ty; $($rest:tt)*) => {
		$(#[$meta])*
		$vis struct $row;

		// SAFETY: the reference arm emits `REPARK`, and the two glue fns below
		// are each other's inverse at `&$value`.
		unsafe impl $crate::attribute::AttrValue for $row {
			type Value<'e> = &'e $value;

			fn from_stored<'a>(stored: &'a dyn ::std::any::Any) -> ::core::option::Option<Self::Value<'a>> {
				stored.downcast_ref::<<$value as ::std::borrow::ToOwned>::Owned>().map(::std::borrow::Borrow::borrow)
			}

			unsafe fn read_erased(ptr: *const u8) -> ::std::boxed::Box<dyn $crate::list::AnyAttributeValue> {
				::std::boxed::Box::new(unsafe { ptr.cast::<&$value>().read() }.to_owned())
			}

			const REPARK: ::core::option::Option<$crate::list::ReparkFn> = {
				unsafe fn repark(value: &dyn $crate::list::AnyAttributeValue, dst: *mut u8, arena: &$crate::arena::Arena) -> ::core::option::Option<()> {
					let owned: &<$value as ::std::borrow::ToOwned>::Owned = value.as_any().downcast_ref().expect("a reference row replays its owned clone");
					let (parked, _) = arena.alloc(<$value as ::std::borrow::ToOwned>::to_owned(::std::borrow::Borrow::borrow(owned)))?;
					unsafe { dst.cast::<&$value>().write(::std::borrow::Borrow::borrow(parked)) };
					::core::option::Option::Some(())
				}
				::core::option::Option::Some(repark)
			};
		}

		$crate::named_value!($($rest)*);
	};
}

named_value! {
	for f64;
	for u32;
	for u64;
	for bool;
	for DVec2;
	for DAffine2;
	for crate::Color;
	for crate::blending::BlendMode;
	/// A document node path, the value type of `editor:layer_path`.
	pub NodeIdPath: &[crate::uuid::NodeId];
	/// Free text, parked in the arena by the writing kernel.
	pub Text: &str;
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
	///
	/// `dst` must address a live field of *this row's* value type: the glue
	/// writes its own `Value`'s worth of bytes there, so a caller resolving the
	/// field by [`name`](Self::name) checks [`size`](Self::size) and
	/// [`value_type`](Self::value_type) against the field first. The fields
	/// here are public and the struct is `Copy`, so a row is a claim about a
	/// marker rather than a proof about a field.
	pub write_stored: unsafe fn(&dyn AnyAttributeValue, *mut u8, &crate::arena::Arena) -> Option<()>,
}

/// The default's object representation, staged through zeroed storage so a
/// value with padding or an unused payload (`Option<f64>`'s `None`) hands the
/// caller deterministic bytes rather than whatever the stack held.
fn write_default_bytes<A: Attribute>(out: &mut [u8]) {
	assert_eq!(out.len(), size_of::<A::Value<'static>>());
	let mut staged = std::mem::MaybeUninit::<A::Value<'static>>::zeroed();
	staged.write(A::default());
	// SAFETY: the staging holds a live value of the type `out` is sized for.
	unsafe { std::ptr::copy_nonoverlapping(staged.as_ptr().cast::<u8>(), out.as_mut_ptr(), size_of::<A::Value<'static>>()) };
}

fn field_write_at<A: Attribute>(level: u8) -> crate::record::FieldWrite
where
	A::Value<'static>: graphene_hash::CacheHash + PartialEq,
{
	crate::record::FieldWrite::of::<A>(level)
}

/// # Safety
/// `dst` must address a live field of `A`'s value type.
unsafe fn write_stored<A: Attribute>(stored: &dyn AnyAttributeValue, dst: *mut u8, arena: &crate::arena::Arena) -> Option<()> {
	if A::from_stored(stored.as_any()).is_none() {
		// A wrong-typed stored value reads as absent, so the field keeps its default.
		return Some(());
	}
	match A::REPARK {
		// SAFETY: the caller's contract; the glue is this marker's own.
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

		// SAFETY: the reference-valued arms emit `REPARK`, the plain arm's value
		// type cannot name `'e`, and `read_erased` and `from_stored` are emitted
		// as each other's inverse.
		unsafe impl $crate::attribute::Attribute for $marker {
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

		// SAFETY: the reference-valued arms emit `REPARK`, the plain arm's value
		// type cannot name `'e`, and `read_erased` and `from_stored` are emitted
		// as each other's inverse.
		unsafe impl $crate::attribute::Attribute for $marker {
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

		// SAFETY: the reference-valued arms emit `REPARK`, the plain arm's value
		// type cannot name `'e`, and `read_erased` and `from_stored` are emitted
		// as each other's inverse.
		unsafe impl $crate::attribute::Attribute for $marker {
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
	/// position its drag cage.
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
	fn an_owned_reference_crossing_parks_into_the_serving_arena() {
		let owned = OwnedAttr::<Name>::new("crossing");
		let arena = crate::arena::Arena::new(1024).unwrap();
		assert_eq!(owned.park(&arena).unwrap(), "crossing");
		assert_eq!(owned.clone().park(&arena).unwrap(), "crossing", "the crossing parks again on every evaluation");
	}

	#[test]
	fn an_owned_plain_crossing_rides_its_bytes() {
		let arena = crate::arena::Arena::new(64).unwrap();
		assert_eq!(OwnedAttr::<Opacity>::new(0.25).park(&arena).unwrap(), 0.25);
	}

	#[test]
	fn an_exhausted_arena_refuses_an_owned_reference_crossing() {
		let owned = OwnedAttr::<Name>::new("too long for this arena");
		let arena = crate::arena::Arena::new(8).unwrap();
		assert!(owned.park(&arena).is_none());
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
		// SAFETY: `bool` borrows nothing, so the plain-value arm is the right one.
		unsafe impl Attribute for Conflict {
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
