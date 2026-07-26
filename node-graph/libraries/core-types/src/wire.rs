use crate::Type;
use crate::arena::{Arena, ArenaCell};
use crate::concrete;
use crate::context::{ContextImpl, Ctx, ExtractArena};
use crate::frame_table::{FrameTable, Lookup};
use crate::gnode::GNode;
use crate::gpoll::{Extent, Finality, GPoll};
use graphene_hash::CacheHash;
use std::any::Any;
use std::hash::Hasher;
use std::sync::Mutex;

pub type ErasedGNode<T> = dyn for<'c> GNode<ContextImpl<'c>, Output = T>;
pub type ErasedLendGNode<T> = dyn for<'c> GNode<ContextImpl<'c>, Output = &'c T>;

pub fn cache_key<C: CacheHash + ?Sized>(ctx: &C) -> u64 {
	let mut hasher = std::hash::DefaultHasher::new();
	ctx.cache_hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug, PartialEq)]
pub enum WireError {
	Arity { expected: usize, got: usize },
	Type { expected: Type, found: Type },
	MissingCapability { ty: Type },
}

#[derive(Clone, Copy, Default)]
pub struct WireCapabilities {
	pub memoize: Option<fn(EdgeHandle) -> Result<EdgeHandle, WireError>>,
	pub lend: Option<fn(EdgeHandle) -> Result<EdgeHandle, WireError>>,
}

fn memoize_edge<T: Clone + 'static>(edge: EdgeHandle) -> Result<EdgeHandle, WireError> {
	let content = edge.downcast::<T>()?;
	Ok(EdgeHandle::new(Box::new(MemoizeNode::new(content)) as Box<ErasedGNode<T>>))
}

fn lend_edge<T: Clone + 'static>(edge: EdgeHandle) -> Result<EdgeHandle, WireError> {
	let content = edge.downcast::<T>()?;
	Ok(EdgeHandle::new_ref(Box::new(FrameMemoNode::new(content)) as Box<ErasedLendGNode<T>>))
}

pub struct EdgeHandle {
	node: Box<dyn Any>,
	ty: Type,
	capabilities: WireCapabilities,
}

impl std::fmt::Debug for EdgeHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("EdgeHandle").field("ty", &self.ty).finish_non_exhaustive()
	}
}

impl EdgeHandle {
	pub fn new<T: Clone + 'static>(node: Box<ErasedGNode<T>>) -> Self {
		Self::new_erased(
			node,
			concrete!(T),
			WireCapabilities {
				memoize: Some(memoize_edge::<T>),
				lend: Some(lend_edge::<T>),
			},
		)
	}

	pub fn new_ref<T: 'static>(node: Box<ErasedLendGNode<T>>) -> Self {
		Self::new_erased(node, Type::Ref(Box::new(concrete!(T))), WireCapabilities::default())
	}

	pub fn new_erased<N: ?Sized>(node: Box<N>, ty: Type, capabilities: WireCapabilities) -> Self
	where
		Box<N>: Any,
	{
		Self {
			node: Box::new(node),
			ty,
			capabilities,
		}
	}

	pub fn wire_type(&self) -> &Type {
		&self.ty
	}

	pub fn memoized(self) -> Result<EdgeHandle, WireError> {
		match self.capabilities.memoize {
			Some(wrap) => wrap(self),
			None => Err(WireError::MissingCapability { ty: self.ty }),
		}
	}

	pub fn lent(self) -> Result<EdgeHandle, WireError> {
		match self.capabilities.lend {
			Some(wrap) => wrap(self),
			None => Err(WireError::MissingCapability { ty: self.ty }),
		}
	}

	pub fn downcast<T: 'static>(self) -> Result<Box<ErasedGNode<T>>, WireError> {
		self.downcast_erased(concrete!(T))
	}

	pub fn downcast_lend<T: 'static>(self) -> Result<Box<ErasedLendGNode<T>>, WireError> {
		self.downcast_erased(Type::Ref(Box::new(concrete!(T))))
	}

	pub fn downcast_erased<N: ?Sized>(self, expected: Type) -> Result<Box<N>, WireError>
	where
		Box<N>: Any,
	{
		let found = self.ty;
		self.node.downcast::<Box<N>>().map(|node| *node).map_err(|_| WireError::Type { expected, found })
	}
}

pub struct NodeIoRecord {
	pub inputs: Vec<Type>,
	pub output: Type,
}

pub struct RegistryEntry {
	pub io: NodeIoRecord,
	pub wire: fn(Vec<EdgeHandle>) -> Result<EdgeHandle, WireError>,
}

pub fn resolve_and_wire(entry: &RegistryEntry, inputs: Vec<EdgeHandle>) -> Result<EdgeHandle, WireError> {
	if inputs.len() != entry.io.inputs.len() {
		return Err(WireError::Arity {
			expected: entry.io.inputs.len(),
			got: inputs.len(),
		});
	}
	for (handle, expected) in inputs.iter().zip(&entry.io.inputs) {
		if handle.wire_type() != expected {
			return Err(WireError::Type {
				expected: expected.clone(),
				found: handle.wire_type().clone(),
			});
		}
	}
	(entry.wire)(inputs)
}

pub struct MemoizeNode<T, NodeContent> {
	cache: Mutex<Option<(u64, T, Finality)>>,
	content: NodeContent,
}

impl<T, NodeContent> MemoizeNode<T, NodeContent> {
	pub fn new(content: NodeContent) -> Self {
		Self {
			cache: Mutex::new(None),
			content,
		}
	}
}

impl<T, Input, NodeContent> GNode<Input> for MemoizeNode<T, NodeContent>
where
	T: Clone,
	Input: Ctx + CacheHash,
	NodeContent: GNode<Input, Output = T>,
{
	type Output = T;

	fn eval(&self, input: &Input) -> GPoll<T> {
		let key = cache_key(input);
		if let Some((hash, value, finality)) = self.cache.lock().unwrap().as_ref() {
			if *hash == key {
				return match finality {
					Finality::AllFinal => GPoll::Final(value.clone()),
					Finality::Partial => GPoll::Partial(value.clone()),
				};
			}
		}
		let result = self.content.eval(input);
		match &result {
			GPoll::Final(value) => *self.cache.lock().unwrap() = Some((key, value.clone(), Finality::AllFinal)),
			GPoll::Partial(value) => *self.cache.lock().unwrap() = Some((key, value.clone(), Finality::Partial)),
			GPoll::Pending | GPoll::Fallback(_) | GPoll::Error(_) => {}
		}
		result
	}

	fn extent(&self, input: &Input) -> GPoll<Extent> {
		self.content.extent(input)
	}
}

pub struct FrameMemoNode<T, NodeContent> {
	cell: ArenaCell<FrameTable<T, 32>>,
	content: NodeContent,
}

impl<T, NodeContent> FrameMemoNode<T, NodeContent> {
	pub fn new(content: NodeContent) -> Self {
		Self {
			cell: ArenaCell::new(),
			content,
		}
	}
}

impl<'e, T, Input, NodeContent> GNode<Input> for FrameMemoNode<T, NodeContent>
where
	T: Clone + 'static,
	Input: Ctx + CacheHash + ExtractArena<ArenaRef = &'e Arena>,
	NodeContent: GNode<Input, Output = T>,
{
	type Output = &'e T;

	fn eval(&self, input: &Input) -> GPoll<&'e T> {
		let arena = input.arena();
		let table = match self.cell.load(arena) {
			Some(table) => table,
			None => match arena.alloc(FrameTable::new()) {
				Some((table, weak)) => {
					self.cell.store(weak);
					table
				}
				None => return park(arena, self.content.eval(input)),
			},
		};
		match table.lookup(cache_key(input)) {
			Lookup::Hit(Finality::AllFinal, value) => GPoll::Final(value),
			Lookup::Hit(Finality::Partial, value) => GPoll::Partial(value),
			Lookup::Vacant(slot) => match self.content.eval(input) {
				GPoll::Final(value) => GPoll::Final(slot.publish(value, Finality::AllFinal)),
				GPoll::Partial(value) => GPoll::Partial(slot.publish(value, Finality::Partial)),
				unpublishable => {
					slot.release();
					park(arena, unpublishable)
				}
			},
			Lookup::Full => park(arena, self.content.eval(input)),
		}
	}

	fn extent(&self, input: &Input) -> GPoll<Extent> {
		self.content.extent(input)
	}
}

pub fn park<'e, T>(arena: &'e Arena, result: GPoll<T>) -> GPoll<&'e T> {
	match result {
		GPoll::Final(value) => match arena.alloc(value) {
			Some((parked, _)) => GPoll::Final(parked),
			None => GPoll::arena_exhausted(),
		},
		GPoll::Partial(value) => match arena.alloc(value) {
			Some((parked, _)) => GPoll::Partial(parked),
			None => GPoll::arena_exhausted(),
		},
		GPoll::Fallback(boxed) => {
			let (value, error) = *boxed;
			match arena.alloc(value) {
				Some((parked, _)) => GPoll::Fallback(Box::new((parked, error))),
				None => GPoll::arena_exhausted(),
			}
		}
		GPoll::Pending => GPoll::Pending,
		GPoll::Error(error) => GPoll::Error(error),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::context::EvalScope;
	use crate::SourceId;
	use std::sync::atomic::{AtomicU32, Ordering};

	struct CountingNode(AtomicU32);

	impl<Input> GNode<Input> for CountingNode {
		type Output = u32;

		fn eval(&self, _input: &Input) -> GPoll<u32> {
			GPoll::Final(self.0.fetch_add(1, Ordering::Relaxed) + 1)
		}
	}

	struct ValueNode<T>(T);

	impl<T: Clone, Input> GNode<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	#[test]
	fn memo_capability_wraps_edges_type_blind() {
		let arena = Arena::new(1024);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let edge = EdgeHandle::new(Box::new(CountingNode(AtomicU32::new(0))) as Box<ErasedGNode<u32>>);
		let memoized = edge.memoized().unwrap().downcast::<u32>().unwrap();

		assert_eq!(memoized.eval(&ctx), GPoll::Final(1));
		assert_eq!(memoized.eval(&ctx), GPoll::Final(1));
	}

	#[test]
	fn memo_invalidates_on_generation_bump() {
		let arena = Arena::new(1024);
		let source: SourceId = 7;
		let before = [(source, 1)];
		let after = [(source, 2)];
		let scope_before = scope_fixture(&before, &arena);
		let scope_after = scope_fixture(&after, &arena);

		let edge = EdgeHandle::new(Box::new(CountingNode(AtomicU32::new(0))) as Box<ErasedGNode<u32>>);
		let memoized = edge.memoized().unwrap().downcast::<u32>().unwrap();

		assert_eq!(memoized.eval(&ContextImpl::root(&scope_before)), GPoll::Final(1));
		assert_eq!(memoized.eval(&ContextImpl::root(&scope_before)), GPoll::Final(1));
		assert_eq!(memoized.eval(&ContextImpl::root(&scope_after)), GPoll::Final(2));
	}

	#[test]
	fn memoized_edges_stack_and_rewire() {
		let arena = Arena::new(1024);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let edge = EdgeHandle::new(Box::new(CountingNode(AtomicU32::new(0))) as Box<ErasedGNode<u32>>);
		let stacked = edge.memoized().unwrap().memoized().unwrap().downcast::<u32>().unwrap();

		assert_eq!(stacked.eval(&ctx), GPoll::Final(1));
		assert_eq!(stacked.eval(&ctx), GPoll::Final(1));
	}

	#[test]
	fn lend_capability_turns_an_owned_edge_into_a_lending_edge() {
		let arena = Arena::new(4096);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let edge = EdgeHandle::new(Box::new(ValueNode("lent out".to_string())) as Box<ErasedGNode<String>>);
		let lending = edge.lent().unwrap();
		assert_eq!(*lending.wire_type(), Type::Ref(Box::new(concrete!(String))));

		let node = lending.downcast_lend::<String>().unwrap();
		let GPoll::Final(first) = node.eval(&ctx) else {
			panic!("lend must fill the frame table and lend");
		};
		let GPoll::Final(second) = node.eval(&ctx) else {
			panic!("second eval must lend the published value");
		};
		assert_eq!(first, "lent out");
		assert!(std::ptr::eq(first, second));
	}

	#[test]
	fn ref_edges_report_missing_capabilities() {
		let edge = EdgeHandle::new(Box::new(ValueNode(5u32)) as Box<ErasedGNode<u32>>);
		let lending = edge.lent().unwrap();

		match lending.memoized() {
			Err(WireError::MissingCapability { ty }) => assert_eq!(ty, Type::Ref(Box::new(concrete!(u32)))),
			other => panic!("expected missing capability, got {:?}", other.map(|handle| handle.ty)),
		}
	}

	#[test]
	fn borrow_carrying_value_types_wire_through_the_general_constructor() {
		struct SplitBorrow<'c>(&'c str, usize);

		struct SplitNode<Node0> {
			content: Node0,
		}

		impl<'e, Input, Node0> GNode<Input> for SplitNode<Node0>
		where
			Input: Ctx,
			Node0: GNode<Input, Output = &'e String>,
		{
			type Output = SplitBorrow<'e>;

			fn eval(&self, input: &Input) -> GPoll<SplitBorrow<'e>> {
				self.content.eval(input).map(|value| SplitBorrow(value, value.len()))
			}
		}

		type ErasedSplitEdge = dyn for<'c> GNode<ContextImpl<'c>, Output = SplitBorrow<'c>>;

		let arena = Arena::new(4096);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let lending = EdgeHandle::new(Box::new(ValueNode("held".to_string())) as Box<ErasedGNode<String>>).lent().unwrap();
		let upstream = lending.downcast_lend::<String>().unwrap();
		let node: Box<ErasedSplitEdge> = Box::new(SplitNode { content: upstream });
		let handle = EdgeHandle::new_erased(node, concrete!(SplitBorrow<'static>), WireCapabilities::default());
		assert_eq!(*handle.wire_type(), concrete!(SplitBorrow<'static>));

		let wired = handle.downcast_erased::<ErasedSplitEdge>(concrete!(SplitBorrow<'static>)).unwrap();
		let GPoll::Final(split) = wired.eval(&ctx) else {
			panic!("borrow-carrying output must eval through the erased edge");
		};
		assert_eq!(split.0, "held");
		assert_eq!(split.1, 4);
	}

	#[test]
	fn derive_ctx_repeat_pushes_index_levels_through_the_erased_edge() {
		use crate::context::{Derived, DeriveCtx, ExtractIndex};

		struct RepeatNode<Node0> {
			content: Node0,
		}

		impl<C, T, Node0> GNode<C> for RepeatNode<Node0>
		where
			C: Ctx + DeriveCtx,
			Node0: for<'x> GNode<Derived<'x, C>, Output = T>,
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

		impl<Input: ExtractIndex> GNode<Input> for LevelsNode {
			type Output = Vec<usize>;

			fn eval(&self, input: &Input) -> GPoll<Vec<usize>> {
				GPoll::Final(input.try_index().map(|levels| levels.collect()).unwrap_or_default())
			}
		}

		let arena = Arena::new(1024);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let nested = RepeatNode {
			content: RepeatNode { content: LevelsNode },
		};
		let erased: Box<ErasedGNode<Vec<Vec<Vec<usize>>>>> = Box::new(nested);

		let GPoll::Final(outer) = erased.eval(&ctx) else {
			panic!("nested repeat must evaluate");
		};
		assert_eq!(outer.len(), 3);
		assert_eq!(outer[2][1], vec![1, 2, 0]);
		assert_eq!(outer[0][0], vec![0, 0, 0]);
	}

	#[test]
	fn derive_ctx_footprint_replace_reaches_the_content() {
		use crate::context::{Derived, DeriveCtx, ExtractFootprint};
		use crate::transform::Footprint;

		struct ShiftFootprintNode<Node0> {
			content: Node0,
		}

		impl<C, T, Node0> GNode<C> for ShiftFootprintNode<Node0>
		where
			C: Ctx + DeriveCtx + ExtractFootprint,
			Node0: for<'x> GNode<Derived<'x, C>, Output = T>,
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

		impl<Input: ExtractFootprint> GNode<Input> for ResolutionNode {
			type Output = u32;

			fn eval(&self, input: &Input) -> GPoll<u32> {
				GPoll::Final(input.try_footprint().map(|footprint| footprint.resolution.x).unwrap_or(0))
			}
		}

		let arena = Arena::new(1024);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let graph: Box<ErasedGNode<u32>> = Box::new(ShiftFootprintNode {
			content: ShiftFootprintNode { content: ResolutionNode },
		});
		assert_eq!(graph.eval(&ctx), GPoll::Final(Footprint::DEFAULT.resolution.x + 14));
	}

	#[test]
	fn resolve_and_wire_checks_arity_and_types() {
		fn wire_strlen(args: Vec<EdgeHandle>) -> Result<EdgeHandle, WireError> {
			let mut args = args.into_iter();
			let value = args.next().ok_or(WireError::Arity { expected: 1, got: 0 })?.downcast::<String>()?;
			drop(value);
			Ok(EdgeHandle::new(Box::new(ValueNode(0u32)) as Box<ErasedGNode<u32>>))
		}
		let entry = RegistryEntry {
			io: NodeIoRecord {
				inputs: vec![concrete!(String)],
				output: concrete!(u32),
			},
			wire: wire_strlen,
		};

		let owned = EdgeHandle::new(Box::new(ValueNode("typed".to_string())) as Box<ErasedGNode<String>>);
		assert!(resolve_and_wire(&entry, vec![owned]).is_ok());

		assert_eq!(resolve_and_wire(&entry, vec![]).unwrap_err(), WireError::Arity { expected: 1, got: 0 });

		let mistyped = EdgeHandle::new(Box::new(ValueNode(1.0f64)) as Box<ErasedGNode<f64>>);
		assert_eq!(
			resolve_and_wire(&entry, vec![mistyped]).unwrap_err(),
			WireError::Type {
				expected: concrete!(String),
				found: concrete!(f64),
			}
		);

		let lent = EdgeHandle::new(Box::new(ValueNode("typed".to_string())) as Box<ErasedGNode<String>>).lent().unwrap();
		assert_eq!(
			resolve_and_wire(&entry, vec![lent]).unwrap_err(),
			WireError::Type {
				expected: concrete!(String),
				found: Type::Ref(Box::new(concrete!(String))),
			}
		);
	}
}
