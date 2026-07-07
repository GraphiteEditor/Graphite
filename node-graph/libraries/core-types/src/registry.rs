use crate::{ContextFeature, Node, NodeIO, NodeIOTypes, ProtoNodeIdentifier, Type, WasmNotSend};
use dyn_any::{DynAny, StaticType};
pub use no_std_types::registry::types;
use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;
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

/// Metadata for a struct tagged with `#[node_macro::destructure]`, describing how its fields are broken out into individual node connectors.
/// Registered by the macro into [`DESTRUCTURE_METADATA`], keyed by the [`TypeId`] of the struct and of its `Item`/`List` wire forms.
///
/// Currently used for node outputs: a node function returning such a struct becomes a multi-output node whose outputs are the struct's fields.
/// The same registration is intended to eventually also drive destructured inputs, where a single struct parameter expands into one input connector per field.
#[derive(Clone, Debug)]
pub struct DestructureMetadata {
	/// The fields in output-connector order. When `has_primary` is true the first entry is the field marked `#[primary]`,
	/// exposed as the node's primary output at index 0 with the remaining fields following it. Otherwise a hidden primary
	/// output carrying the whole struct occupies index 0 and the fields are the secondary outputs at indices 1 and up.
	pub fields: Vec<DestructureFieldMetadata>,
	pub has_primary: bool,
	/// The struct's canonical type name from [`std::any::type_name`], used to match registry rows whose element descriptors carry no [`TypeId`].
	pub struct_name: &'static str,
}

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct DestructureFieldMetadata {
	pub name: &'static str,
	pub description: &'static str,
	/// The generated proto node that extracts this field from the struct value.
	pub extractor: ProtoNodeIdentifier,
	/// The concrete type of the field.
	pub ty: Type,
}

type NodeRegistry = LazyLock<Mutex<HashMap<ProtoNodeIdentifier, Vec<(NodeConstructor, NodeIOTypes)>>>>;

pub static NODE_REGISTRY: NodeRegistry = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static NODE_METADATA: LazyLock<Mutex<HashMap<ProtoNodeIdentifier, NodeMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static DESTRUCTURE_METADATA: LazyLock<Mutex<HashMap<TypeId, DestructureMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Looks up the [`DestructureMetadata`] registered for a node's return type, if that type is a `#[node_macro::destructure]` struct.
/// Accepts the type as stored in [`NodeIOTypes::return_value`], unwrapping any `Future` wrapper and the `Item`/`List` rank around the concrete element type.
pub fn destructure_metadata_for_type(return_type: &Type) -> Option<DestructureMetadata> {
	let element_type = match return_type.nested_type() {
		Type::Item(inner) | Type::List(inner) => inner.nested_type(),
		other => other,
	};
	let Type::Concrete(descriptor) = element_type else { return None };
	let type_id = descriptor.id?;
	DESTRUCTURE_METADATA.lock().unwrap().get(&type_id).cloned()
}

/// All multi-output proto nodes (those whose return type is a `#[node_macro::destructure]` struct), keyed by their identifier.
/// Snapshotted on first access, which must happen after startup registration of the node and destructure registries completes.
pub static MULTI_OUTPUT_NODES: LazyLock<HashMap<ProtoNodeIdentifier, DestructureMetadata>> = LazyLock::new(|| {
	let node_registry = NODE_REGISTRY.lock().unwrap();
	node_registry
		.iter()
		.filter_map(|(identifier, implementations)| {
			let (_, node_io) = implementations.first()?;
			destructure_metadata_for_type(&node_io.return_value).map(|metadata| (identifier.clone(), metadata))
		})
		.collect()
});

#[cfg(not(target_family = "wasm"))]
pub type DynFuture<'n, T> = Pin<Box<dyn Future<Output = T> + 'n + Send>>;
#[cfg(target_family = "wasm")]
pub type DynFuture<'n, T> = Pin<Box<dyn std::future::Future<Output = T> + 'n>>;
pub type LocalFuture<'n, T> = Pin<Box<dyn Future<Output = T> + 'n>>;
#[cfg(not(target_family = "wasm"))]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n + Send>;
#[cfg(target_family = "wasm")]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;
pub type FutureAny<'n> = DynFuture<'n, Any<'n>>;
// TODO: is this safe? This is assumed to be send+sync.
#[cfg(not(target_family = "wasm"))]
pub type TypeErasedNode<'n> = dyn for<'i> NodeIO<'i, Any<'i>, Output = FutureAny<'i>> + 'n + Send + Sync;
#[cfg(target_family = "wasm")]
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
	#[track_caller]
	fn eval(&'input self, input: I) -> Self::Output {
		{
			let node_name = self.node.node_name();
			let input = Box::new(input);
			let future = self.node.eval(input);
			Box::pin(async move {
				let out = dyn_any::downcast(future.await).unwrap_or_else(|e| panic!("DowncastBothNode wrong output type: {e} in: \n{node_name}"));
				*out
			})
		}
	}
	fn reset(&self) {
		self.node.reset();
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
	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
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
	I: 'input + StaticType + WasmNotSend,
	O: 'input + StaticType + WasmNotSend,
	N: 'input + Node<'input, I, Output = DynFuture<'input, O>>,
{
	type Output = FutureAny<'input>;
	#[inline]
	fn eval(&'input self, input: Any<'input>) -> Self::Output {
		let node_name = std::any::type_name::<N>();
		let output = |input| {
			let result = self.node.eval(input);
			async move { Box::new(result.await) as Any<'input> }
		};
		match dyn_any::downcast(input) {
			Ok(input) => Box::pin(output(*input)),
			Err(e) => panic!("DynAnyNode Input, {e} in:\n{node_name}"),
		}
	}

	fn reset(&self) {
		self.node.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		self.node.serialize()
	}
}
impl<'input, I, O, N> DynAnyNode<I, O, N>
where
	I: 'input + StaticType,
	O: 'input + StaticType,
	N: 'input + Node<'input, I, Output = DynFuture<'input, O>>,
{
	pub const fn new(node: N) -> Self {
		Self {
			node,
			_i: PhantomData,
			_o: PhantomData,
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
