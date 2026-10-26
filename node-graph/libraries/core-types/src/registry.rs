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
	pub icon: Option<&'static str>,
	pub fields: Vec<FieldMetadata>,
	pub description: &'static str,
	pub inspector: Option<&'static str>,
	pub context_features: Vec<ContextFeature>,
	pub memoize: bool,
	pub footprint_invariant: bool,
	pub inject_scope: bool,
	pub asynchronous: bool,
	pub output_fields: Option<DestructureMetadata>,
}

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct FieldMetadata {
	pub name: &'static str,
	pub description: &'static str,
	/// Whether the parameter is internal state of its node (named with a leading `_`), which is never exposed to the graph.
	pub internal: bool,
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

#[derive(Clone, Debug)]
pub struct DestructureMetadata {
	pub fields: Vec<DestructureFieldMetadata>,
	pub has_primary: bool,
	/// The type of [`Destructure::Mapped`], which the node's mapped variant returns.
	pub mapped_type: Type,
}

impl DestructureMetadata {
	/// Whether output 0 is a hidden output, which is the case unless a field is marked `#[primary]` to take its place.
	pub fn hidden_primary_output(&self) -> bool {
		!self.has_primary
	}

	/// One output per field, preceded by the hidden primary output if there is one.
	pub fn number_of_outputs(&self) -> usize {
		self.fields.len() + usize::from(self.hidden_primary_output())
	}

	/// The index into [`Self::fields`] of the field exported at the given output, or `None` for the hidden primary output.
	pub fn field_index_for_output(&self, output_index: usize) -> Option<usize> {
		if self.hidden_primary_output() { output_index.checked_sub(1) } else { Some(output_index) }
	}
}

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct DestructureFieldMetadata {
	pub name: &'static str,
	pub description: &'static str,
	/// The generated proto node that extracts this field from the struct or its [`Destructure::Mapped`] form.
	pub extractor: ProtoNodeIdentifier,
	/// The field's type as declared on the struct, `Item<T>` or `List<T>`.
	pub ty: Type,
	/// The field's type in [`Destructure::Mapped`], always `List<T>`.
	pub mapped_ty: Type,
}

/// A struct returned by a multi-output node, with each of its `Item<T>` or `List<T>` fields becoming an output connector.
/// Implemented by `#[derive(node_macro::Destructure)]`.
pub trait Destructure: Sized {
	/// The struct returned by the node's mapped variant, with every field as a `List<T>`. Lists from a `List<T>` field are
	/// concatenated across frame slots, since nested lists aren't supported.
	type Mapped;

	fn metadata() -> DestructureMetadata;

	/// An empty [`Self::Mapped`] with each field's list allocated for the given number of frame slots.
	fn mapped_with_capacity(capacity: usize) -> Self::Mapped;

	/// Appends this struct's fields to a [`Self::Mapped`], pushing each `Item<T>` field and extending with each `List<T>` field.
	fn push_into(self, mapped: &mut Self::Mapped);
}

/// Moves the field at position `INDEX` out of a [`Destructure`] struct or its [`Destructure::Mapped`] form, so one generated
/// extractor node accepts both.
pub trait ExtractField<const INDEX: usize> {
	type Field;

	fn extract_field(self) -> Self::Field;
}

/// Lets the node macro read a return type's [`DestructureMetadata`] through autoref specialization, with
/// [`DestructureOutputFields`] applying to [`Destructure`] types and [`NoOutputFields`] to all others.
pub struct OutputFields<T>(pub PhantomData<T>);

pub trait DestructureOutputFields {
	fn output_fields(&self) -> Option<DestructureMetadata>;
}

impl<T: Destructure> DestructureOutputFields for OutputFields<T> {
	fn output_fields(&self) -> Option<DestructureMetadata> {
		Some(T::metadata())
	}
}

pub trait NoOutputFields {
	fn output_fields(&self) -> Option<DestructureMetadata>;
}

impl<T> NoOutputFields for &OutputFields<T> {
	fn output_fields(&self) -> Option<DestructureMetadata> {
		None
	}
}

/// Whether nodes in this category stay out of the node catalog: the empty category, or one starting with `_` for generated nodes.
pub fn is_hidden_category(category: &str) -> bool {
	category.is_empty() || category.starts_with('_')
}

/// Whether nodes in this category are developer tools for testing, listed in the node catalog only in development builds.
pub fn is_development_category(category: &str) -> bool {
	category == "Debug"
}

/// The node catalog's categories in display order, each master category's own nodes first and then its subcategories.
pub const CATEGORY_ORDER: &[&str] = &[
	"Value",
	"Routing",
	"Transform",
	"Graphic",
	"Vector: Style",
	"Vector: Geometry",
	"Vector: Point",
	"Vector: Path",
	"Vector: Deform",
	"Vector: Measure",
	"Raster",
	"Raster: Effect",
	"Raster: Adjustment",
	"Raster: Filter",
	"Color",
	"Gradient",
	"Typography",
	"String",
	"String: Regex",
	"String: JSON",
	"Math",
	"Math: Arithmetic",
	"Math: Logic",
	"Math: Numeric",
	"Math: Trig",
	"Math: Vec2",
	"Data: List",
	"Data: Context",
	"Data: Animation",
	"Data: Attribute",
	"Data: Type Assertion",
	"Data: I/O",
];

/// Sorts categories into the node catalog's display order, with any unlisted category (such as Debug) after the listed ones.
pub fn category_sort_key(category: &str) -> (usize, &str) {
	(CATEGORY_ORDER.iter().position(|listed| *listed == category).unwrap_or(CATEGORY_ORDER.len()), category)
}

type NodeRegistry = LazyLock<Mutex<HashMap<ProtoNodeIdentifier, Vec<(NodeConstructor, NodeIOTypes)>>>>;

pub static NODE_REGISTRY: NodeRegistry = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static NODE_METADATA: LazyLock<Mutex<HashMap<ProtoNodeIdentifier, NodeMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// All multi-output proto nodes (those returning a [`Destructure`] struct), keyed by their identifier.
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
