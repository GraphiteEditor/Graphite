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

#[cfg(not(target_family = "wasm"))]
pub type ErasedNode<T> = dyn for<'c> Node<ContextImpl<'c>, Output = T> + Send + Sync;
#[cfg(target_family = "wasm")]
pub type ErasedNode<T> = dyn for<'c> Node<ContextImpl<'c>, Output = T>;
#[cfg(not(target_family = "wasm"))]
pub type ErasedLendNode<T> = dyn for<'c> Node<ContextImpl<'c>, Output = &'c T> + Send + Sync;
#[cfg(target_family = "wasm")]
pub type ErasedLendNode<T> = dyn for<'c> Node<ContextImpl<'c>, Output = &'c T>;

/// Element-independent by erasure; the wire's `Type::Record(El)` keeps element reads proven at wiring.
#[cfg(not(target_family = "wasm"))]
pub type ErasedRecordNode = dyn for<'c> Node<ContextImpl<'c>, Output = crate::record::RecordValue<'c>> + Send + Sync;
#[cfg(target_family = "wasm")]
pub type ErasedRecordNode = dyn for<'c> Node<ContextImpl<'c>, Output = crate::record::RecordValue<'c>>;

#[cfg(not(target_family = "wasm"))]
type DynEdge = dyn std::any::Any + Send + Sync;
#[cfg(target_family = "wasm")]
type DynEdge = dyn std::any::Any;

pub fn edge_type<T: 'static>() -> Type {
	Type::Fn(Box::new(concrete!(Context)), Box::new(concrete!(T)))
}

pub fn ref_type<T: 'static>() -> Type {
	Type::Ref(Box::new(concrete!(T)))
}

pub fn lend_edge_type<T: 'static>() -> Type {
	Type::Fn(Box::new(concrete!(Context)), Box::new(ref_type::<T>()))
}

pub fn record_type<T: 'static>() -> Type {
	Type::Record(Box::new(concrete!(T)))
}

pub fn record_edge_type<T: 'static>() -> Type {
	Type::Fn(Box::new(concrete!(Context)), Box::new(record_type::<T>()))
}

/// The record edge type of a token row, generic over the element.
pub fn generic_record_edge_type(name: &'static str) -> Type {
	Type::Fn(
		Box::new(concrete!(Context)),
		Box::new(Type::Record(Box::new(Type::Generic(std::borrow::Cow::Borrowed(name))))),
	)
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
	MissingLayout,
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
	type Output = N::Output;

	fn eval(&self, input: &Input) -> crate::gpoll::GPoll<Self::Output> {
		// SAFETY: `own` keeps the payload alive for `self`'s lifetime and Arc
		// payloads are address stable.
		unsafe { self.ptr.as_ref() }.eval(input)
	}

	fn extent(&self, input: &Input) -> crate::gpoll::GPoll<crate::gpoll::Extent> {
		// SAFETY: as in eval.
		unsafe { self.ptr.as_ref() }.extent(input)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		// SAFETY: as in eval.
		unsafe { self.ptr.as_ref() }.serialize()
	}

	fn layout(&self) -> Option<&crate::record::Layout> {
		// SAFETY: as in eval.
		unsafe { self.ptr.as_ref() }.layout()
	}

	fn eval_batch<'a>(&self, input: &'a Input, range: std::ops::Range<u64>, scratch: Option<&'a mut [std::mem::MaybeUninit<Self::Output>]>) -> crate::node::BatchStatus<'a, Self::Output>
	where
		Input: crate::context::InjectIndex + Copy,
	{
		// SAFETY: as in eval.
		unsafe { self.ptr.as_ref() }.eval_batch(input, range, scratch)
	}
}

pub struct EdgeHandle {
	node: Box<DynEdge>,
	share: fn(&DynEdge) -> Box<DynEdge>,
	serialize: fn(&DynEdge) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
	layout: fn(&DynEdge) -> Option<&crate::record::Layout>,
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
	pub fn new<T: 'static>(node: std::sync::Arc<ErasedNode<T>>) -> Self {
		Self::new_erased(node, edge_type::<T>())
	}

	pub fn new_ref<T: 'static>(node: std::sync::Arc<ErasedLendNode<T>>) -> Self {
		Self::new_erased(node, lend_edge_type::<T>())
	}

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
			ty: self.ty.clone(),
		}
	}

	pub fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(self.serialize)(&*self.node)
	}

	pub fn layout(&self) -> Option<&crate::record::Layout> {
		(self.layout)(&*self.node)
	}

	pub fn downcast<T: 'static>(self) -> Result<SharedEdge<ErasedNode<T>>, ConstructionError> {
		self.downcast_erased(edge_type::<T>())
	}

	pub fn downcast_lend<T: 'static>(self) -> Result<SharedEdge<ErasedLendNode<T>>, ConstructionError> {
		self.downcast_erased(lend_edge_type::<T>())
	}

	pub fn downcast_record<T: 'static>(self) -> Result<SharedEdge<ErasedRecordNode>, ConstructionError> {
		self.downcast_erased(record_edge_type::<T>())
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
	use crate::context::{Ctx, EvalScope, ExtractArena};
	use crate::gpoll::GPoll;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicU32, Ordering};

	struct CountingNode(AtomicU32);

	impl<Input> Node<Input> for CountingNode {
		type Output = u32;

		fn eval(&self, _input: &Input) -> GPoll<u32> {
			GPoll::Final(self.0.fetch_add(1, Ordering::Relaxed) + 1)
		}
	}

	struct ValueNode<T>(T);

	impl<T: Clone, Input> Node<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	struct LendNode(String);

	impl<'e, Input: Ctx + ExtractArena<ArenaRef = &'e Arena>> Node<Input> for LendNode {
		type Output = &'e String;

		fn eval(&self, input: &Input) -> GPoll<&'e String> {
			match input.arena().alloc(self.0.clone()) {
				Some((parked, _)) => GPoll::Final(parked),
				None => GPoll::arena_exhausted(),
			}
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	#[test]
	fn borrow_carrying_value_types_wire_through_the_general_constructor() {
		struct SplitBorrow<'c>(&'c str, usize);

		struct SplitNode<Node0> {
			content: Node0,
		}

		impl<'e, Input, Node0> Node<Input> for SplitNode<Node0>
		where
			Input: Ctx,
			Node0: Node<Input, Output = &'e String>,
		{
			type Output = SplitBorrow<'e>;

			fn eval(&self, input: &Input) -> GPoll<SplitBorrow<'e>> {
				self.content.eval(input).map(|value| SplitBorrow(value, value.len()))
			}
		}

		type ErasedSplitEdge = dyn for<'c> Node<ContextImpl<'c>, Output = SplitBorrow<'c>> + Send + Sync;

		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let lending = EdgeHandle::new_ref(Arc::new(LendNode("held".to_string())) as Arc<ErasedLendNode<String>>);
		let upstream = lending.downcast_lend::<String>().unwrap();
		let node: Arc<ErasedSplitEdge> = Arc::new(SplitNode { content: upstream });
		let handle = EdgeHandle::new_erased(node, concrete!(SplitBorrow<'static>));
		assert_eq!(*handle.ty(), concrete!(SplitBorrow<'static>));

		let wired = handle.downcast_erased::<ErasedSplitEdge>(concrete!(SplitBorrow<'static>)).unwrap();
		let GPoll::Final(split) = wired.eval(&ctx) else {
			panic!("borrow-carrying output must eval through the erased edge");
		};
		assert_eq!(split.0, "held");
		assert_eq!(split.1, 4);
	}

	#[test]
	fn derive_ctx_repeat_pushes_index_levels_through_the_erased_edge() {
		use crate::context::{DeriveCtx, Derived, ExtractIndex};

		struct RepeatNode<Node0> {
			content: Node0,
		}

		impl<C, T, Node0> Node<C> for RepeatNode<Node0>
		where
			C: Ctx + DeriveCtx,
			Node0: for<'x> Node<Derived<'x, C>, Output = T>,
		{
			type Output = Vec<T>;

			fn eval(&self, input: &C) -> GPoll<Vec<T>> {
				let spilled = input.index_head();
				let mut result = Vec::new();
				for index in 0..3 {
					let derived = input.promoted(&spilled, index);
					match self.content.eval(&derived) {
						GPoll::Final(value) => result.push(value),
						other => return other.map(|_| Vec::new()),
					}
				}
				GPoll::Final(result)
			}
		}

		struct LevelsNode;

		impl<Input: ExtractIndex> Node<Input> for LevelsNode {
			type Output = Vec<usize>;

			fn eval(&self, input: &Input) -> GPoll<Vec<usize>> {
				GPoll::Final(input.try_index().map(|levels| levels.collect()).unwrap_or_default())
			}
		}

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let nested = RepeatNode {
			content: RepeatNode { content: LevelsNode },
		};
		let erased: Box<ErasedNode<Vec<Vec<Vec<usize>>>>> = Box::new(nested);

		let GPoll::Final(outer) = erased.eval(&ctx) else {
			panic!("nested repeat must evaluate");
		};
		assert_eq!(outer.len(), 3);
		assert_eq!(outer[2][1], vec![1, 2, 0]);
		assert_eq!(outer[0][0], vec![0, 0, 0]);
	}

	#[test]
	fn derive_ctx_footprint_replace_reaches_the_content() {
		use crate::context::{DeriveCtx, Derived, ExtractFootprint};
		use crate::transform::Footprint;

		struct ShiftFootprintNode<Node0> {
			content: Node0,
		}

		impl<C, T, Node0> Node<C> for ShiftFootprintNode<Node0>
		where
			C: Ctx + DeriveCtx + ExtractFootprint,
			Node0: for<'x> Node<Derived<'x, C>, Output = T>,
		{
			type Output = T;

			fn eval(&self, input: &C) -> GPoll<T> {
				let mut footprint = input.try_footprint().copied().unwrap_or(Footprint::DEFAULT);
				footprint.resolution.x += 7;
				let derived = input.with_footprint(&footprint);
				self.content.eval(&derived)
			}
		}

		struct ResolutionNode;

		impl<Input: ExtractFootprint> Node<Input> for ResolutionNode {
			type Output = u32;

			fn eval(&self, input: &Input) -> GPoll<u32> {
				GPoll::Final(input.try_footprint().map(|footprint| footprint.resolution.x).unwrap_or(0))
			}
		}

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let graph: Box<ErasedNode<u32>> = Box::new(ShiftFootprintNode {
			content: ShiftFootprintNode { content: ResolutionNode },
		});
		assert_eq!(graph.eval(&ctx), GPoll::Final(Footprint::DEFAULT.resolution.x + 14));
	}

	#[test]
	fn construct_checks_arity_and_types() {
		fn construct_strlen(args: Vec<EdgeHandle>) -> Result<EdgeHandle, ConstructionError> {
			let mut args = args.into_iter();
			let value = args.next().ok_or(ConstructionError::Arity { expected: 1, got: 0 })?.downcast::<String>()?;
			drop(value);
			Ok(EdgeHandle::new(Arc::new(ValueNode(0u32)) as Arc<ErasedNode<u32>>))
		}
		let entry = RegistryEntry {
			io: NodeIOTypes::new(concrete!(Context), concrete!(u32), vec![edge_type::<String>()]),
			constructor: construct_strlen,
		};

		let owned = EdgeHandle::new(Arc::new(ValueNode("typed".to_string())) as Arc<ErasedNode<String>>);
		assert!(construct(&entry, vec![owned]).is_ok());

		assert_eq!(construct(&entry, vec![]).unwrap_err(), ConstructionError::Arity { expected: 1, got: 0 });

		let mistyped = EdgeHandle::new(Arc::new(ValueNode(1.0f64)) as Arc<ErasedNode<f64>>);
		assert_eq!(
			construct(&entry, vec![mistyped]).unwrap_err(),
			ConstructionError::Type {
				expected: Box::new(edge_type::<String>()),
				found: Box::new(edge_type::<f64>()),
			}
		);

		let lent = EdgeHandle::new_ref(Arc::new(LendNode("typed".to_string())) as Arc<ErasedLendNode<String>>);
		assert_eq!(
			construct(&entry, vec![lent]).unwrap_err(),
			ConstructionError::Type {
				expected: Box::new(edge_type::<String>()),
				found: Box::new(lend_edge_type::<String>()),
			}
		);
	}

	#[test]
	fn duplicated_edges_share_one_instance_and_outlive_each_other() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let handle = EdgeHandle::new(Arc::new(CountingNode(AtomicU32::new(0))) as Arc<ErasedNode<u32>>);
		let duplicate = handle.duplicate();
		assert_eq!(*duplicate.ty(), edge_type::<u32>());

		let first = handle.downcast::<u32>().unwrap();
		let second = duplicate.downcast::<u32>().unwrap();
		assert_eq!(first.eval(&ctx), GPoll::Final(1));
		assert_eq!(second.eval(&ctx), GPoll::Final(2));

		drop(first);
		assert_eq!(second.eval(&ctx), GPoll::Final(3));
	}
}
