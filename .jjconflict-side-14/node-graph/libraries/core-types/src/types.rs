use std::any::TypeId;
pub use std::borrow::Cow;
use std::fmt::{Display, Formatter};

#[macro_export]
macro_rules! concrete {
	($type:ty) => {
		$crate::Type::Concrete($crate::descriptor!($type))
	};
	($type:ty, $name:ty) => {
		$crate::Type::Concrete($crate::descriptor!($type, $name))
	};
}

#[macro_export]
macro_rules! descriptor {
	($type:ty) => {
		$crate::TypeDescriptor {
			id: Some(std::any::TypeId::of::<$type>()),
			name: $crate::Cow::Borrowed(std::any::type_name::<$type>()),
			alias: None,
			size: std::mem::size_of::<$type>(),
			align: std::mem::align_of::<$type>(),
		}
	};
	($type:ty, $name:ty) => {
		$crate::TypeDescriptor {
			id: Some(std::any::TypeId::of::<$type>()),
			name: $crate::Cow::Borrowed(std::any::type_name::<$type>()),
			alias: Some($crate::Cow::Borrowed(stringify!($name))),
			size: std::mem::size_of::<$type>(),
			align: std::mem::align_of::<$type>(),
		}
	};
}

#[macro_export]
macro_rules! concrete_with_name {
	($type:ty, $name:expr_2021) => {
		$crate::Type::Concrete($crate::TypeDescriptor {
			id: Some(std::any::TypeId::of::<$type>()),
			name: $crate::Cow::Borrowed($name),
			alias: None,
			size: std::mem::size_of::<$type>(),
			align: std::mem::align_of::<$type>(),
		})
	};
}

#[macro_export]
macro_rules! generic {
	($type:ty) => {{ $crate::Type::Generic($crate::Cow::Borrowed(stringify!($type))) }};
}

/// Constructs the [`Type`] of an `Item` holding the given element type, e.g. `item!(f64)` is the type of an `Item<f64>`.
/// The two-argument form tags the element descriptor with an alias, preserving the source spelling for widget dispatch.
#[macro_export]
macro_rules! item {
	(Item<$inner:ty>) => {
		$crate::Type::Item(Box::new($crate::item!($inner)))
	};
	($element:ty) => {
		$crate::Type::Item(Box::new($crate::concrete!($element)))
	};
	($element:ty, $alias:ty) => {
		$crate::Type::Item(Box::new($crate::concrete!($element, $alias)))
	};
}

/// Constructs the [`Type`] of a `List` holding the given element type, e.g. `list!(f64)` is the type of a `List<f64>`.
#[macro_export]
macro_rules! list {
	(List<$inner:ty>) => {
		$crate::Type::List(Box::new($crate::list!($inner)))
	};
	($element:ty) => {
		$crate::Type::List(Box::new($crate::concrete!($element)))
	};
}

// The `List<...>`/`Item<...>` rules must appear before the generic `$type:ty` rules, and in each macro that sees the literal tokens,
// because a type captured as `ty` becomes opaque to any inner macro's ranked pattern
#[macro_export]
macro_rules! future {
	(List<$inner:ty>) => {
		$crate::Type::Future(Box::new($crate::list!($inner)))
	};
	(List<$inner:ty>, $name:ty) => {
		$crate::Type::Future(Box::new($crate::list!($inner)))
	};
	(Item<$inner:ty>) => {
		$crate::Type::Future(Box::new($crate::item!($inner)))
	};
	(Item<$inner:ty>, $name:ty) => {
		$crate::Type::Future(Box::new($crate::item!($inner, $name)))
	};
	($type:ty) => {{ $crate::Type::Future(Box::new(concrete!($type))) }};
	($type:ty, $name:ty) => {
		$crate::Type::Future(Box::new(concrete!($type, $name)))
	};
}

#[macro_export]
macro_rules! fn_type {
	(List<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new($crate::list!($inner)))
	};
	(Item<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new($crate::item!($inner)))
	};
	($type:ty) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new(concrete!($type)))
	};
	($in_type:ty, List<$inner:ty>, alias: $outname:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::list!($inner)))
	};
	($in_type:ty, List<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::list!($inner)))
	};
	($in_type:ty, Item<$inner:ty>, alias: $outname:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::item!($inner, $inner)))
	};
	($in_type:ty, Item<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::item!($inner)))
	};
	($in_type:ty, $type:ty, alias: $outname:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new(concrete!($type, $outname)))
	};
	($in_type:ty, $type:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new(concrete!($type)))
	};
}
#[macro_export]
macro_rules! fn_type_fut {
	(List<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new($crate::Type::Future(Box::new($crate::list!($inner)))))
	};
	(Item<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new($crate::Type::Future(Box::new($crate::item!($inner)))))
	};
	($type:ty) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new(future!($type)))
	};
	($in_type:ty, List<$inner:ty>, alias: $outname:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::Type::Future(Box::new($crate::list!($inner)))))
	};
	($in_type:ty, List<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::Type::Future(Box::new($crate::list!($inner)))))
	};
	($in_type:ty, Item<$inner:ty>, alias: $outname:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::Type::Future(Box::new($crate::item!($inner, $inner)))))
	};
	($in_type:ty, Item<$inner:ty>) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new($crate::Type::Future(Box::new($crate::item!($inner)))))
	};
	($in_type:ty, $type:ty, alias: $outname:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new(future!($type, $outname)))
	};
	($in_type:ty, $type:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new(future!($type)))
	};
}

// TODO: Rename to NodeSignatureMonomorphization
#[derive(Clone, PartialEq, Eq, Hash, graphene_hash::CacheHash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeIOTypes {
	pub call_argument: Type,
	pub return_value: Type,
	pub inputs: Vec<Type>,
}

impl NodeIOTypes {
	pub const fn new(call_argument: Type, return_value: Type, inputs: Vec<Type>) -> Self {
		Self { call_argument, return_value, inputs }
	}

	/// Applies [`Type::normalize_rank`] to every type in the signature.
	pub fn normalize_rank(self) -> Self {
		Self {
			call_argument: self.call_argument.normalize_rank(),
			return_value: self.return_value.normalize_rank(),
			inputs: self.inputs.into_iter().map(Type::normalize_rank).collect(),
		}
	}

	pub const fn empty() -> Self {
		let tds1 = TypeDescriptor {
			id: None,
			name: Cow::Borrowed("()"),
			alias: None,
			size: 0,
			align: 0,
		};
		let tds2 = TypeDescriptor {
			id: None,
			name: Cow::Borrowed("()"),
			alias: None,
			size: 0,
			align: 0,
		};
		Self {
			call_argument: Type::Concrete(tds1),
			return_value: Type::Concrete(tds2),
			inputs: Vec::new(),
		}
	}

	pub fn ty(&self) -> Type {
		Type::Fn(Box::new(self.call_argument.clone()), Box::new(self.return_value.clone()))
	}
}

impl std::fmt::Debug for NodeIOTypes {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let inputs = self.inputs.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
		let return_value = &self.return_value;
		let call_argument = &self.call_argument;
		f.write_fmt(format_args!("({inputs}) → {return_value} called with {call_argument}"))
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, Eq, Hash, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProtoNodeIdentifier {
	name: Cow<'static, str>,
}

impl ProtoNodeIdentifier {
	pub const fn new(name: &'static str) -> Self {
		ProtoNodeIdentifier { name: Cow::Borrowed(name) }
	}

	pub const fn with_owned_string(name: String) -> Self {
		ProtoNodeIdentifier { name: Cow::Owned(name) }
	}

	pub fn as_str(&self) -> &str {
		self.name.as_ref()
	}

	pub const fn as_static_str(&self) -> &'static str {
		match self.name {
			Cow::Borrowed(name) => name,
			Cow::Owned(_) => panic!("`as_static_str` called on a `ProtoNodeIdentifier` backed by an owned string"),
		}
	}
}

impl From<ProtoNodeIdentifier> for Cow<'static, str> {
	fn from(val: ProtoNodeIdentifier) -> Self {
		val.name
	}
}

impl Display for ProtoNodeIdentifier {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("ProtoNodeIdentifier").field(&self.name).finish()
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeDescriptor {
	#[cfg_attr(feature = "serde", serde(skip))]
	pub id: Option<TypeId>,
	pub name: Cow<'static, str>,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
	pub alias: Option<Cow<'static, str>>,
	#[cfg_attr(feature = "serde", serde(skip))]
	pub size: usize,
	#[cfg_attr(feature = "serde", serde(skip))]
	pub align: usize,
}

impl std::hash::Hash for TypeDescriptor {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.name.hash(state);
	}
}

impl graphene_hash::CacheHash for TypeDescriptor {
	fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
		graphene_hash::CacheHash::cache_hash(&self.name, state);
	}
}

impl std::fmt::Display for TypeDescriptor {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let text = make_type_user_readable(&simplify_identifier_name(&self.name));
		write!(f, "{text}")
	}
}

impl PartialEq for TypeDescriptor {
	fn eq(&self, other: &Self) -> bool {
		match (self.id, other.id) {
			(Some(id), Some(other_id)) => id == other_id,
			_ => {
				// TODO: Add a flag to disable this warning
				// warn!("TypeDescriptor::eq: comparing types without ids based on name");
				self.name == other.name
			}
		}
	}
}

/// Graph runtime type information used for type inference.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, PartialEq, Eq, Hash, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
	/// A wrapper for some type variable used within the inference system. Resolved at inference time and replaced with a concrete type.
	Generic(Cow<'static, str>),
	/// A wrapper around the Rust type id for any concrete Rust type. Allows us to do equality comparisons, like checking if a String == a String.
	Concrete(TypeDescriptor),
	/// Runtime type information for a function. Given some input, gives some output.
	Fn(Box<Type>, Box<Type>),
	/// Represents a future which promises to return the inner type.
	Future(Box<Type>),
	/// Represents a recursive [Type] allowing nested levels of types to represent the type of an Item<T>.
	Item(Box<Type>),
	/// Represents a list of this recursive [Type] allowing nested levels of types to represent the type of a List<T>.
	List(Box<Type>),
}

impl Default for Type {
	fn default() -> Self {
		concrete!(())
	}
}

unsafe impl dyn_any::StaticType for Type {
	type Static = Self;
}

impl Type {
	pub fn is_generic(&self) -> bool {
		matches!(self, Type::Generic(_))
	}

	pub fn is_concrete(&self) -> bool {
		matches!(self, Type::Concrete(_))
	}

	pub fn is_fn(&self) -> bool {
		matches!(self, Type::Fn(_, _))
	}

	pub fn is_value(&self) -> bool {
		matches!(self, Type::Fn(_, _) | Type::Concrete(_))
	}

	pub fn is_unit(&self) -> bool {
		matches!(self, Type::Fn(_, _) | Type::Concrete(_))
	}

	pub fn is_generic_or_fn(&self) -> bool {
		matches!(self, Type::Fn(_, _) | Type::Generic(_))
	}

	pub fn fn_input(&self) -> Option<&Type> {
		match self {
			Type::Fn(first, _) => Some(first),
			_ => None,
		}
	}

	pub fn fn_output(&self) -> Option<&Type> {
		match self {
			Type::Fn(_, second) => Some(second),
			_ => None,
		}
	}

	pub fn function(input: &Type, output: &Type) -> Type {
		Type::Fn(Box::new(input.clone()), Box::new(output.clone()))
	}
}

impl Type {
	pub fn new<T: dyn_any::StaticType + Sized>() -> Self {
		Self::Concrete(TypeDescriptor {
			id: Some(TypeId::of::<T::Static>()),
			name: Cow::Borrowed(std::any::type_name::<T::Static>()),
			alias: None,
			size: size_of::<T>(),
			align: align_of::<T>(),
		})
	}

	pub fn size(&self) -> Option<usize> {
		match self {
			Self::Generic(_) => None,
			Self::Concrete(ty) => Some(ty.size),
			Self::Fn(_, _) => None,
			Self::Future(_) => None,
			Self::Item(_) => None,
			Self::List(_) => None,
		}
	}

	pub fn align(&self) -> Option<usize> {
		match self {
			Self::Generic(_) => None,
			Self::Concrete(ty) => Some(ty.align),
			Self::Fn(_, _) => None,
			Self::Future(_) => None,
			Self::Item(_) => None,
			Self::List(_) => None,
		}
	}

	pub fn nested_type(&self) -> &Type {
		match self {
			Self::Generic(_) => self,
			Self::Concrete(_) => self,
			Self::Fn(_, output) => output.nested_type(),
			Self::Future(output) => output.nested_type(),
			Self::Item(_) => self,
			Self::List(_) => self,
		}
	}

	pub fn replace_nested(&mut self, f: impl Fn(&Type) -> Option<Type>) -> Option<Type> {
		if let Some(replacement) = f(self) {
			return Some(std::mem::replace(self, replacement));
		}
		match self {
			Self::Generic(_) => None,
			Self::Concrete(_) => None,
			Self::Fn(_, output) => output.replace_nested(f),
			Self::Future(output) => output.replace_nested(f),
			Self::Item(_) => None,
			Self::List(_) => None,
		}
	}

	pub fn identifier_name(&self) -> String {
		match self {
			Type::Generic(name) => name.to_string(),
			Type::Concrete(ty) => simplify_identifier_name(&ty.name),
			Type::Fn(call_arg, return_value) => format!("{} called with {}", return_value.identifier_name(), call_arg.identifier_name()),
			Type::Future(ty) => ty.identifier_name(),
			Type::Item(element) => element.identifier_name(),
			Type::List(element) => format!("{}[]", element.identifier_name()),
		}
	}

	/// Constructs the [`Type`] of a `List` holding elements of the given type, the expression-position counterpart of [`list!`].
	pub fn list_of(element: Type) -> Type {
		Type::List(Box::new(element))
	}

	/// The element type if this is a rank-1 `List` wire type.
	pub fn list_element(&self) -> Option<&Type> {
		match self {
			Type::List(element) => Some(element),
			_ => None,
		}
	}

	/// The element name if this is the concrete type of a rank-0 `Item` cell, e.g. `f64` from `Item<f64>`.
	pub fn item_element_name(&self) -> Option<&str> {
		let Type::Concrete(descriptor) = self else { return None };
		descriptor.name.strip_prefix("core_types::list::Item<")?.strip_suffix('>')
	}

	/// The element name if this is the type of an `Item<Bundle<X>>` cell carrying a whole list, e.g. `f64` from `Item<Bundle<f64>>`.
	/// The `Bundle` layer stays name-encoded inside the structural `Item` since it has no structural variant.
	pub fn bundle_element_name(&self) -> Option<&str> {
		let Type::Item(element) = self else { return None };
		let Type::Concrete(descriptor) = element.as_ref() else { return None };
		descriptor.name.strip_prefix("core_types::list::Bundle<")?.strip_suffix('>')
	}

	/// Converts a name-encoded `List` or `Item` concrete type into its structural form, recursively.
	/// Structurally-built types pass through unchanged, so sources which cannot construct ranked types
	/// (reflection and opaque macro captures) converge with macro-built ones at this single point.
	pub fn normalize_rank(self) -> Type {
		fn parse_element(element_name: &str) -> Type {
			let element = Type::Concrete(TypeDescriptor {
				id: None,
				name: Cow::Owned(element_name.to_string()),
				alias: None,
				size: 0,
				align: 0,
			});
			element.normalize_rank()
		}

		match self {
			Type::Concrete(descriptor) => {
				if let Some(element_name) = descriptor.name.strip_prefix("core_types::list::List<").and_then(|rest| rest.strip_suffix('>')) {
					return Type::List(Box::new(parse_element(element_name)));
				}
				if let Some(element_name) = descriptor.name.strip_prefix("core_types::list::Item<").and_then(|rest| rest.strip_suffix('>')) {
					return Type::Item(Box::new(parse_element(element_name)));
				}
				Type::Concrete(descriptor)
			}
			Type::Fn(input, output) => Type::Fn(Box::new(input.normalize_rank()), Box::new(output.normalize_rank())),
			Type::Future(inner) => Type::Future(Box::new(inner.normalize_rank())),
			Type::Item(element) => Type::Item(Box::new(element.normalize_rank())),
			Type::List(element) => Type::List(Box::new(element.normalize_rank())),
			Type::Generic(_) => self,
		}
	}
}

pub fn simplify_identifier_name(ty: &str) -> String {
	ty.split('<')
		.map(|path| path.split(',').map(|path| path.split("::").last().unwrap_or(path)).collect::<Vec<_>>().join(","))
		.collect::<Vec<_>>()
		.join("<")
}

/// Converts a Rust-internal type name to its user-facing form.
pub fn make_type_user_readable(ty: &str) -> String {
	let ty = ty
		.replace("Option<Arc<OwnedContextImpl>>", "Context")
		.replace("Raster<CPU>", "Raster")
		.replace("Raster<GPU>", "Raster")
		.replace("DAffine2", "Transform")
		.replace("Affine2", "Transform")
		.replace("DVec2", "Vec2")
		.replace("IVec2", "Vec2")
		.replace("UVec2", "Vec2")
		.replace("&str", "String");

	rewrite_ranked_type_wrappers(&ty)
}

/// Rewrites `List<T>` and the whole-collection `Bundle<T>` as `T[]`, and unwraps `Item<T>` to `T`, so ranked wires read as their element type.
/// Handles nesting (e.g. `List<List<Vector>>` becomes `Vector[][]`).
/// Respects word boundaries so unrelated identifiers that happen to end in `List` or `Item` are not affected.
fn rewrite_ranked_type_wrappers(input: &str) -> String {
	let bytes = input.as_bytes();
	let mut result = String::with_capacity(input.len());
	let mut i = 0;

	while i < bytes.len() {
		let at_word_boundary = i == 0 || !is_identifier_byte(bytes[i - 1]);
		if at_word_boundary && bytes[i..].starts_with(b"List<") {
			let inner_start = i + b"List<".len();
			if let Some(close) = find_matching_angle_bracket(bytes, inner_start) {
				let inner = &input[inner_start..close];
				result.push_str(&rewrite_ranked_type_wrappers(inner));
				result.push_str("[]");
				i = close + 1;
				continue;
			}
		}
		if at_word_boundary && bytes[i..].starts_with(b"Bundle<") {
			let inner_start = i + b"Bundle<".len();
			if let Some(close) = find_matching_angle_bracket(bytes, inner_start) {
				let inner = &input[inner_start..close];
				result.push_str(&rewrite_ranked_type_wrappers(inner));
				result.push_str("[]");
				i = close + 1;
				continue;
			}
		}
		if at_word_boundary && bytes[i..].starts_with(b"Item<") {
			let inner_start = i + b"Item<".len();
			if let Some(close) = find_matching_angle_bracket(bytes, inner_start) {
				let inner = &input[inner_start..close];
				result.push_str(&rewrite_ranked_type_wrappers(inner));
				i = close + 1;
				continue;
			}
		}
		if bytes[i].is_ascii() {
			result.push(bytes[i] as char);
			i += 1;
		} else {
			let ch = input[i..].chars().next().unwrap();
			result.push(ch);
			i += ch.len_utf8();
		}
	}

	result
}

fn is_identifier_byte(byte: u8) -> bool {
	byte.is_ascii_alphanumeric() || byte == b'_'
}

fn find_matching_angle_bracket(bytes: &[u8], start: usize) -> Option<usize> {
	let mut depth = 1_usize;
	for (offset, &byte) in bytes[start..].iter().enumerate() {
		match byte {
			b'<' => depth += 1,
			b'>' => {
				depth -= 1;
				if depth == 0 {
					return Some(start + offset);
				}
			}
			_ => {}
		}
	}
	None
}

impl std::fmt::Debug for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{self}")
	}
}

// Display
impl std::fmt::Display for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Type::Generic(name) => write!(f, "{}", make_type_user_readable(name)),
			Type::Concrete(ty) => write!(f, "{ty}"),
			Type::Fn(_, return_value) => write!(f, "{return_value}"),
			Type::Future(ty) => write!(f, "{ty}"),
			Type::Item(element) => write!(f, "{element}"),
			Type::List(element) => write!(f, "{element}[]"),
		}
	}
}
