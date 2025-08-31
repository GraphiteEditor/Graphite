#[macro_use]
extern crate log;

pub mod animation;
pub mod artboard;
pub mod blending_nodes;
pub mod bounds;
pub mod consts;
pub mod context;
pub mod context_modification;
pub mod debug;
pub mod extract_xy;
pub mod generic;
pub mod gradient;
pub mod graphic;
pub mod logic;
pub mod math;
pub mod memo;
pub mod misc;
pub mod ops;
pub mod raster;
pub mod raster_types;
pub mod registry;
pub mod render_complexity;
pub mod subpath;
pub mod table;
pub mod text;
pub mod transform;
pub mod transform_nodes;
pub mod uuid;
pub mod value;
pub mod vector;

pub use crate as graphene_core;
pub use artboard::Artboard;
pub use blending::*;
pub use color::Color;
pub use context::*;
pub use ctor;
pub use dyn_any::{StaticTypeSized, WasmNotSend, WasmNotSync};
pub use graphene_core_shaders::AsU32;
pub use graphene_core_shaders::blending;
pub use graphene_core_shaders::choice_type;
pub use graphene_core_shaders::color;
pub use graphene_core_shaders::shaders;
pub use graphic::Graphic;
pub use memo::MemoHash;
pub use num_traits;
use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;
pub use types::Cow;

// pub trait Node: for<'n> NodeIO<'n> {
/// The node trait allows for defining any node. Nodes can only take one call argument input, however they can store references to other nodes inside the struct.
/// See `node-graph/README.md` for information on how to define a new node.
pub trait Node<'i, Input> {
	type Output: 'i;
	/// Evaluates the node with the single specified input.
	fn eval(&'i self, input: Input) -> Self::Output;
	/// Resets the node, e.g. the LetNode's cache is set to None.
	fn reset(&self) {}
	/// Returns the name of the node for diagnostic purposes.
	fn node_name(&self) -> &'static str {
		std::any::type_name::<Self>()
	}
	/// Serialize the node which is used for the `introspect` function which can retrieve values from monitor nodes.
	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		log::warn!("Node::serialize not implemented for {}", std::any::type_name::<Self>());
		None
	}
}

mod types;
pub use types::*;

pub trait NodeIO<'i, Input>: Node<'i, Input>
where
	Self::Output: 'i + StaticTypeSized,
	Input: StaticTypeSized,
{
	fn input_type(&self) -> TypeId {
		TypeId::of::<Input::Static>()
	}
	fn input_type_name(&self) -> &'static str {
		std::any::type_name::<Input>()
	}
	fn output_type(&self) -> TypeId {
		TypeId::of::<<Self::Output as StaticTypeSized>::Static>()
	}
	fn output_type_name(&self) -> &'static str {
		std::any::type_name::<Self::Output>()
	}
	fn to_node_io(&self, inputs: Vec<Type>) -> NodeIOTypes {
		NodeIOTypes {
			call_argument: concrete!(<Input as StaticTypeSized>::Static),
			return_value: concrete!(<Self::Output as StaticTypeSized>::Static),
			inputs,
		}
	}
	fn to_async_node_io(&self, inputs: Vec<Type>) -> NodeIOTypes
	where
		<Self::Output as Future>::Output: StaticTypeSized,
		Self::Output: Future,
	{
		NodeIOTypes {
			call_argument: concrete!(<Input as StaticTypeSized>::Static),
			return_value: future!(<<Self::Output as Future>::Output as StaticTypeSized>::Static),
			inputs,
		}
	}
}

impl<'i, N: Node<'i, I>, I> NodeIO<'i, I> for N
where
	N::Output: 'i + StaticTypeSized,
	I: StaticTypeSized,
{
}

impl<'i, I: 'i, N: Node<'i, I> + ?Sized> Node<'i, I> for &'i N {
	type Output = N::Output;
	fn eval(&'i self, input: I) -> N::Output {
		(*self).eval(input)
	}
}
impl<'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for Box<N> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}
impl<'i, I: 'i, O: 'i, N: Node<'i, I, Output = O> + ?Sized> Node<'i, I> for std::sync::Arc<N> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}

impl<'i, I, O: 'i> Node<'i, I> for Pin<Box<dyn Node<'i, I, Output = O> + 'i>> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}
impl<'i, I, O: 'i> Node<'i, I> for Pin<&'i (dyn NodeIO<'i, I, Output = O> + 'i)> {
	type Output = O;
	fn eval(&'i self, input: I) -> O {
		(**self).eval(input)
	}
}

pub trait InputAccessorSource<'a, T>: InputAccessorSourceIdentifier + std::fmt::Debug {
	fn get_input(&'a self, index: usize) -> Option<&'a T>;
	fn set_input(&'a mut self, index: usize, value: T);
}

pub trait InputAccessorSourceIdentifier {
	fn has_identifier(&self, identifier: &str) -> bool;
}

pub trait InputAccessor<'n, Source: 'n>
where
	Self: Sized,
{
	fn new_with_source(source: &'n Source) -> Option<Self>;
}

pub trait NodeInputDecleration {
	const INDEX: usize;
	fn identifier() -> ProtoNodeIdentifier;
	type Result;
}
