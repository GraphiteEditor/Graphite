use crate::{Color, ContextFeature, Node, NodeIO, NodeIOTypes, ProtoNodeIdentifier, Type, WasmNotSend};
use dyn_any::{DynAny, StaticType};
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
	/// The output connectors of a `destructure_output` node, taken from the [`Destructure`] struct it returns.
	pub output_fields: Option<DestructureMetadata>,
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
}

/// Metadata for a `#[derive(node_macro::Destructure)]` struct, describing how its fields are broken out into individual node connectors.
/// Produced by [`Destructure::metadata`] and stored in [`NodeMetadata::output_fields`] for each node declared `destructure_output`.
#[derive(Clone, Debug)]
pub struct DestructureMetadata {
	/// The fields in output-connector order, starting with the one marked `#[primary]` if any.
	pub fields: Vec<DestructureFieldMetadata>,
	pub has_primary: bool,
	/// The type of the struct's rank-lifted twin ([`Destructure::Mapped`]), which the node's mapped variant returns when framed over a list.
	pub mapped_type: Type,
}

impl DestructureMetadata {
	/// Whether output 0 is a hidden output carrying the whole struct, which is the case unless a field is marked `#[primary]` to take its place.
	pub fn hidden_primary_output(&self) -> bool {
		!self.has_primary
	}

	/// One output per field, preceded by the hidden struct output if there is one.
	pub fn number_of_outputs(&self) -> usize {
		self.fields.len() + usize::from(self.hidden_primary_output())
	}

	/// The index into [`Self::fields`] of the field exported at the given output, or `None` for the hidden struct output.
	pub fn field_index_for_output(&self, output_index: usize) -> Option<usize> {
		if self.hidden_primary_output() { output_index.checked_sub(1) } else { Some(output_index) }
	}
}

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct DestructureFieldMetadata {
	pub name: &'static str,
	pub description: &'static str,
	/// The generated proto node that extracts this field from the struct or from its rank-lifted twin.
	pub extractor: ProtoNodeIdentifier,
	/// The field's wire type as declared on the struct, `Item<T>` or `List<T>`.
	pub ty: Type,
	/// The field's wire type on the rank-lifted twin, always `List<T>`.
	pub mapped_ty: Type,
}

/// A struct of wires returned by a multi-output node, implemented by `#[derive(node_macro::Destructure)]`.
///
/// Each field is an `Item<T>` or `List<T>` wire that becomes one output connector. The struct travels only within the network
/// the Graphene preprocessor expands the node into, where a generated extractor node exports each field.
pub trait Destructure: Sized {
	/// The rank-lifted twin returned by the node's mapped variant when it is framed over a list: each `Item<T>` field
	/// becomes `List<T>` and each `List<T>` field stays `List<T>`, flat-mapped per the rank-2 rule.
	type Mapped;

	fn metadata() -> DestructureMetadata;

	/// An empty twin with each field's list sized for the given number of frame slots.
	fn mapped_with_capacity(capacity: usize) -> Self::Mapped;

	/// Appends this struct's fields to the twin as one frame slot, pushing each `Item<T>` field and extending with each `List<T>` field.
	fn push_into(self, mapped: &mut Self::Mapped);
}

/// Moves one field out of a [`Destructure`] struct or its rank-lifted twin, by the field's output-connector index.
/// The generated extractor nodes are written against this trait so one node serves both rank forms.
pub trait DestructureField<const INDEX: usize> {
	type Wire;

	fn field(self) -> Self::Wire;
}

type NodeRegistry = LazyLock<Mutex<HashMap<ProtoNodeIdentifier, Vec<(NodeConstructor, NodeIOTypes)>>>>;

pub static NODE_REGISTRY: NodeRegistry = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static NODE_METADATA: LazyLock<Mutex<HashMap<ProtoNodeIdentifier, NodeMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// All multi-output proto nodes (those declared `destructure_output`), keyed by their identifier.
/// Snapshotted on first access, which must happen after startup registration of the node metadata completes.
pub static MULTI_OUTPUT_NODES: LazyLock<HashMap<ProtoNodeIdentifier, DestructureMetadata>> = LazyLock::new(|| {
	NODE_METADATA
		.lock()
		.unwrap()
		.iter()
		.filter_map(|(identifier, metadata)| metadata.output_fields.clone().map(|output_fields| (identifier.clone(), output_fields)))
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
