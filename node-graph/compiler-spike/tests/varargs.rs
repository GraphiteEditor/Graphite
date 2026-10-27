use compiler_spike::baseline::types::CONTEXT_NAME;
use compiler_spike::lean::Lean;
use compiler_spike::synthetic::wrap_in_scope;
use compiler_spike::workload::{Implementation, Input, Network, Node, NodeReference, Type, Value};
use std::hash::{DefaultHasher, Hash, Hasher};

const COLOR: &str = "no_std_types::color::color_types::Color";
const VECTOR: &str = "vector_types::vector::vector_types::Vector";
const STRING: &str = "alloc::string::String";
const VARARGS: u32 = 1 << 6;

fn item(element: &str) -> Type {
	Type::Item(Box::new(Type::Concrete(element.to_string())))
}

fn value(element: &str, hash: u64) -> Input {
	Input::Value(Value {
		ty: item(element),
		hash,
		size: 8,
		exposed: false,
		producer: None,
	})
}

fn name(name: &str) -> Input {
	let mut hasher = DefaultHasher::new();
	name.hash(&mut hasher);
	value(STRING, hasher.finish())
}

fn node(id: u64) -> Input {
	Input::Node(NodeReference { node: id, output: 0 })
}

fn proto(identifier: &str, inputs: Vec<Input>) -> Node {
	Node {
		inputs,
		call_argument: Type::Concrete(CONTEXT_NAME.to_string()),
		implementation: Implementation::Proto(identifier.to_string()),
		visible: true,
		skip_deduplication: false,
		compile_metadata: None,
	}
}

fn read(variable: &str) -> Node {
	proto("spike::ReadVarArg", vec![name(variable)])
}

fn inject(variable: &str, value: Input, body: Input) -> Node {
	proto("spike::InjectVarArg", vec![name(variable), value, body])
}

fn network(nodes: Vec<(u64, Node)>, output: u64) -> Network {
	Network {
		exports: vec![node(output)],
		nodes,
		scope_injections: Vec::new(),
		generated: false,
	}
}

struct Compiled {
	lean: Lean,
	output: u32,
}

impl Compiled {
	fn output_type(&self) -> Type {
		self.lean.typing.types.to_workload(self.lean.typing.info[self.output as usize].return_type)
	}

	fn identifiers(&self) -> Vec<String> {
		self.lean
			.ir
			.reachable(self.output)
			.into_iter()
			.map(|id| self.lean.identifier_name(self.lean.ir.nodes[id as usize].identifier).to_string())
			.collect()
	}

	fn readers(&self) -> Vec<String> {
		let mut readers: Vec<_> = self.identifiers().into_iter().filter(|identifier| identifier.starts_with("spike::ReadVarArg<")).collect();
		readers.sort();
		readers
	}

	fn node_with(&self, prefix: &str) -> u32 {
		self.lean
			.ir
			.reachable(self.output)
			.into_iter()
			.find(|&id| self.lean.identifier_name(self.lean.ir.nodes[id as usize].identifier).starts_with(prefix))
			.unwrap_or_else(|| panic!("No {prefix} node"))
	}
}

fn compile(nodes: Vec<(u64, Node)>, output: u64) -> Result<Compiled, String> {
	let mut lean = Lean::load();
	let document = lean.convert_document(&wrap_in_scope(network(nodes, output), false));
	let output = lean.compile(&document)?.output;
	Ok(Compiled { lean, output })
}

#[test]
fn readers_take_the_type_of_their_binding() {
	let compiled = compile(vec![(1, read("x")), (2, inject("x", value(COLOR, 1), node(1)))], 2).unwrap();
	assert_eq!(compiled.output_type(), item(COLOR));
	assert_eq!(compiled.readers(), [format!("spike::ReadVarArg<Item<{COLOR}>>")]);
}

#[test]
fn read_types_flow_into_selection_and_coercion() {
	let compiled = compile(vec![(1, read("x")), (2, proto("spike::Blur", vec![node(1)])), (3, inject("x", value(COLOR, 1), node(2)))], 3).unwrap();
	assert_eq!(compiled.output_type(), item(VECTOR));
	assert!(compiled.identifiers().contains(&format!("graphene_core::ops::ConvertItemNode<{VECTOR}>")));
}

#[test]
fn shared_subgraphs_instantiate_once_per_binding_type() {
	let nodes = vec![
		(1, read("x")),
		(2, proto("spike::Blur", vec![node(1)])),
		(3, inject("x", value(COLOR, 1), node(2))),
		(4, inject("x", value(VECTOR, 2), node(2))),
		(5, inject("y", node(3), node(4))),
	];
	let compiled = compile(nodes, 5).unwrap();
	assert_eq!(compiled.output_type(), item(VECTOR));
	assert_eq!(compiled.readers(), [format!("spike::ReadVarArg<Item<{COLOR}>>"), format!("spike::ReadVarArg<Item<{VECTOR}>>")]);
	let blurs = compiled.identifiers().into_iter().filter(|identifier| identifier == "spike::Blur").count();
	assert_eq!(blurs, 2, "The Blur reading each binding is instantiated for it");
}

#[test]
fn bindings_of_the_same_type_share_their_readers() {
	let nodes = vec![
		(1, read("x")),
		(2, inject("x", value(COLOR, 1), node(1))),
		(3, inject("x", value(COLOR, 2), node(1))),
		(4, inject("y", node(2), node(3))),
	];
	let compiled = compile(nodes, 4).unwrap();
	assert_eq!(compiled.readers(), [format!("spike::ReadVarArg<Item<{COLOR}>>")]);
}

#[test]
fn inner_bindings_shadow_outer_ones() {
	let nodes = vec![(1, read("x")), (2, inject("x", value(COLOR, 1), node(1))), (3, inject("x", value(VECTOR, 2), node(2)))];
	assert_eq!(compile(nodes, 3).unwrap().output_type(), item(COLOR));
}

#[test]
fn readers_inside_nested_networks_see_outer_bindings() {
	let inner = network(vec![(1, read("x"))], 1);
	let mut instance = proto("unused", Vec::new());
	instance.implementation = Implementation::Network(inner);
	let compiled = compile(vec![(1, instance), (2, inject("x", value(COLOR, 1), node(1)))], 2).unwrap();
	assert_eq!(compiled.output_type(), item(COLOR));
}

#[test]
fn unbound_readers_are_errors() {
	let error = compile(vec![(1, read("x")), (2, inject("y", value(COLOR, 1), node(1)))], 2).err().expect("x is unbound");
	assert!(error.contains("Unbound vararg"), "{error}");
}

#[test]
fn vararg_names_must_be_constants() {
	let nodes = vec![(1, proto("spike::Blur", vec![value(VECTOR, 1)])), (2, proto("spike::ReadVarArg", vec![node(1)]))];
	let error = compile(nodes, 2).err().expect("the name is computed");
	assert!(error.contains("compile-time constants"), "{error}");
}

#[test]
fn injectors_supply_the_feature_their_readers_extract() {
	let compiled = compile(vec![(1, read("x")), (2, inject("x", value(COLOR, 1), node(1)))], 2).unwrap();
	let analysis = &compiled.lean.context.analysis;
	assert_ne!(analysis[compiled.node_with("spike::ReadVarArg<") as usize].features & VARARGS, 0);
	assert_eq!(analysis[compiled.node_with("spike::InjectVarArg") as usize].features & VARARGS, 0);
}

#[test]
fn incremental_compiles_match_cold_compiles_with_bindings() {
	let nodes = vec![
		(1, read("x")),
		(2, proto("spike::Blur", vec![node(1)])),
		(3, inject("x", value(COLOR, 1), node(2))),
		(4, inject("x", value(VECTOR, 2), node(2))),
		(5, inject("y", node(3), node(4))),
	];
	let mut lean = Lean::load();
	let mut document = lean.convert_document(&wrap_in_scope(network(nodes, 5), false));
	let cold = lean.compile(&document).unwrap().output;
	let cold_nodes = lean.ir.reachable(cold).len();

	let mut incremental = Lean::load();
	for _ in 0..2 {
		let output = incremental.compile_incremental(&mut document).unwrap().output;
		assert_eq!(incremental.ir.reachable(output).len(), cold_nodes);
	}
}
