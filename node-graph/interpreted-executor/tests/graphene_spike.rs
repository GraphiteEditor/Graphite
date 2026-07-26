use std::any::Any;
use std::mem::MaybeUninit;
use std::ops::Add;

use core_types::arena::{Arena, ArenaCell};
use core_types::context::{ContextImpl, Ctx, EvalScope, ExtractArena, InjectIndex};
use core_types::gnode::{BatchStatus, GNode, StatusCell};
use core_types::gpoll::{ErrorKind, Finality, GPoll, Interrupt};

fn add<A: Add<B>, B, C: Ctx>(_ctx: &C, augend: A, addend: B) -> <A as Add<B>>::Output {
	augend + addend
}

struct AddNode<Node0, Node1> {
	augend: Node0,
	addend: Node1,
}

impl<Node0, Node1> AddNode<Node0, Node1> {
	fn new(augend: Node0, addend: Node1) -> Self {
		Self { augend, addend }
	}
}

impl<A, B, Input, Node0, Node1> GNode<Input> for AddNode<Node0, Node1>
where
	A: Add<B>,
	Input: Ctx,
	Node0: GNode<Input, Output = A>,
	Node1: GNode<Input, Output = B>,
{
	type Output = <A as Add<B>>::Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output> {
		let cell = StatusCell::new();
		let augend = match cell.eval_input(0, &self.augend, input) {
			Ok(value) => value,
			Err(interrupt) => return interrupt.into(),
		};
		let addend = match cell.eval_input(1, &self.addend, input) {
			Ok(value) => value,
			Err(interrupt) => return interrupt.into(),
		};
		cell.finish(add(input, augend, addend))
	}
}

struct ValueNode<T>(T);

impl<T: Clone, Input> GNode<Input> for ValueNode<T> {
	type Output = T;

	fn eval(&self, _input: &Input) -> GPoll<T> {
		GPoll::Final(self.0.clone())
	}
}

struct ReadIndexNode;

impl<Input: InjectIndex + Copy + ExtractIndexValue> GNode<Input> for ReadIndexNode {
	type Output = f64;

	fn eval(&self, input: &Input) -> GPoll<f64> {
		GPoll::Final(input.index_value() as f64)
	}
}

trait ExtractIndexValue {
	fn index_value(&self) -> u64;
}

impl ExtractIndexValue for ContextImpl<'_> {
	fn index_value(&self) -> u64 {
		self.index_head().index
	}
}

fn string_length<C: Ctx>(_ctx: &C, value: &String) -> f64 {
	value.len() as f64
}

struct LendStringNode {
	value: String,
	cell: ArenaCell<String>,
}

impl LendStringNode {
	fn new(value: String) -> Self {
		Self {
			value,
			cell: ArenaCell::new(),
		}
	}
}

impl<'e, Input> GNode<Input> for LendStringNode
where
	Input: Ctx + ExtractArena<ArenaRef = &'e Arena>,
{
	type Output = &'e String;

	fn eval(&self, input: &Input) -> GPoll<&'e String> {
		let arena = input.arena();
		if let Some(value) = self.cell.load(arena) {
			return GPoll::Final(value);
		}
		match arena.alloc(self.value.clone()) {
			Some((value, weak)) => {
				self.cell.store(weak);
				GPoll::Final(value)
			}
			None => GPoll::arena_exhausted(),
		}
	}
}

struct StringLengthNode<Node0> {
	value: Node0,
}

impl<Node0> StringLengthNode<Node0> {
	fn new(value: Node0) -> Self {
		Self { value }
	}
}

impl<'e, Input, Node0> GNode<Input> for StringLengthNode<Node0>
where
	Input: Ctx,
	Node0: GNode<Input, Output = &'e String>,
{
	type Output = f64;

	fn eval(&self, input: &Input) -> GPoll<f64> {
		let cell = StatusCell::new();
		let value = match cell.eval_input(0, &self.value, input) {
			Ok(value) => value,
			Err(interrupt) => return interrupt.into(),
		};
		cell.finish(string_length(input, value))
	}
}

type ErasedGNode<T> = dyn for<'c> GNode<ContextImpl<'c>, Output = T>;
type ErasedLendEdge = dyn for<'c> GNode<ContextImpl<'c>, Output = &'c String>;

fn string_length_constructor(args: Vec<Box<dyn Any>>) -> Result<Box<ErasedGNode<f64>>, &'static str> {
	let mut args = args.into_iter();
	let value = *args.next().ok_or("arity")?.downcast::<Box<ErasedLendEdge>>().map_err(|_| "type")?;
	Ok(Box::new(StringLengthNode::new(value)))
}

fn add_constructor_f64(args: Vec<Box<dyn Any>>) -> Result<Box<ErasedGNode<f64>>, &'static str> {
	let mut args = args.into_iter();
	let augend = *args.next().ok_or("arity")?.downcast::<Box<ErasedGNode<f64>>>().map_err(|_| "type")?;
	let addend = *args.next().ok_or("arity")?.downcast::<Box<ErasedGNode<f64>>>().map_err(|_| "type")?;
	Ok(Box::new(AddNode::new(augend, addend)))
}

fn scope_fixture<'a>(generations: &'a [(u64, u64)], arena: &'a Arena) -> EvalScope<'a> {
	EvalScope::new(Some(0.5), None, None, generations, arena)
}

#[test]
fn hand_expansion_evaluates_through_typed_erased_edges() {
	let arena = Arena::new(1024);
	let generations = [];
	let scope = scope_fixture(&generations, &arena);
	let ctx = ContextImpl::root(&scope);

	let augend: Box<dyn Any> = Box::new(Box::new(ValueNode(1.0f64)) as Box<ErasedGNode<f64>>);
	let addend: Box<dyn Any> = Box::new(Box::new(ValueNode(2.0f64)) as Box<ErasedGNode<f64>>);
	let wired = add_constructor_f64(vec![augend, addend]).unwrap();

	assert_eq!(wired.eval(&ctx), GPoll::Final(3.0));
}

#[test]
fn wiring_rejects_type_and_arity_mismatches() {
	let augend: Box<dyn Any> = Box::new(Box::new(ValueNode(1.0f64)) as Box<ErasedGNode<f64>>);
	let addend: Box<dyn Any> = Box::new(Box::new(ValueNode(2u32)) as Box<ErasedGNode<u32>>);
	assert_eq!(add_constructor_f64(vec![augend, addend]).map(|_| ()), Err("type"));

	let augend: Box<dyn Any> = Box::new(Box::new(ValueNode(1.0f64)) as Box<ErasedGNode<f64>>);
	assert_eq!(add_constructor_f64(vec![augend]).map(|_| ()), Err("arity"));
}

#[test]
fn spec_loop_batches_through_the_erased_edge() {
	let arena = Arena::new(1024);
	let generations = [];
	let scope = scope_fixture(&generations, &arena);
	let ctx = ContextImpl::root(&scope);

	let graph: Box<ErasedGNode<f64>> = Box::new(AddNode::new(ReadIndexNode, ValueNode(10.0f64)));
	let mut scratch = [const { MaybeUninit::uninit() }; 4];
	let status = graph.eval_batch(&ctx, 2..6, Some(&mut scratch));
	let BatchStatus::Filled(lanes, finality) = status else {
		panic!("expected filled, got {status:?}");
	};
	assert_eq!(lanes, &[12.0, 13.0, 14.0, 15.0]);
	assert_eq!(finality, Finality::AllFinal);
}

#[test]
fn lending_kernel_clones_once_per_generation_and_lends_after() {
	let arena = Arena::new(1024);
	let generations = [];
	let scope = scope_fixture(&generations, &arena);
	let ctx = ContextImpl::root(&scope);

	let node = LendStringNode::new("lend me".to_string());
	let GPoll::Final(first) = node.eval(&ctx) else {
		panic!("first eval must clone into the arena and lend");
	};
	let GPoll::Final(second) = node.eval(&ctx) else {
		panic!("second eval must hit the cell");
	};
	assert_eq!(first, "lend me");
	assert!(std::ptr::eq(first, second));
}

#[test]
fn exhausted_arena_reports_the_operational_error() {
	let arena = Arena::new(0);
	let generations = [];
	let scope = scope_fixture(&generations, &arena);
	let ctx = ContextImpl::root(&scope);

	let node = LendStringNode::new("too big".to_string());
	let GPoll::Error(error) = node.eval(&ctx) else {
		panic!("exhaustion must surface as an operational error");
	};
	assert_eq!(error.kind, ErrorKind::ArenaExhausted);
}

#[test]
fn lending_edges_erase_and_wire_like_owned_edges() {
	let arena = Arena::new(1024);
	let generations = [];
	let scope = scope_fixture(&generations, &arena);
	let ctx = ContextImpl::root(&scope);

	let value: Box<dyn Any> = Box::new(Box::new(LendStringNode::new("across the boundary".to_string())) as Box<ErasedLendEdge>);
	let wired = string_length_constructor(vec![value]).unwrap();

	assert_eq!(wired.eval(&ctx), GPoll::Final(19.0));
}

#[test]
fn spec_loop_batches_through_the_erased_lending_edge() {
	let arena = Arena::new(1024);
	let generations = [];
	let scope = scope_fixture(&generations, &arena);
	let ctx = ContextImpl::root(&scope);

	let graph: Box<ErasedLendEdge> = Box::new(LendStringNode::new("batched".to_string()));
	let mut scratch = [const { MaybeUninit::uninit() }; 3];
	let status = graph.eval_batch(&ctx, 0..3, Some(&mut scratch));
	let BatchStatus::Filled(lanes, finality) = status else {
		panic!("expected filled, got {status:?}");
	};
	assert_eq!(lanes.len(), 3);
	assert!(lanes.iter().all(|lane| std::ptr::eq(*lane, lanes[0])));
	assert_eq!(*lanes[0], "batched");
	assert_eq!(finality, Finality::AllFinal);
}

#[test]
fn fallback_input_records_partiality_invisibly() {
	struct FallbackNode;
	impl<Input> GNode<Input> for FallbackNode {
		type Output = f64;
		fn eval(&self, _input: &Input) -> GPoll<f64> {
			GPoll::fallback(0.0, "upstream failed")
		}
	}

	let arena = Arena::new(1024);
	let generations = [];
	let scope = scope_fixture(&generations, &arena);
	let ctx = ContextImpl::root(&scope);

	let graph = AddNode::new(FallbackNode, ValueNode(5.0f64));
	let GPoll::Fallback(boxed) = graph.eval(&ctx) else {
		panic!("fallback must propagate with the computed stand-in");
	};
	assert_eq!(boxed.0, 5.0);
	assert!(boxed.1.kind == "upstream failed");
	assert_eq!(boxed.1.trace, vec![0]);
}
