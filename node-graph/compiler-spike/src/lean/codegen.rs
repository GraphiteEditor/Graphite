//! A structural Rust codegen backend: one `let` per reachable IR node in IR order, against stub node and type definitions
//! whose trait bounds make rustc check that every node's inputs have the types its selected implementation expects.
//!
//! Each stub node hashes its inputs' payloads with the context, so the emitted program also measures evaluation overhead.

use super::Lean;
use super::document::IdentifierId;
use super::types::{TypeData, TypeId};
use rustc_hash::FxHashMap;
use std::fmt::Write;

const MONOMORPHIC_RECURSION_LIMIT: u32 = 4096;

const PRELUDE: &str = "#![allow(dead_code, non_camel_case_types, unused_mut, unused_variables, clippy::all)]

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

pub type EvalContext = u64;

pub trait Payload {
	fn payload(&self) -> u64;
	fn from_payload(payload: u64) -> Self;
}

pub struct Item<T>(T);
pub struct List<T>(T);
pub struct Function<I, O>(O, PhantomData<I>);

impl<T: Payload> Payload for Item<T> {
	fn payload(&self) -> u64 {
		self.0.payload()
	}
	fn from_payload(payload: u64) -> Self {
		Self(T::from_payload(payload))
	}
}

impl<T: Payload> Payload for List<T> {
	fn payload(&self) -> u64 {
		self.0.payload()
	}
	fn from_payload(payload: u64) -> Self {
		Self(T::from_payload(payload))
	}
}

impl<I, O: Payload> Payload for Function<I, O> {
	fn payload(&self) -> u64 {
		self.0.payload()
	}
	fn from_payload(payload: u64) -> Self {
		Self(O::from_payload(payload), PhantomData)
	}
}

pub fn mix(hash: u64, value: u64) -> u64 {
	(hash ^ value).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29)
}

pub struct Value<T>(u64, PhantomData<T>);

impl<T> Value<T> {
	pub fn new(hash: u64) -> Self {
		Self(hash, PhantomData)
	}
}
";

const SYNC_PRELUDE: &str = "
pub trait Node {
	type Output;
	fn eval(&self, context: EvalContext) -> Self::Output;
}

impl<N: Node + ?Sized> Node for &N {
	type Output = N::Output;
	fn eval(&self, context: EvalContext) -> Self::Output {
		(**self).eval(context)
	}
}

impl<T: Payload> Node for Value<T> {
	type Output = T;
	fn eval(&self, _: EvalContext) -> T {
		T::from_payload(self.0)
	}
}

pub struct Erased<'a, O>(pub &'a dyn Node<Output = O>);

impl<O> Node for Erased<'_, O> {
	type Output = O;
	fn eval(&self, context: EvalContext) -> O {
		self.0.eval(context)
	}
}

pub fn evaluate<N: Node>(node: &N, context: EvalContext) -> u64
where
	N::Output: Payload,
{
	node.eval(context).payload()
}

pub fn assert_output<T, N: Node<Output = T>>(_: &N) {}
";

const ASYNC_PRELUDE: &str = "
pub trait Node {
	type Output;
	fn eval(&self, context: EvalContext) -> impl Future<Output = Self::Output>;
}

impl<N: Node + ?Sized> Node for &N {
	type Output = N::Output;
	fn eval(&self, context: EvalContext) -> impl Future<Output = Self::Output> {
		(**self).eval(context)
	}
}

impl<T: Payload> Node for Value<T> {
	type Output = T;
	fn eval(&self, _: EvalContext) -> impl Future<Output = T> {
		std::future::ready(T::from_payload(self.0))
	}
}

pub trait DynNode {
	type Output;
	fn eval_dyn(&self, context: EvalContext) -> Pin<Box<dyn Future<Output = Self::Output> + '_>>;
}

impl<N: Node> DynNode for N {
	type Output = N::Output;
	fn eval_dyn(&self, context: EvalContext) -> Pin<Box<dyn Future<Output = Self::Output> + '_>> {
		Box::pin(self.eval(context))
	}
}

pub struct Erased<'a, O>(pub &'a dyn DynNode<Output = O>);

impl<O> Node for Erased<'_, O> {
	type Output = O;
	fn eval(&self, context: EvalContext) -> impl Future<Output = O> {
		self.0.eval_dyn(context)
	}
}

pub fn block_on<F: Future>(future: F) -> F::Output {
	let mut future = std::pin::pin!(future);
	let mut task_context = std::task::Context::from_waker(std::task::Waker::noop());
	loop {
		if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut task_context) {
			return output;
		}
	}
}

pub fn evaluate<N: Node>(node: &N, context: EvalContext) -> u64
where
	N::Output: Payload,
{
	block_on(node.eval(context)).payload()
}

pub fn assert_output<T, N: Node<Output = T>>(_: &N) {}
";

const MAIN: &str = "
fn main() {
	let iterations: u64 = std::env::args().nth(1).and_then(|argument| argument.parse().ok()).unwrap_or(1000);
	run(iterations / 10 + 1);

	let start = std::time::Instant::now();
	let checksum = run(iterations);
	let elapsed = start.elapsed();
	println!(\"{} {checksum}\", elapsed.as_nanos() / iterations as u128);
}
";

/// Where node types are erased behind `&dyn Node<Output = T>`, which bounds how deeply node types nest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Erasure {
	Monomorphic,
	/// At nodes with several consumers, and where a node type would nest deeper than `MAX_NESTING`.
	Bounded,
	Everywhere,
}

const MAX_NESTING: u32 = 32;

/// Graphite's nodes return futures, which erased nodes box like `DynFuture`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evaluation {
	Sync,
	Async,
}

pub struct Emitted {
	pub source: String,
	pub statements: usize,
	pub node_types: usize,
	pub concrete_types: usize,
	/// How many node evaluations one evaluation of the output performs, since nothing caches shared nodes.
	pub evaluations_per_run: f64,
}

/// Generic implementations get one stub per specialization, since their input and output types vary between uses.
#[derive(PartialEq, Eq, Hash)]
struct Monomorphization {
	identifier: IdentifierId,
	implementation: u32,
	specialization: Option<(TypeId, Box<[TypeId]>)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
	Structs(Evaluation),
	/// Generic over `impl Fn` inputs, so every IR node instantiates its implementation with flat function item types.
	Functions,
	/// Over `fn` pointer inputs, so every implementation is instantiated once and called indirectly.
	FunctionPointers,
}

struct Emitter<'a> {
	lean: &'a Lean,
	style: Style,
	rendered_types: FxHashMap<TypeId, String>,
	type_definitions: String,
	concrete_types: usize,
	node_types: FxHashMap<Monomorphization, String>,
	node_definitions: String,
}

fn evaluations_per_run(lean: &Lean, reachable: &[u32], output: u32) -> f64 {
	let mut evaluations = vec![0_f64; lean.ir.len()];
	evaluations[output as usize] = 1.;
	for &id in reachable.iter().rev() {
		for &input in lean.ir.inputs(id) {
			evaluations[input as usize] += evaluations[id as usize];
		}
	}
	reachable.iter().map(|&id| evaluations[id as usize]).sum()
}

/// Emits each IR node as a function calling its node implementation with its inputs' functions, evaluating synchronously.
pub fn emit_functions(lean: &Lean, output: u32, pointers: bool) -> Emitted {
	let mut emitter = Emitter::new(lean, if pointers { Style::FunctionPointers } else { Style::Functions });

	let reachable = lean.ir.reachable(output);
	let mut functions = String::new();
	for &id in &reachable {
		let node = &lean.ir.nodes[id as usize];
		let return_type = emitter.render(lean.typing.info[id as usize].return_type);
		let body = match node.value {
			Some(value) => format!("<{return_type}>::from_payload({:#x})", value.hash),
			None => {
				let node_type = emitter.node_type(id);
				let arguments: Vec<String> = lean.ir.inputs(id).iter().map(|input| format!("n{input}")).collect();
				format!("nodes::{node_type}(context, {})", arguments.join(", "))
			}
		};
		writeln!(functions, "pub fn n{id}(context: EvalContext) -> {return_type} {{\n\t{body}\n}}\n").unwrap();
	}

	let mut source = String::with_capacity(PRELUDE.len() + emitter.type_definitions.len() + emitter.node_definitions.len() + functions.len() + 4096);
	if !pointers {
		writeln!(source, "#![recursion_limit = \"{MONOMORPHIC_RECURSION_LIMIT}\"]").unwrap();
	}
	source.push_str(PRELUDE);
	write!(source, "\npub mod types {{\n\tuse super::Payload;\n\n{}}}\n", emitter.type_definitions).unwrap();
	write!(source, "\npub mod nodes {{\n\tuse super::*;\n\n{}}}\n", emitter.node_definitions).unwrap();
	write!(source, "\n{functions}").unwrap();
	write!(
		source,
		"pub fn run(iterations: u64) -> u64 {{\n\tlet mut checksum = 0;\n\tfor iteration in 0..iterations {{\n\t\tchecksum ^= n{output}(std::hint::black_box(iteration)).payload();\n\t}}\n\tchecksum\n}}\n"
	)
	.unwrap();
	source.push_str(MAIN);

	Emitted {
		source,
		statements: reachable.len(),
		node_types: emitter.node_types.len(),
		concrete_types: emitter.concrete_types,
		evaluations_per_run: evaluations_per_run(lean, &reachable, output),
	}
}

pub fn emit(lean: &Lean, output: u32, erasure: Erasure, evaluation: Evaluation) -> Emitted {
	let mut emitter = Emitter::new(lean, Style::Structs(evaluation));

	let reachable = lean.ir.reachable(output);
	let mut consumers = vec![0_u32; lean.ir.len()];
	for &id in &reachable {
		for &input in lean.ir.inputs(id) {
			consumers[input as usize] += 1;
		}
	}

	let mut nesting = vec![0_u32; lean.ir.len()];
	let mut body = String::new();
	for &id in &reachable {
		let node = &lean.ir.nodes[id as usize];
		nesting[id as usize] = 1 + lean.ir.inputs(id).iter().map(|&input| nesting[input as usize]).max().unwrap_or(0);
		let expression = match node.value {
			Some(value) => format!("Value::<{}>::new({:#x})", emitter.render(value.ty), value.hash),
			None => {
				let node_type = emitter.node_type(id);
				let arguments: Vec<String> = lean.ir.inputs(id).iter().map(|input| format!("&n{input}")).collect();
				format!("nodes::{node_type}::new({})", arguments.join(", "))
			}
		};

		let erased = match erasure {
			Erasure::Monomorphic => false,
			Erasure::Bounded => consumers[id as usize] > 1 || nesting[id as usize] >= MAX_NESTING,
			Erasure::Everywhere => true,
		};
		if erased {
			nesting[id as usize] = 1;
			let output = emitter.render(lean.typing.info[id as usize].return_type);
			writeln!(body, "\tlet n{id}_node = {expression};\n\tlet n{id} = Erased::<{output}>(&n{id}_node);").unwrap();
		} else {
			writeln!(body, "\tlet n{id} = {expression};").unwrap();
		}
	}
	let output_type = emitter.render(lean.typing.info[output as usize].return_type);
	writeln!(body, "\tassert_output::<{output_type}, _>(&n{output});\n").unwrap();
	writeln!(body, "\tlet mut checksum = 0;\n\tfor iteration in 0..iterations {{").unwrap();
	writeln!(body, "\t\tchecksum ^= evaluate(&n{output}, std::hint::black_box(iteration));\n\t}}\n\tchecksum").unwrap();

	let mut source = String::with_capacity(PRELUDE.len() + emitter.type_definitions.len() + emitter.node_definitions.len() + body.len() + 4096);
	if erasure != Erasure::Everywhere {
		writeln!(source, "#![recursion_limit = \"{MONOMORPHIC_RECURSION_LIMIT}\"]").unwrap();
	}
	source.push_str(PRELUDE);
	source.push_str(match evaluation {
		Evaluation::Sync => SYNC_PRELUDE,
		Evaluation::Async => ASYNC_PRELUDE,
	});
	write!(source, "\npub mod types {{\n\tuse super::Payload;\n\n{}}}\n", emitter.type_definitions).unwrap();
	write!(source, "\npub mod nodes {{\n\tuse super::*;\n\n{}}}\n", emitter.node_definitions).unwrap();
	write!(source, "\npub fn run(iterations: u64) -> u64 {{\n{body}}}\n").unwrap();
	source.push_str(MAIN);

	Emitted {
		source,
		statements: reachable.len(),
		node_types: emitter.node_types.len(),
		concrete_types: emitter.concrete_types,
		evaluations_per_run: evaluations_per_run(lean, &reachable, output),
	}
}

impl<'a> Emitter<'a> {
	fn new(lean: &'a Lean, style: Style) -> Self {
		Self {
			lean,
			style,
			rendered_types: FxHashMap::default(),
			type_definitions: String::new(),
			concrete_types: 0,
			node_types: FxHashMap::default(),
			node_definitions: String::new(),
		}
	}

	fn render(&mut self, ty: TypeId) -> String {
		if let Some(rendered) = self.rendered_types.get(&ty) {
			return rendered.clone();
		}

		let rendered = match self.lean.typing.types.get(ty) {
			TypeData::Concrete(_) | TypeData::Generic(_) => {
				let name = self.lean.typing.types.concrete_name(ty).unwrap_or("unresolved generic");
				let stub = format!("T{}", self.concrete_types);
				writeln!(
					self.type_definitions,
					"\t#[doc = {name:?}]\n\tpub struct {stub}(u64);\n\timpl Payload for {stub} {{\n\t\tfn payload(&self) -> u64 {{\n\t\t\tself.0\n\t\t}}\n\t\tfn from_payload(payload: u64) -> Self {{\n\t\t\tSelf(payload)\n\t\t}}\n\t}}\n"
				)
				.unwrap();
				self.concrete_types += 1;
				format!("types::{stub}")
			}
			TypeData::Item(element) => format!("Item<{}>", self.render(element)),
			TypeData::List(element) => format!("List<{}>", self.render(element)),
			TypeData::Fn(input, output) => format!("Function<{}, {}>", self.render(input), self.render(output)),
		};
		self.rendered_types.insert(ty, rendered.clone());
		rendered
	}

	fn node_type(&mut self, id: u32) -> String {
		let (ir, typing) = (&self.lean.ir, &self.lean.typing);
		let node = &ir.nodes[id as usize];
		let info = typing.info[id as usize];
		let signature = &typing.registry.implementations[&node.identifier][info.implementation as usize];

		let generic = !signature.generics.is_empty();
		let input_types: Box<[TypeId]> = if generic {
			ir.inputs(id).iter().map(|&input| typing.info[input as usize].return_type).collect()
		} else {
			signature.inputs.iter().map(|&input| typing.types.nested(input)).collect()
		};
		let key = Monomorphization {
			identifier: node.identifier,
			implementation: info.implementation,
			specialization: generic.then(|| (info.return_type, input_types.clone())),
		};
		if let Some(name) = self.node_types.get(&key) {
			return name.clone();
		}

		let seed = self.node_types.len();
		let sanitized: String = self.lean.identifier_name(node.identifier).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
		let name = format!("{sanitized}_{seed}");
		let output = self.render(info.return_type);
		let bounds: Vec<String> = input_types.iter().enumerate().map(|(index, &ty)| format!("I{index}: Node<Output = {}>", self.render(ty))).collect();
		let parameters: Vec<String> = (0..input_types.len()).map(|index| format!("I{index}")).collect();
		let arguments: Vec<String> = (0..input_types.len()).map(|index| format!("i{index}: I{index}")).collect();
		let fields: Vec<String> = (0..input_types.len()).map(|index| format!("i{index}")).collect();

		let mut evaluation = format!("let mut hash = mix(context, {seed});");
		for index in 0..input_types.len() {
			match self.style {
				Style::Structs(Evaluation::Sync) => write!(evaluation, " hash = mix(hash, self.{index}.eval(context).payload());").unwrap(),
				Style::Structs(Evaluation::Async) => write!(evaluation, " hash = mix(hash, self.{index}.eval(context).await.payload());").unwrap(),
				Style::Functions | Style::FunctionPointers => write!(evaluation, " hash = mix(hash, i{index}(context).payload());").unwrap(),
			}
		}
		write!(evaluation, " <{output}>::from_payload(std::hint::black_box(hash))").unwrap();

		let documentation = format!("\t#[doc = {:?}]", self.lean.identifier_name(node.identifier));
		if matches!(self.style, Style::Functions | Style::FunctionPointers) {
			let input_types: Vec<String> = input_types.iter().map(|&ty| self.render(ty)).collect();
			let parameters: Vec<String> = input_types
				.iter()
				.enumerate()
				.map(|(index, ty)| match self.style {
					Style::FunctionPointers => format!("i{index}: fn(EvalContext) -> {ty}"),
					_ => format!("i{index}: impl Fn(EvalContext) -> {ty}"),
				})
				.collect();
			let parameters = std::iter::once("context: EvalContext".to_string()).chain(parameters).collect::<Vec<_>>().join(", ");
			writeln!(self.node_definitions, "{documentation}\n\tpub fn {name}({parameters}) -> {output} {{\n\t\t{evaluation}\n\t}}\n").unwrap();

			self.node_types.insert(key, name.clone());
			return name;
		}

		let eval = match self.style {
			Style::Structs(Evaluation::Async) => format!("fn eval(&self, context: EvalContext) -> impl Future<Output = Self::Output> {{ async move {{ {evaluation} }} }}"),
			_ => format!("fn eval(&self, context: EvalContext) -> Self::Output {{ {evaluation} }}"),
		};

		let (bounds, parameters) = (bounds.join(", "), parameters.join(", "));
		let definitions = &mut self.node_definitions;
		writeln!(definitions, "{documentation}").unwrap();
		writeln!(definitions, "\tpub struct {name}<{parameters}>({parameters});").unwrap();
		writeln!(definitions, "\timpl<{bounds}> Node for {name}<{parameters}> {{\n\t\ttype Output = {output};\n\t\t{eval}\n\t}}").unwrap();
		writeln!(
			definitions,
			"\timpl<{bounds}> {name}<{parameters}> {{\n\t\tpub fn new({}) -> Self {{\n\t\t\tSelf({})\n\t\t}}\n\t}}\n",
			arguments.join(", "),
			fields.join(", ")
		)
		.unwrap();

		self.node_types.insert(key, name.clone());
		name
	}
}
