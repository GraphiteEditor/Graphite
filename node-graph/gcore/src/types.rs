use std::any::TypeId;

pub use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[macro_export]
macro_rules! concrete {
	($type:ty) => {
		$crate::Type::Concrete($crate::TypeDescriptor {
			id: Some(std::any::TypeId::of::<$type>()),
			name: $crate::Cow::Borrowed(std::any::type_name::<$type>()),
			alias: None,
			size: std::mem::size_of::<$type>(),
			align: std::mem::align_of::<$type>(),
		})
	};
	($type:ty, $name:ty) => {
		$crate::Type::Concrete($crate::TypeDescriptor {
			id: Some(std::any::TypeId::of::<$type>()),
			name: $crate::Cow::Borrowed(std::any::type_name::<$type>()),
			alias: Some($crate::Cow::Borrowed(stringify!($name))),
			size: std::mem::size_of::<$type>(),
			align: std::mem::align_of::<$type>(),
		})
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

#[macro_export]
macro_rules! future {
	($type:ty) => {{ $crate::Type::Future(Box::new(concrete!($type))) }};
	($type:ty, $name:ty) => {
		$crate::Type::Future(Box::new(concrete!($type, $name)))
	};
}

#[macro_export]
macro_rules! fn_type {
	($type:ty) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new(concrete!($type)))
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
	($type:ty) => {
		$crate::Type::Fn(Box::new(concrete!(())), Box::new(future!($type)))
	};
	($in_type:ty, $type:ty, alias: $outname:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new(future!($type, $outname)))
	};
	($in_type:ty, $type:ty) => {
		$crate::Type::Fn(Box::new(concrete!($in_type)), Box::new(future!($type)))
	};
}

#[derive(Clone, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeIOTypes {
	pub call_argument: Type,
	pub return_value: Type,
	pub inputs: Vec<Type>,
}

impl NodeIOTypes {
	pub const fn new(call_argument: Type, return_value: Type, inputs: Vec<Type>) -> Self {
		Self { call_argument, return_value, inputs }
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
		f.write_fmt(format_args!(
			"node({}) → {}",
			[&self.call_argument].into_iter().chain(&self.inputs).map(|input| input.to_string()).collect::<Vec<_>>().join(", "),
			self.return_value
		))
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, specta::Type, serde::Serialize, serde::Deserialize)]
pub struct ProtoNodeIdentifier {
	pub name: Cow<'static, str>,
}

impl From<String> for ProtoNodeIdentifier {
	fn from(value: String) -> Self {
		Self { name: Cow::Owned(value) }
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

	pub const fn with_owned_string(name: String) -> Self {
		ProtoNodeIdentifier { name: Cow::Owned(name) }
	}
}

impl Deref for ProtoNodeIdentifier {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.name.as_ref()
	}
}

impl Display for ProtoNodeIdentifier {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("ProtoNodeIdentifier").field(&self.name).finish()
	}
}

fn migrate_type_descriptor_names<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Cow<'static, str>, D::Error> {
	use serde::Deserialize;

	let name = String::deserialize(deserializer)?;
	let name = match name.as_str() {
		"f32" => "f64".to_string(),
		"graphene_core::transform::Footprint" => "std::option::Option<std::sync::Arc<graphene_core::context::OwnedContextImpl>>".to_string(),
		"graphene_core::graphic_element::GraphicGroup" => "graphene_core::table::Table<graphene_core::graphic::Graphic>".to_string(),
		"graphene_core::raster::image::ImageFrame<Color>"
		| "graphene_core::raster::image::ImageFrame<graphene_core::raster::color::Color>"
		| "graphene_core::instances::Instances<graphene_core::raster::image::ImageFrame<Color>>"
		| "graphene_core::instances::Instances<graphene_core::raster::image::ImageFrame<graphene_core::raster::color::Color>>"
		| "graphene_core::instances::Instances<graphene_core::raster::image::Image<graphene_core::raster::color::Color>>" => {
			"graphene_core::table::Table<graphene_core::raster::image::Image<graphene_core::raster::color::Color>>".to_string()
		}
		"graphene_core::vector::vector_data::VectorData"
		| "graphene_core::instances::Instances<graphene_core::vector::vector_data::VectorData>"
		| "graphene_core::table::Table<graphene_core::vector::vector_data::VectorData>"
		| "graphene_core::table::Table<graphene_core::vector::vector_data::Vector>" => "graphene_core::table::Table<graphene_core::vector::vector_types::Vector>".to_string(),
		"graphene_core::instances::Instances<graphene_core::graphic_element::Artboard>" => "graphene_core::table::Table<graphene_core::artboard::Artboard>".to_string(),
		"graphene_core::vector::vector_data::modification::VectorModification" => "graphene_core::vector::vector_modification::VectorModification".to_string(),
		"graphene_core::table::Table<graphene_core::graphic_element::Graphic>" => "graphene_core::table::Table<graphene_core::graphic::Graphic>".to_string(),
		_ => name,
	};

	Ok(Cow::Owned(name))
}

#[derive(Clone, Debug, Eq, specta::Type, serde::Serialize, serde::Deserialize)]
pub struct TypeDescriptor {
	#[serde(skip)]
	#[specta(skip)]
	pub id: Option<TypeId>,
	#[serde(deserialize_with = "migrate_type_descriptor_names")]
	pub name: Cow<'static, str>,
	#[serde(default)]
	pub alias: Option<Cow<'static, str>>,
	#[serde(skip)]
	pub size: usize,
	#[serde(skip)]
	pub align: usize,
}

impl std::hash::Hash for TypeDescriptor {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.name.hash(state);
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
#[derive(Clone, PartialEq, Eq, Hash, specta::Type, serde::Serialize, serde::Deserialize)]
pub enum Type {
	/// A wrapper for some type variable used within the inference system. Resolved at inference time and replaced with a concrete type.
	Generic(Cow<'static, str>),
	/// A wrapper around the Rust type id for any concrete Rust type. Allows us to do equality comparisons, like checking if a String == a String.
	Concrete(TypeDescriptor),
	/// Runtime type information for a function. Given some input, gives some output.
	Fn(Box<Type>, Box<Type>),
	/// Represents a future which promises to return the inner type.
	Future(Box<Type>),
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

	pub fn nested_type(&self) -> &Type {
		match self {
			Self::Generic(_) => self,
			Self::Concrete(_) => self,
			Self::Fn(_, output) => output.nested_type(),
			Self::Future(output) => output.nested_type(),
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
		}
	}

	pub fn to_cow_string(&self) -> Cow<'static, str> {
		match self {
			Type::Generic(name) => name.clone(),
			_ => Cow::Owned(self.to_string()),
		}
	}
}

fn format_type(ty: &str) -> String {
	ty.split('<')
		.map(|path| path.split(',').map(|path| path.split("::").last().unwrap_or(path)).collect::<Vec<_>>().join(","))
		.collect::<Vec<_>>()
		.join("<")
}

impl std::fmt::Debug for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let result = match self {
			Self::Generic(name) => name.to_string(),
			#[cfg(feature = "type_id_logging")]
			Self::Concrete(ty) => format!("Concrete<{}, {:?}>", ty.name, ty.id),
			#[cfg(not(feature = "type_id_logging"))]
			Self::Concrete(ty) => format_type(&ty.name),
			Self::Fn(call_arg, return_value) => format!("{return_value:?} called with {call_arg:?}"),
			Self::Future(ty) => format!("{ty:?}"),
		};
		let result = result.replace("Option<Arc<OwnedContextImpl>>", "Context");
		write!(f, "{result}")
	}
}

impl std::fmt::Display for Type {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let result = match self {
			Type::Generic(name) => name.to_string(),
			Type::Concrete(ty) => format_type(&ty.name),
			Type::Fn(call_arg, return_value) => format!("{return_value} called with {call_arg}"),
			Type::Future(ty) => ty.to_string(),
		};
		let result = result.replace("Option<Arc<OwnedContextImpl>>", "Context");
		write!(f, "{result}")
	}
}
