use compiler_spike::baseline::types::CONTEXT_NAME;
use compiler_spike::lean::Lean;
use compiler_spike::synthetic::wrap_in_scope;
use compiler_spike::workload::{Implementation, Input, Network, Node, NodeReference, Type, Value};

const COLOR: &str = "no_std_types::color::color_types::Color";
const VECTOR: &str = "vector_types::vector::vector_types::Vector";
const GRADIENT: &str = "vector_types::gradient::Gradient";
const RASTER_CPU: &str = "raster_types::raster_types::Raster<raster_types::raster_types::cpu::CPU>";
const RASTER_GPU: &str = "raster_types::raster_types::Raster<raster_types::raster_types::gpu::GPU>";

/// What compiling a value of `element` fed into `node` produced: the output's element type, and the identifier and input
/// count of the node feeding it.
struct Coerced {
	output: Type,
	feeding: String,
	feeding_inputs: Vec<String>,
	feeding_reads_footprint: bool,
}

fn feed(element: &str, node: &str) -> Result<Coerced, String> {
	let value = Input::Value(Value {
		ty: Type::Item(Box::new(Type::Concrete(element.to_string()))),
		hash: 7,
		size: 8,
		exposed: false,
		producer: None,
	});
	let consumer = Node {
		inputs: vec![value],
		call_argument: Type::Concrete(CONTEXT_NAME.to_string()),
		implementation: Implementation::Proto(node.to_string()),
		visible: true,
		skip_deduplication: false,
		compile_metadata: None,
	};
	let document = Network {
		exports: vec![Input::Node(NodeReference { node: 1, output: 0 })],
		nodes: vec![(1, consumer)],
		scope_injections: Vec::new(),
		generated: false,
	};

	let mut lean = Lean::load();
	let document = lean.convert_document(&wrap_in_scope(document, false));
	let output = lean.compile(&document)?.output;

	let [feeding] = lean.ir.inputs(output) else {
		return Err("The consumer should have one input".to_string());
	};
	let feeding = *feeding;
	Ok(Coerced {
		output: lean.typing.types.to_workload(lean.typing.info[output as usize].return_type),
		feeding: lean.identifier_name(lean.ir.nodes[feeding as usize].identifier).to_string(),
		feeding_inputs: lean
			.ir
			.inputs(feeding)
			.iter()
			.map(|&input| lean.identifier_name(lean.ir.nodes[input as usize].identifier).to_string())
			.collect(),
		feeding_reads_footprint: lean.ir.nodes[feeding as usize].compile_metadata.extract != 0,
	})
}

fn item(element: &str) -> Type {
	Type::Item(Box::new(Type::Concrete(element.to_string())))
}

#[test]
fn matching_inputs_need_no_conversion() {
	let coerced = feed(VECTOR, "spike::Blur").unwrap();
	assert_eq!(coerced.output, item(VECTOR));
	assert_eq!(coerced.feeding, "core_types::value::ClonedNode");
}

#[test]
fn a_purer_input_converts_to_the_implementation_it_can_reach() {
	// Color converts to Vector, while no conversion reaches Raster directly and chains are not followed
	let coerced = feed(COLOR, "spike::Blur").unwrap();
	assert_eq!(coerced.output, item(VECTOR));
	assert_eq!(coerced.feeding, format!("graphene_core::ops::ConvertItemNode<{VECTOR}>"));
}

#[test]
fn the_purest_reachable_target_wins() {
	// Color reaches both Vector and Gradient, and Gradient is purer
	let coerced = feed(COLOR, "spike::Shade").unwrap();
	assert_eq!(coerced.output, item(GRADIENT));
}

#[test]
fn conversions_regaining_purity_are_not_automatic() {
	// Vectorizing exists as a conversion, but it would discard information to regain purity
	assert!(feed(RASTER_CPU, "spike::VectorOnly").is_err());
}

#[test]
fn incomparable_targets_are_ambiguous() {
	let error = feed(COLOR, "spike::Ambiguous").err().expect("Vector and String are incomparable");
	assert!(error.contains("Ambiguous"), "{error}");
}

#[test]
fn representation_changes_convert_with_their_converter() {
	let coerced = feed(RASTER_CPU, "spike::GpuOnly").unwrap();
	assert_eq!(coerced.output, item(RASTER_GPU));
	assert_eq!(coerced.feeding, format!("graphene_core::ops::ConvertItemNode<{RASTER_GPU}>"));
	assert_eq!(coerced.feeding_inputs, ["core_types::value::ClonedNode", "graphene_std::platform_application_io::WgpuExecutorNode"]);
}

#[test]
fn only_rasterizing_conversions_read_the_footprint() {
	assert!(!feed(COLOR, "spike::Blur").unwrap().feeding_reads_footprint);
	assert!(feed(VECTOR, "spike::RasterOnly").unwrap().feeding_reads_footprint);
}

#[test]
fn types_without_a_purity_relation_do_not_convert() {
	assert!(feed("f64", "spike::VectorOnly").is_err());
}
