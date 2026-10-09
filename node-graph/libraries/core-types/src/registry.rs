use crate::poll::Poll;
use crate::{Color, ContextFeature, Node, NodeIOTypes, ProtoNodeIdentifier, Type, WasmNotSend};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{LazyLock, Mutex};

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct NodeMetadata {
	pub display_name: &'static str,
	pub category: &'static str,
	pub fields: Vec<FieldMetadata>,
	pub description: &'static str,
	pub properties: Option<&'static str>,
	pub context_features: Vec<ContextFeature>,
	pub memoize: bool,
	pub inject_scope: bool,
	pub asynchronous: bool,
}

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct FieldMetadata {
	pub name: &'static str,
	pub description: &'static str,
	pub hidden: bool,
	pub exposed: bool,
	pub widget_override: RegistryWidgetOverride,
	pub value_source: RegistryValueSource,
	/// The default expression's colors, resolved by the macro when the expression consists solely of `Color::*` constants.
	pub default_colors: Option<&'static [Color]>,
	pub default_type: Option<Type>,
	/// The slider's suggested extent, from `#[soft(a..b)]`. Typed values may exceed it.
	pub number_soft_min: Option<f64>,
	pub number_soft_max: Option<f64>,
	/// The enforced clamp, from `#[hard(a..b)]`. Applied to typed values and at eval time.
	pub number_hard_min: Option<f64>,
	pub number_hard_max: Option<f64>,
	pub number_mode_range: bool,
	pub number_display_decimal_places: Option<u32>,
	pub number_step: Option<f64>,
	pub unit: Option<&'static str>,
	/// Whether a `String` parameter uses the multi-line text area widget instead of the single-line field, from `#[multiline]`.
	pub multiline: bool,
	/// Whether a number parameter uses the progression widget, which splits the value into fractional-progress and whole-element-number fields, from `#[progression]`.
	pub progression: bool,
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
	SourceId,
}

type NodeRegistry = LazyLock<Mutex<HashMap<ProtoNodeIdentifier, Vec<(NodeConstructor, NodeIOTypes)>>>>;

pub static NODE_REGISTRY: NodeRegistry = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static NODE_METADATA: LazyLock<Mutex<HashMap<ProtoNodeIdentifier, NodeMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(not(target_family = "wasm"))]
pub type Any = Box<dyn std::any::Any + Send>;
#[cfg(target_family = "wasm")]
pub type Any = Box<dyn std::any::Any>;
#[cfg(not(target_family = "wasm"))]
pub type AnyInput = dyn std::any::Any + Send;
#[cfg(target_family = "wasm")]
pub type AnyInput = dyn std::any::Any;
// TODO: is this safe? This is assumed to be send+sync.
#[cfg(not(target_family = "wasm"))]
pub type TypeErasedNode<'n> = dyn Node<AnyInput, Output = Any> + 'n + Send + Sync;
#[cfg(target_family = "wasm")]
pub type TypeErasedNode<'n> = dyn Node<AnyInput, Output = Any> + 'n;
pub type TypeErasedRef<'n> = &'n TypeErasedNode<'n>;
pub type TypeErasedBox<'n> = Box<TypeErasedNode<'n>>;

pub type SharedNodeContainer = std::sync::Arc<NodeContainer>;

pub type NodeConstructor = fn(Vec<SharedNodeContainer>) -> TypeErasedBox<'static>;

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

impl std::fmt::Debug for NodeContainer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
			drop(Box::from_raw(self.node as *mut TypeErasedNode));
		}
	}
}

/// Erases the input and downcasts the output.
/// Wraps around a node taking `&dyn Any` and returning `Box<dyn Any>`
#[derive(Clone)]
pub struct DowncastBothNode<I, O> {
	node: SharedNodeContainer,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}
impl<I, O> Node<I> for DowncastBothNode<I, O>
where
	I: 'static + WasmNotSend,
	O: 'static + WasmNotSend,
{
	type Output = O;
	#[inline]
	fn eval(&self, input: &I) -> Poll<O> {
		self.node.eval(input).map(|output| {
			*output
				.downcast::<O>()
				.unwrap_or_else(|_| panic!("DowncastBothNode wrong output type: expected {} in: \n{}", std::any::type_name::<O>(), self.node.node_name()))
		})
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		self.node.serialize()
	}
}
impl<I, O> DowncastBothNode<I, O> {
	pub const fn new(node: SharedNodeContainer) -> Self {
		Self {
			node,
			_i: PhantomData,
			_o: PhantomData,
		}
	}
}

pub struct DynAnyNode<I, O, Node> {
	node: Node,
	_i: PhantomData<I>,
	_o: PhantomData<O>,
}

impl<I, O, N> Node<AnyInput> for DynAnyNode<I, O, N>
where
	I: 'static,
	O: 'static + WasmNotSend,
	N: Node<I, Output = O>,
{
	type Output = Any;
	#[inline]
	fn eval(&self, input: &AnyInput) -> Poll<Any> {
		let input = input
			.downcast_ref::<I>()
			.unwrap_or_else(|| panic!("DynAnyNode Input: expected {} in:\n{}", std::any::type_name::<I>(), std::any::type_name::<N>()));
		self.node.eval(input).map(|output| Box::new(output) as Any)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		self.node.serialize()
	}
}
impl<I, O, N> DynAnyNode<I, O, N> {
	pub const fn new(node: N) -> Self {
		Self {
			node,
			_i: PhantomData,
			_o: PhantomData,
		}
	}
}
pub struct PanicNode<I: WasmNotSend, O: WasmNotSend>(PhantomData<I>, PhantomData<O>);

impl<I: WasmNotSend, O: WasmNotSend> Node<I> for PanicNode<I, O> {
	type Output = O;
	fn eval(&self, _: &I) -> Poll<O> {
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
