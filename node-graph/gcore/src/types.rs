use core::any::TypeId;

#[cfg(not(feature = "std"))]
pub use alloc::borrow::Cow;
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
}

#[macro_export]
macro_rules! concrete {
	($type:ty) => {
		Type::Concrete(TypeDescriptor {
			id: Some(core::any::TypeId::of::<$type>()),
			name: Cow::Borrowed(core::any::type_name::<$type>()),
		})
	};
}
#[macro_export]
macro_rules! generic {
	($type:ty) => {{
		Type::Generic(Cow::Borrowed(stringify!($type)))
	}};
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
}

impl core::hash::Hash for TypeDescriptor {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
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
}

impl core::fmt::Debug for Type {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Generic(arg0) => f.write_fmt(format_args!("Generic({})", arg0)),
			#[cfg(feature = "type_id_logging")]
			Self::Concrete(arg0) => f.write_fmt(format_args!("Concrete({}, {:?}))", arg0.name, arg0.id)),
			#[cfg(not(feature = "type_id_logging"))]
			Self::Concrete(arg0) => f.write_fmt(format_args!("Concrete({})", arg0.name)),
		}
	}
}

impl std::fmt::Display for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Type::Generic(name) => write!(f, "{}", name),
			Type::Concrete(ty) => write!(f, "{}", ty.name),
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
