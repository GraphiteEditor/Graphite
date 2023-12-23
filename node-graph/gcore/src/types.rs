use core::any::TypeId;

#[cfg(not(feature = "std"))]
pub use alloc::borrow::Cow;
use dyn_any::StaticType;
#[cfg(feature = "std")]
pub use std::borrow::Cow;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NodeIOTypes {
	pub input: Type,
	pub output: Type,
	pub parameters: Vec<Type>,
}

impl NodeIOTypes {
	pub fn new(input: Type, output: Type, parameters: Vec<Type>) -> Self {
		Self { input, output, parameters }
	}

	pub fn ty(&self) -> Type {
		Type::Fn(Box::new(self.input.clone()), Box::new(self.output.clone()))
	}
}

impl core::fmt::Debug for NodeIOTypes {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_fmt(format_args!(
			"node({}) -> {}",
			[&self.input].into_iter().chain(&self.parameters).map(|input| input.to_string()).collect::<Vec<_>>().join(", "),
			self.output
		))
	}
}

#[macro_export]
macro_rules! concrete {
	($type:ty) => {
		$crate::Type::Concrete($crate::TypeDescriptor {
			id: Some(core::any::TypeId::of::<$type>()),
			name: $crate::Cow::Borrowed(core::any::type_name::<$type>()),
			size: core::mem::size_of::<$type>(),
			align: core::mem::align_of::<$type>(),
		})
	};
}

#[macro_export]
macro_rules! concrete_with_name {
	($type:ty, $name:expr) => {
		$crate::Type::Concrete($crate::TypeDescriptor {
			id: Some(core::any::TypeId::of::<$type>()),
			name: $crate::Cow::Borrowed($name),
			size: core::mem::size_of::<$type>(),
			align: core::mem::align_of::<$type>(),
		})
	};
}

#[macro_export]
macro_rules! generic {
	($type:ty) => {{
		$crate::Type::Generic($crate::Cow::Borrowed(stringify!($type)))
	}};
}

#[macro_export]
macro_rules! fn_type {
	($type:ty) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new(concrete!($type)))
	};
	($in_type:ty, $type:ty) => {
		$crate::Type::Fn(Box::new(concrete!(($in_type))), Box::new(concrete!($type)))
	};
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProtoNodeIdentifier {
	pub name: Cow<'static, str>,
}

#[derive(Clone, Debug, Eq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeDescriptor {
	#[cfg_attr(feature = "serde", serde(skip))]
	#[specta(skip)]
	pub id: Option<TypeId>,
	pub name: Cow<'static, str>,
	#[serde(default)]
	pub size: usize,
	#[serde(default)]
	pub align: usize,
}

impl core::hash::Hash for TypeDescriptor {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.name.hash(state);
	}
}

impl PartialEq for TypeDescriptor {
	fn eq(&self, other: &Self) -> bool {
		match (self.id, other.id) {
			(Some(id), Some(other_id)) => id == other_id,
			_ => {
				// TODO: Add a flag to disable this warning
				//warn!("TypeDescriptor::eq: comparing types without ids based on name");
				self.name == other.name
			}
		}
	}
}

/// Graph runtime type information used for type inference.
#[derive(Clone, PartialEq, Eq, Hash, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
	/// A wrapper for some type variable used within the inference system. Resolved at inference time and replaced with a concrete type.
	Generic(Cow<'static, str>),
	/// A wrapper around the Rust type id for any concrete Rust type. Allows us to do equality comparisons, like checking if a String == a String.
	Concrete(TypeDescriptor),
	/// Runtime type information for a function. Given some input, gives some output.
	/// See the example and explanation in the `ComposeNode` implementation within the node registry for more info.
	Fn(Box<Type>, Box<Type>),
	/// Not used at the moment.
	Future(Box<Type>),
}

unsafe impl StaticType for Type {
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
	pub fn new<T: StaticType + Sized>() -> Self {
		Self::Concrete(TypeDescriptor {
			id: Some(TypeId::of::<T::Static>()),
			name: Cow::Borrowed(core::any::type_name::<T::Static>()),
			size: core::mem::size_of::<T>(),
			align: core::mem::align_of::<T>(),
		})
	}
	pub fn size(&self) -> Option<usize> {
		match self {
			Self::Generic(_) => None,
			Self::Concrete(ty) => Some(ty.size),
			Self::Fn(_, _) => None,
			Self::Future(_) => None,
		}
	}

	pub fn align(&self) -> Option<usize> {
		match self {
			Self::Generic(_) => None,
			Self::Concrete(ty) => Some(ty.align),
			Self::Fn(_, _) => None,
			Self::Future(_) => None,
		}
	}
}

fn format_type(ty: &str) -> String {
	ty.split('<')
		.map(|path| path.split(',').map(|path| path.split("::").last().unwrap_or(path)).collect::<Vec<_>>().join(","))
		.collect::<Vec<_>>()
		.join("<")
}

impl core::fmt::Debug for Type {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Generic(arg0) => write!(f, "Generic({arg0})"),
			#[cfg(feature = "type_id_logging")]
			Self::Concrete(arg0) => write!(f, "Concrete({}, {:?})", arg0.name, arg0.id),
			#[cfg(not(feature = "type_id_logging"))]
			Self::Concrete(arg0) => write!(f, "Concrete({})", format_type(&arg0.name)),
			Self::Fn(arg0, arg1) => write!(f, "({arg0:?} -> {arg1:?})"),
			Self::Future(arg0) => write!(f, "Future({arg0:?})"),
		}
	}
}

impl std::fmt::Display for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Type::Generic(name) => write!(f, "{name}"),
			Type::Concrete(ty) => write!(f, "{}", format_type(&ty.name)),
			Type::Fn(input, output) => write!(f, "({input} -> {output})"),
			Type::Future(ty) => write!(f, "Future<{ty}>"),
		}
	}
}

impl From<&'static str> for ProtoNodeIdentifier {
	fn from(s: &'static str) -> Self {
		ProtoNodeIdentifier { name: Cow::Borrowed(s) }
	}
}

impl ProtoNodeIdentifier {
	pub const fn new(name: &'static str) -> Self {
		ProtoNodeIdentifier { name: Cow::Borrowed(name) }
	}
}
