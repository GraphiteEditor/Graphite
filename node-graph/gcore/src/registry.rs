use crate::{Node, NodeIO, NodeIOTypes, Type, WasmNotSend};
use dyn_any::{DynAny, StaticType};
use std::borrow::Cow;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};

pub mod types {
	/// 0% - 100%
	pub type Percentage = f64;
	/// -100% - 100%
	pub type SignedPercentage = f64;
	/// -180° - 180°
	pub type Angle = f64;
	/// Ends in the unit of x
	pub type Multiplier = f64;
	/// Non-negative integer with px unit
	pub type PixelLength = f64;
	/// Non-negative
	pub type Length = f64;
	/// 0 to 1
	pub type Fraction = f64;
	/// Unsigned integer
	pub type IntegerCount = u32;
	/// Unsigned integer to be used for random seeds
	pub type SeedValue = u32;
	/// Non-negative integer coordinate with px unit
	pub type Resolution = glam::UVec2;
	/// DVec2 with px unit
	pub type PixelSize = glam::DVec2;
}

// Translation struct between macro and definition
#[derive(Clone)]
pub struct NodeMetadata {
	pub display_name: &'static str,
	pub category: Option<&'static str>,
	pub fields: Vec<FieldMetadata>,
	pub description: &'static str,
	pub properties: Option<&'static str>,
}

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct FieldMetadata {
	pub name: &'static str,
	pub description: &'static str,
	pub exposed: bool,
	pub widget_override: RegistryWidgetOverride,
	pub value_source: RegistryValueSource,
	pub default_type: Option<Type>,
	pub number_min: Option<f64>,
	pub number_max: Option<f64>,
	pub number_mode_range: Option<(f64, f64)>,
	pub number_display_decimal_places: Option<u32>,
	pub number_step: Option<f64>,
	pub unit: Option<&'static str>,
}

pub trait ChoiceTypeStatic: Sized + Copy + crate::vector::misc::AsU32 + Send + Sync {
	const WIDGET_HINT: ChoiceWidgetHint;
	const DESCRIPTION: Option<&'static str>;
	fn list() -> &'static [&'static [(Self, VariantMetadata)]];
}

pub enum ChoiceWidgetHint {
	Dropdown,
	RadioButtons,
}

/// Translation struct between macro and definition.
#[derive(Clone, Debug)]
pub struct VariantMetadata {
	/// Name as declared in source code.
	pub name: Cow<'static, str>,

	/// Name to be displayed in UI.
	pub label: Cow<'static, str>,

	/// User-facing documentation text.
	pub docstring: Option<Cow<'static, str>>,

	/// Name of icon to display in radio buttons and such.
	pub icon: Option<Cow<'static, str>>,
}

#[derive(Clone, Debug)]
pub enum RegistryWidgetOverride {
	None,
	Hidden,
	String(&'static str),
	Custom(&'static str),
}

#[derive(Clone, Debug)]
pub enum RegistryValueSource {
	None,
	Default(&'static str),
	Scope(&'static str),
}

type NodeRegistry = LazyLock<Mutex<HashMap<String, Vec<(NodeConstructor, NodeIOTypes)>>>>;

pub static NODE_REGISTRY: NodeRegistry = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static NODE_METADATA: LazyLock<Mutex<HashMap<String, NodeMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(not(target_arch = "wasm32"))]
pub type DynFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n + Send>>;
#[cfg(target_arch = "wasm32")]
pub type DynFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n>>;
pub type LocalFuture<'n, T> = Pin<Box<dyn core::future::Future<Output = T> + 'n>>;
#[cfg(not(target_arch = "wasm32"))]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n + Send>;
#[cfg(target_arch = "wasm32")]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;
pub type FutureAny<'n> = DynFuture<'n, Any<'n>>;
// TODO: is this safe? This is assumed to be send+sync.
#[cfg(not(target_arch = "wasm32"))]
pub type TypeErasedNode<'n> = dyn for<'i> NodeIO<'i, Any<'i>, Output = FutureAny<'i>> + 'n + Send + Sync;
#[cfg(target_arch = "wasm32")]
pub type TypeErasedNode<'n> = dyn for<'i> NodeIO<'i, Any<'i>, Output = FutureAny<'i>> + 'n;
pub type TypeErasedPinnedRef<'n> = Pin<&'n TypeErasedNode<'n>>;
pub type TypeErasedRef<'n> = &'n TypeErasedNode<'n>;
pub type TypeErasedBox<'n> = Box<TypeErasedNode<'n>>;
pub type TypeErasedPinned<'n> = Pin<Box<TypeErasedNode<'n>>>;

pub type SharedNodeContainer = std::sync::Arc<NodeContainer>;

pub type NodeConstructor = fn(Vec<SharedNodeContainer>) -> DynFuture<'static, TypeErasedBox<'static>>;

#[derive(Clone)]
pub struct NodeContainer {
	#[cfg(feature = "dealloc_nodes")]
	pub node: *const TypeErasedNode<'static>,
	#[cfg(not(feature = "dealloc_nodes"))]
	pub node: TypeErasedRef<'static>,
}

impl Deref for NodeContainer {
	type Target = TypeErasedNode<'static>;

	#[cfg(feature = "dealloc_nodes")]
	fn deref(&self) -> &Self::Target {
		unsafe { &*(self.node) }
		#[cfg(not(feature = "dealloc_nodes"))]
		self.node
	}
	#[cfg(not(feature = "dealloc_nodes"))]
	fn deref(&self) -> &Self::Target {
		self.node
	}
}

/// # Safety
/// Marks NodeContainer as Sync. This dissallows the use of threadlocal storage for nodes as this would invalidate references to them.
// TODO: implement this on a higher level wrapper to avoid missuse
#[cfg(feature = "dealloc_nodes")]
unsafe impl Send for NodeContainer {}
#[cfg(feature = "dealloc_nodes")]
unsafe impl Sync for NodeContainer {}

#[cfg(feature = "dealloc_nodes")]
impl Drop for NodeContainer {
	fn drop(&mut self) {
		unsafe { self.dealloc_unchecked() }
	}
}

impl core::fmt::Debug for NodeContainer {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeContainer").finish()
	}
}

impl NodeContainer {
	pub fn new(node: TypeErasedBox<'static>) -> SharedNodeContainer {
		let node = Box::leak(node);
		Self { node }.into()
	}

	#[cfg(feature = "dealloc_nodes")]
	unsafe fn dealloc_unchecked(&mut self) {
		unsafe {
			std::mem::drop(Box::from_raw(self.node as *mut TypeErasedNode));
		}
	}
}

/// Boxes the input and downcasts the output.
/// Wraps around a node taking Box<dyn DynAny> and returning Box<dyn DynAny>
#[derive(Clone)]
pub struct DowncastBothNode<I, O> {
	node: SharedNodeContainer,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
impl<'input, O, I> Node<'input, I> for DowncastBothNode<I, O>
where
	O: 'input + StaticType + WasmNotSend,
	I: 'input + StaticType + WasmNotSend,
{
	type Output = DynFuture<'input, O>;
	#[inline]
	fn eval(&'input self, input: I) -> Self::Output {
		{
			let node_name = self.node.node_name();
			let input = Box::new(input);
			let future = self.node.eval(input);
			Box::pin(async move {
				let out = dyn_any::downcast(future.await).unwrap_or_else(|e| panic!("DowncastBothNode Input {e} in: \n{node_name}"));
				*out
			})
		}
	}
	fn reset(&self) {
		self.node.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any + Send + Sync>> {
		self.node.serialize()
	}
}
impl<I, O> DowncastBothNode<I, O> {
	pub const fn new(node: SharedNodeContainer) -> Self {
		Self {
			node,
			_i: core::marker::PhantomData,
			_o: core::marker::PhantomData,
		}
	}
}
pub struct FutureWrapperNode<Node> {
	node: Node,
}

impl<'i, T: 'i + WasmNotSend, N> Node<'i, T> for FutureWrapperNode<N>
where
	N: Node<'i, T, Output: WasmNotSend> + WasmNotSend,
{
	type Output = DynFuture<'i, N::Output>;
	#[inline(always)]
	fn eval(&'i self, input: T) -> Self::Output {
		let result = self.node.eval(input);
		Box::pin(async move { result })
	}
	#[inline(always)]
	fn reset(&self) {
		self.node.reset();
	}

	#[inline(always)]
	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any + Send + Sync>> {
		self.node.serialize()
	}
}

impl<N> FutureWrapperNode<N> {
	pub const fn new(node: N) -> Self {
		Self { node }
	}
}

pub struct DynAnyNode<I, O, Node> {
	node: Node,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}

impl<'input, I, O, N> Node<'input, Any<'input>> for DynAnyNode<I, O, N>
where
	I: 'input + dyn_any::StaticType + WasmNotSend,
	O: 'input + dyn_any::StaticType + WasmNotSend,
	N: 'input + Node<'input, I, Output = DynFuture<'input, O>>,
{
	type Output = FutureAny<'input>;
	#[inline]
	fn eval(&'input self, input: Any<'input>) -> Self::Output {
		let node_name = core::any::type_name::<N>();
		let output = |input| {
			let result = self.node.eval(input);
			async move { Box::new(result.await) as Any<'input> }
		};
		match dyn_any::downcast(input) {
			Ok(input) => Box::pin(output(*input)),
			Err(e) => panic!("DynAnyNode Input, {0} in:\n{1}", e, node_name),
		}
	}

	fn reset(&self) {
		self.node.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn core::any::Any + Send + Sync>> {
		self.node.serialize()
	}
}
impl<'input, I, O, N> DynAnyNode<I, O, N>
where
	I: 'input + dyn_any::StaticType,
	O: 'input + dyn_any::StaticType,
	N: 'input + Node<'input, I, Output = DynFuture<'input, O>>,
{
	pub const fn new(node: N) -> Self {
		Self {
			node,
			_i: core::marker::PhantomData,
			_o: core::marker::PhantomData,
		}
	}
}
pub struct PanicNode<I: WasmNotSend, O: WasmNotSend>(PhantomData<I>, PhantomData<O>);

impl<'i, I: 'i + WasmNotSend, O: 'i + WasmNotSend> Node<'i, I> for PanicNode<I, O> {
	type Output = O;
	fn eval(&'i self, _: I) -> Self::Output {
		unimplemented!("This node should never be evaluated")
	}
}

impl<I: WasmNotSend, O: WasmNotSend> PanicNode<I, O> {
	pub const fn new() -> Self {
		Self(PhantomData, PhantomData)
	}
}

impl<I: WasmNotSend, O: WasmNotSend> Default for PanicNode<I, O> {
	fn default() -> Self {
		Self::new()
	}
}

// TODO: Evaluate safety
unsafe impl<I: WasmNotSend, O: WasmNotSend> Sync for PanicNode<I, O> {}
