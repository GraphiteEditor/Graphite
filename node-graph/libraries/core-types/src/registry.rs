use crate::concrete;
use crate::context::{Context, ContextImpl};
use crate::node::Node;
use crate::{ContextFeature, ProtoNodeIdentifier, Type, WasmNotSend, WasmNotSync};
use dyn_any::DynAny;
use graphene_hash::CacheHash;
pub use no_std_types::registry::types;
use std::collections::HashMap;
use std::hash::Hasher;
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
	/// The macro appended its hidden `_runtime` and `_source` fields as the last two entries of `fields`.
	pub async_source_fields: bool,
}

// Translation struct between macro and definition
#[derive(Clone, Debug)]
pub struct FieldMetadata {
	pub name: &'static str,
	pub description: &'static str,
	/// Index levels the node pushes when evaluating this input.
	pub pushed_levels: u8,
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
	SourceId,
}

type NodeRegistry = LazyLock<Mutex<HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>>>>;

pub static NODE_REGISTRY: NodeRegistry = LazyLock::new(|| Mutex::new(HashMap::new()));

pub static NODE_METADATA: LazyLock<Mutex<HashMap<ProtoNodeIdentifier, NodeMetadata>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub use crate::NodeIOTypes;

/// Element-independent by erasure; the wire's `Type::Record(El)` keeps element reads proven at wiring.
#[cfg(not(target_family = "wasm"))]
pub type ErasedRecordNode = dyn for<'c> Node<ContextImpl<'c>> + Send + Sync;
#[cfg(target_family = "wasm")]
pub type ErasedRecordNode = dyn for<'c> Node<ContextImpl<'c>>;

#[cfg(not(target_family = "wasm"))]
type DynEdge = dyn std::any::Any + Send + Sync;
#[cfg(target_family = "wasm")]
type DynEdge = dyn std::any::Any;

pub fn record_type<T: 'static>() -> Type {
	Type::Record(Box::new(concrete!(T)))
}

pub fn record_edge_type<T: 'static>() -> Type {
	Type::Fn(Box::new(concrete!(Context)), Box::new(record_type::<T>()))
}

/// The record edge type of a token row, generic over the element.
pub fn generic_record_edge_type(name: &'static str) -> Type {
	Type::Fn(Box::new(concrete!(Context)), Box::new(Type::Record(Box::new(Type::Generic(std::borrow::Cow::Borrowed(name))))))
}

pub fn cache_key<C: CacheHash + ?Sized>(ctx: &C) -> u64 {
	let mut hasher = graphene_hash::FxHasher64::new();
	ctx.cache_hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug, PartialEq)]
pub enum ConstructionError {
	Arity { expected: usize, got: usize },
	Type { expected: Box<Type>, found: Box<Type> },
}

pub struct SharedEdge<N: ?Sized> {
	ptr: std::ptr::NonNull<N>,
	own: std::sync::Arc<N>,
}

impl<N: ?Sized> SharedEdge<N> {
	pub fn new(own: std::sync::Arc<N>) -> Self {
		Self {
			ptr: std::ptr::NonNull::from(&*own),
			own,
		}
	}

	pub fn share(&self) -> Self {
		Self { ptr: self.ptr, own: self.own.clone() }
	}

	/// Re-derives the cached pointer from the owned payload. An exclusive
	/// re-borrow of the payload invalidates the pointer taken before it, so
	/// every mutation through `own` ends here.
	pub fn rederive(&mut self) {
		self.ptr = std::ptr::NonNull::from(&*self.own);
	}
}

// SAFETY: `ptr` is derived from the owned Arc and never mutated through, so the edge is exactly as
// thread safe as the payload it shares.
unsafe impl<N: ?Sized + Send + Sync> Send for SharedEdge<N> {}
// SAFETY: as in Send.
unsafe impl<N: ?Sized + Send + Sync> Sync for SharedEdge<N> {}

impl<Input, N> Node<Input> for SharedEdge<N>
where
	N: Node<Input> + ?Sized,
{
	/// A node takes exactly its own frame out of its caller's free space: the
	/// caller minted the claim and kept the cursor, so the frame accounting is
	/// structural here and asserted where a claim is split.
	fn serve<'e, 'l>(&self, input: &Input, slot: crate::record::FrameClaim<'e, 'l>) -> crate::gpoll::GPoll<crate::record::Served<'e>>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		// SAFETY: `own` keeps the payload alive for `self`'s lifetime and Arc
		// payloads are address stable.
		unsafe { self.ptr.as_ref() }.serve(input, slot)
	}

	fn extent_at<'x>(&self, input: &Input, level: u8, frames: &crate::record::Frames<'x>) -> crate::gpoll::GPoll<crate::gpoll::Extent>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'x crate::arena::Arena>,
	{
		// SAFETY: as in serve.
		unsafe { self.ptr.as_ref() }.extent_at(input, level, frames)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		// SAFETY: as in serve.
		unsafe { self.ptr.as_ref() }.serialize()
	}

	fn layout(&self) -> &crate::record::Layout {
		// SAFETY: as in serve.
		unsafe { self.ptr.as_ref() }.layout()
	}

	fn eval_batch<'a, 'x>(
		&'a self,
		input: &'a Input,
		range: std::ops::Range<u64>,
		scratch: Option<&'a mut [std::mem::MaybeUninit<u64>]>,
		frames: &crate::record::Frames<'x>,
	) -> crate::node::BatchStatus<'a>
	where
		Input: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'x crate::arena::Arena>,
	{
		// SAFETY: as in serve.
		unsafe { self.ptr.as_ref() }.eval_batch(input, range, scratch, frames)
	}
}

pub struct EdgeHandle {
	node: Box<DynEdge>,
	share: fn(&DynEdge) -> Box<DynEdge>,
	serialize: fn(&DynEdge) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
	layout: fn(&DynEdge) -> &crate::record::Layout,
	set_layout: fn(&mut DynEdge, crate::record::RecordLayout),
	ty: Type,
}

impl std::fmt::Debug for EdgeHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("EdgeHandle").field("ty", &self.ty).finish_non_exhaustive()
	}
}

// SAFETY: wasm is single threaded, so the marker-free payload never actually crosses a thread.
#[cfg(target_family = "wasm")]
unsafe impl Send for EdgeHandle {}
// SAFETY: as in Send.
#[cfg(target_family = "wasm")]
unsafe impl Sync for EdgeHandle {}

impl EdgeHandle {
	pub fn new_record<T: 'static>(node: std::sync::Arc<ErasedRecordNode>) -> Self {
		Self::new_erased(node, record_edge_type::<T>())
	}

	pub fn new_erased<N>(node: std::sync::Arc<N>, ty: Type) -> Self
	where
		N: ?Sized + 'static + for<'c> Node<ContextImpl<'c>>,
		SharedEdge<N>: WasmNotSend + WasmNotSync,
	{
		Self {
			node: Box::new(SharedEdge::new(node)),
			share: |edge| Box::new(edge.downcast_ref::<SharedEdge<N>>().expect("share hook matches the stored edge type").share()),
			serialize: |edge| Node::<ContextImpl>::serialize(edge.downcast_ref::<SharedEdge<N>>().expect("serialize hook matches the stored edge type")),
			layout: |edge| Node::<ContextImpl>::layout(edge.downcast_ref::<SharedEdge<N>>().expect("layout hook matches the stored edge type")),
			set_layout: |edge, layout| {
				let shared = edge.downcast_mut::<SharedEdge<N>>().expect("set_layout hook matches the stored edge type");
				let node = std::sync::Arc::get_mut(&mut shared.own).expect("layout is installed before the node is shared");
				Node::<ContextImpl>::set_layout(node, layout);
				shared.rederive();
			},
			ty,
		}
	}

	pub fn ty(&self) -> &Type {
		&self.ty
	}

	pub fn duplicate(&self) -> Self {
		Self {
			node: (self.share)(&*self.node),
			share: self.share,
			serialize: self.serialize,
			layout: self.layout,
			set_layout: self.set_layout,
			ty: self.ty.clone(),
		}
	}

	pub fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(self.serialize)(&*self.node)
	}

	pub fn layout(&self) -> &crate::record::Layout {
		(self.layout)(&*self.node)
	}

	pub fn set_layout(&mut self, layout: crate::record::RecordLayout) {
		(self.set_layout)(&mut *self.node, layout);
	}

	pub fn downcast_record<T: 'static>(self) -> Result<SharedEdge<ErasedRecordNode>, ConstructionError> {
		self.downcast_erased(record_edge_type::<T>())
	}

	/// The erased record edge, for callers that dispatch on the layout rather
	/// than a static element type. `None` for an edge erased to another node type.
	pub fn record_edge(self) -> Option<SharedEdge<ErasedRecordNode>> {
		self.node.downcast::<SharedEdge<ErasedRecordNode>>().ok().map(|edge| *edge)
	}

	pub fn downcast_erased<N: ?Sized + 'static>(self, expected: Type) -> Result<SharedEdge<N>, ConstructionError> {
		let found = self.ty;
		self.node.downcast::<SharedEdge<N>>().map(|edge| *edge).map_err(|_| ConstructionError::Type {
			expected: Box::new(expected),
			found: Box::new(found),
		})
	}
}

pub type NodeConstructor = fn(Vec<EdgeHandle>) -> Result<EdgeHandle, ConstructionError>;

#[derive(Clone)]
pub struct RegistryEntry {
	pub io: NodeIOTypes,
	pub constructor: NodeConstructor,
	/// Declarative record-io metadata for the compiler layout pass; `None` for
	/// nodes whose layout the pass does not yet fold (routing/opaque, hand-written
	/// rows), which keep the construction-time path.
	pub layout_meta: Option<crate::record::LayoutMeta>,
}

pub fn construct(entry: &RegistryEntry, inputs: Vec<EdgeHandle>) -> Result<EdgeHandle, ConstructionError> {
	if inputs.len() != entry.io.inputs.len() {
		return Err(ConstructionError::Arity {
			expected: entry.io.inputs.len(),
			got: inputs.len(),
		});
	}
	for (handle, expected) in inputs.iter().zip(&entry.io.inputs) {
		if handle.ty() != expected {
			return Err(ConstructionError::Type {
				expected: Box::new(expected.clone()),
				found: Box::new(handle.ty().clone()),
			});
		}
	}
	(entry.constructor)(inputs)
}

#[cfg(not(target_family = "wasm"))]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n + Send>;
#[cfg(target_family = "wasm")]
pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::SourceId;
	use crate::arena::Arena;
	use crate::context::{Ctx, EvalScope, ExtractArena, ExtractIndices};
	use crate::gpoll::GPoll;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicU32, Ordering};

	use crate::record::{FrameClaim, Layout, LiftedSource, Served, element_write, read_element, serve_input};

	fn counting() -> LiftedSource<u32, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<u32>> {
		let count = AtomicU32::new(0);
		LiftedSource::new(move |_: &ContextImpl<'_>| GPoll::Final(count.fetch_add(1, Ordering::Relaxed) + 1))
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	/// Serves a parked borrow of its own value: the element is a reference into
	/// the evaluation's arena, so its lifetime is the serving one.
	struct LendNode {
		value: String,
		layout: Layout,
	}

	impl LendNode {
		fn new(value: &str) -> Self {
			Self {
				value: value.to_string(),
				layout: Layout::default().with_writes(0, element_write::<&'static String>(), &[]),
			}
		}
	}

	impl<Input: Ctx> Node<Input> for LendNode {
		fn serve<'e, 'l>(&self, input: &Input, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			Input: ExtractArena<ArenaRef = &'e Arena>,
		{
			match input.arena().alloc(self.value.clone()) {
				Some((parked, _)) => slot.lift_served(GPoll::Final(parked), input.arena()),
				None => GPoll::arena_exhausted(),
			}
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	#[test]
	fn borrow_carrying_value_types_wire_through_the_general_constructor() {
		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let frames = crate::record::test_frames(1 << 12);

		let node = LendNode::new("held");
		let layout = Node::<ContextImpl>::layout(&node).clone();
		let handle = EdgeHandle::new_erased(Arc::new(node) as Arc<ErasedRecordNode>, concrete!(String));
		assert_eq!(*handle.ty(), concrete!(String));

		let wired = handle.downcast_erased::<ErasedRecordNode>(concrete!(String)).unwrap();
		let GPoll::Final(value) = serve_input(&wired, &ctx, &frames) else {
			panic!("borrow-carrying output must serve through the erased edge");
		};
		// SAFETY: the record was served at `layout`, whose element is the borrow.
		let held = unsafe { read_element::<&String>(layout.rec(&value)) };
		assert_eq!(held, "held");
		assert_eq!(held.len(), 4);
	}

	/// Evaluates its content at three promoted index levels and serves the
	/// collected elements.
	struct RepeatNode<Node0, T> {
		content: Node0,
		inner: Layout,
		layout: Layout,
		_marker: std::marker::PhantomData<fn() -> T>,
	}

	impl<Node0, T: Clone + Send + Sync + dyn_any::StaticTypeSized> RepeatNode<Node0, T>
	where
		Vec<T>: Clone + Send + Sync + dyn_any::StaticTypeSized,
		<Vec<T> as dyn_any::StaticTypeSized>::Static: Clone + Send + Sync,
	{
		fn new(content: Node0, inner: Layout) -> Self {
			Self {
				content,
				inner,
				layout: Layout::default().with_writes(0, element_write::<Vec<T>>(), &[]),
				_marker: std::marker::PhantomData,
			}
		}
	}

	impl<C, T, Node0> Node<C> for RepeatNode<Node0, T>
	where
		C: Ctx + crate::context::DeriveCtx,
		T: Clone + 'static,
		Vec<T>: Send + Sync + dyn_any::StaticTypeSized,
		Node0: for<'x> crate::record::DerivedRecordInput<'x, crate::context::Derived<'x, C>>,
	{
		fn serve<'e, 'l>(&self, input: &C, mut slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let cell = crate::node::StatusCell::new();
			let spilled = input.index_head();
			let mut result = Vec::new();
			for index in 0..3 {
				// The element copies out by value, so the content's frame is
				// dead when the scope ends.
				let scope = slot.frames().scope();
				let derived = input.promoted(&spilled, index);
				match self.content.eval_derived(&cell, 0, &derived, &scope) {
					// SAFETY: the content served at its own layout, whose
					// element is `T`.
					Ok(value) => result.push(unsafe { read_element::<T>(self.inner.rec(&value)) }),
					Err(interrupt) => return interrupt.into(),
				}
			}
			slot.lift_served(cell.finish(result), input.arena())
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	#[test]
	fn derive_ctx_repeat_pushes_index_levels_through_the_erased_edge() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let levels = LiftedSource::<Vec<usize>, _>::new(|input: &ContextImpl| GPoll::Final(input.try_index().map(|levels| levels.collect()).unwrap_or_default()));
		let levels_layout = Node::<ContextImpl>::layout(&levels).clone();
		let inner = RepeatNode::<_, Vec<usize>>::new(levels, levels_layout);
		let inner_layout = Node::<ContextImpl>::layout(&inner).clone();
		let nested = RepeatNode::<_, Vec<Vec<usize>>>::new(inner, inner_layout);
		let layout = Node::<ContextImpl>::layout(&nested).clone();
		let erased: Box<ErasedRecordNode> = Box::new(nested);
		let frames = crate::record::test_frames(1 << 12);

		let GPoll::Final(value) = serve_input(&*erased, &ctx, &frames) else {
			panic!("nested repeat must evaluate");
		};
		// SAFETY: the record was served at `layout`, whose element is the output.
		let outer = unsafe { read_element::<Vec<Vec<Vec<usize>>>>(layout.rec(&value)) };
		assert_eq!(outer.len(), 3);
		assert_eq!(outer[2][1], vec![1, 2, 0]);
		assert_eq!(outer[0][0], vec![0, 0, 0]);
	}

	/// Shifts the footprint's resolution and serves its content's element under
	/// the derived context.
	struct ShiftFootprintNode<Node0> {
		content: Node0,
		inner: Layout,
		layout: Layout,
	}

	impl<Node0> ShiftFootprintNode<Node0> {
		fn new(content: Node0, inner: Layout) -> Self {
			Self {
				content,
				inner,
				layout: Layout::default().with_writes(0, element_write::<u32>(), &[]),
			}
		}
	}

	impl<C, Node0> Node<C> for ShiftFootprintNode<Node0>
	where
		C: Ctx + crate::context::DeriveCtx + crate::context::ExtractFootprint,
		Node0: for<'x> crate::record::DerivedRecordInput<'x, crate::context::Derived<'x, C>>,
	{
		fn serve<'e, 'l>(&self, input: &C, mut slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			use crate::transform::Footprint;
			let cell = crate::node::StatusCell::new();
			let mut footprint = input.try_footprint().copied().unwrap_or(Footprint::DEFAULT);
			footprint.resolution.x += 7;
			// The element copies out by value, so the content's frame is dead
			// when the scope ends.
			let value = {
				let scope = slot.frames().scope();
				let derived = input.with_footprint(&footprint);
				match self.content.eval_derived(&cell, 0, &derived, &scope) {
					// SAFETY: the content served at its own layout, whose
					// element is the resolution.
					Ok(value) => unsafe { read_element::<u32>(self.inner.rec(&value)) },
					Err(interrupt) => return interrupt.into(),
				}
			};
			slot.lift_served(cell.finish(value), input.arena())
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	#[test]
	fn derive_ctx_footprint_replace_reaches_the_content() {
		use crate::context::ExtractFootprint;
		use crate::transform::Footprint;

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let frames = crate::record::test_frames(1 << 12);

		let resolution = LiftedSource::<u32, _>::new(|input: &ContextImpl| GPoll::Final(input.try_footprint().map(|footprint| footprint.resolution.x).unwrap_or(0)));
		let resolution_layout = Node::<ContextImpl>::layout(&resolution).clone();
		let shifted = ShiftFootprintNode::new(resolution, resolution_layout);
		let shifted_layout = Node::<ContextImpl>::layout(&shifted).clone();
		let graph = ShiftFootprintNode::new(shifted, shifted_layout);
		let layout = Node::<ContextImpl>::layout(&graph).clone();

		let GPoll::Final(value) = serve_input(&graph, &ctx, &frames) else {
			panic!("the footprint shift must reach the content");
		};
		// SAFETY: the record was served at `layout`, whose element is the resolution.
		assert_eq!(unsafe { read_element::<u32>(layout.rec(&value)) }, Footprint::DEFAULT.resolution.x + 14);
	}

	#[test]
	fn construct_checks_arity_and_types() {
		fn construct_strlen(args: Vec<EdgeHandle>) -> Result<EdgeHandle, ConstructionError> {
			let mut args = args.into_iter();
			let value = args.next().ok_or(ConstructionError::Arity { expected: 1, got: 0 })?.downcast_record::<String>()?;
			drop(value);
			Ok(crate::value::record_value_edge(0u32))
		}
		let entry = RegistryEntry {
			layout_meta: None,
			io: NodeIOTypes::new(concrete!(Context), record_type::<u32>(), vec![record_edge_type::<String>()]),
			constructor: construct_strlen,
		};

		let owned = crate::value::record_value_edge("typed".to_string());
		assert!(construct(&entry, vec![owned]).is_ok());

		assert_eq!(construct(&entry, vec![]).unwrap_err(), ConstructionError::Arity { expected: 1, got: 0 });

		let mistyped = crate::value::record_value_edge(1.0f64);
		assert_eq!(
			construct(&entry, vec![mistyped]).unwrap_err(),
			ConstructionError::Type {
				expected: Box::new(record_edge_type::<String>()),
				found: Box::new(record_edge_type::<f64>()),
			}
		);
	}

	#[test]
	fn duplicated_edges_share_one_instance_and_outlive_each_other() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let counting = counting();
		let layout = Node::<ContextImpl>::layout(&counting).clone();
		let handle = EdgeHandle::new_record::<u32>(Arc::new(counting) as Arc<ErasedRecordNode>);
		let duplicate = handle.duplicate();
		assert_eq!(*duplicate.ty(), record_edge_type::<u32>());
		let frames = crate::record::test_frames(1 << 12);

		let first = handle.downcast_record::<u32>().unwrap();
		let second = duplicate.downcast_record::<u32>().unwrap();
		// SAFETY: each record was served at `layout`, whose element is the count.
		let count = |value| unsafe { layout.rec(&value).element::<u32>() };
		assert_eq!(serve_input(&first, &ctx, &frames).map(count), GPoll::Final(1));
		assert_eq!(serve_input(&second, &ctx, &frames).map(count), GPoll::Final(2));

		drop(first);
		assert_eq!(serve_input(&second, &ctx, &frames).map(count), GPoll::Final(3));
	}
}
