use core::any::TypeId;

#[cfg(not(feature = "std"))]
pub use alloc::borrow::Cow;
use dyn_any::StaticType;
#[cfg(feature = "std")]
pub use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[macro_export]
macro_rules! concrete {
	($type:ty) => {
		Type::Concrete(TypeDescriptor {
			id: Some(core::any::TypeId::of::<$type>()),
			name: Cow::Borrowed(core::any::type_name::<$type>()),
			size: core::mem::size_of::<$type>(),
			align: core::mem::align_of::<$type>(),
		})
	};
}

#[macro_export]
macro_rules! concrete_with_name {
	($type:ty, $name:expr) => {
		Type::Concrete(TypeDescriptor {
			id: Some(core::any::TypeId::of::<$type>()),
			name: Cow::Borrowed($name),
			size: core::mem::size_of::<$type>(),
			align: core::mem::align_of::<$type>(),
		})
	};
}

#[macro_export]
macro_rules! generic {
	($type:ty) => {{
		Type::Generic(Cow::Borrowed(stringify!($type)))
	}};
}

#[macro_export]
macro_rules! fn_type {
	($input:ty, $output:ty) => {
		Type::Fn(Box::new(concrete!($input)), Box::new(concrete!($output)))
	};
}

#[macro_export]
macro_rules! value_fn {
	($output:ty) => {
		Type::Fn(Box::new(concrete!(())), Box::new(concrete!($output)))
	};
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeIdentifier {
	pub name: Cow<'static, str>,
}

#[derive(Clone, Debug, Eq, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeDescriptor {
	#[cfg_attr(feature = "serde", serde(skip))]
	#[specta(skip)]
	pub id: Option<TypeId>,
	pub name: Cow<'static, str>,
	pub size: usize,
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
				warn!("TypeDescriptor::eq: comparing types without ids based on name");
				self.name == other.name
			}
		}
	}
}

#[derive(Clone, PartialEq, Eq, Hash, specta::Type)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
	Generic(Cow<'static, str>),
	Concrete(TypeDescriptor),
	Fn(Box<Type>, Box<Type>),
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
		}
	}

	pub fn align(&self) -> Option<usize> {
		match self {
			Self::Generic(_) => None,
			Self::Concrete(ty) => Some(ty.align),
			Self::Fn(_, _) => None,
		}
	}
}

impl core::fmt::Debug for Type {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Generic(arg0) => write!(f, "Generic({})", arg0),
			#[cfg(feature = "type_id_logging")]
			Self::Concrete(arg0) => write!(f, "Concrete({}, {:?})", arg0.name, arg0.id),
			#[cfg(not(feature = "type_id_logging"))]
			Self::Concrete(arg0) => write!(f, "Concrete({})", arg0.name),
			Self::Fn(arg0, arg1) => write!(f, "({:?} -> {:?})", arg0, arg1),
		}
	}
}

impl std::fmt::Display for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Type::Generic(name) => write!(f, "{}", name),
			Type::Concrete(ty) => write!(f, "{}", ty.name),
			Type::Fn(input, output) => write!(f, "({} -> {})", input, output),
		}
	}
}

impl From<&'static str> for NodeIdentifier {
	fn from(s: &'static str) -> Self {
		NodeIdentifier { name: Cow::Borrowed(s) }
	}
}

impl NodeIdentifier {
	pub const fn new(name: &'static str) -> Self {
		NodeIdentifier { name: Cow::Borrowed(name) }
	}
}
