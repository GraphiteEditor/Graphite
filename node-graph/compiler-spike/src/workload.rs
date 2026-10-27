//! The exported workload schema, mirroring `preprocessor/src/spike_export.rs` in Graphite.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub enum Type {
	Generic(String),
	Concrete(String),
	Fn(Box<Type>, Box<Type>),
	Item(Box<Type>),
	List(Box<Type>),
}

#[derive(Clone, Debug, Deserialize)]
pub struct NodeReference {
	pub node: u64,
	pub output: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Value {
	pub ty: Type,
	pub hash: u64,
	pub size: usize,
	pub exposed: bool,
	pub producer: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Import {
	pub ty: Type,
	pub index: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Reflection {
	DocumentNodePath,
	SourceId,
}

#[derive(Clone, Debug, Deserialize)]
pub enum Input {
	Node(NodeReference),
	Value(Value),
	Import(Import),
	Scope(String),
	Reflection(Reflection),
	Inline,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash)]
pub struct CompileMetadata {
	pub extract: u32,
	pub inject: u32,
	pub footprint_invariant: bool,
	pub forwards_input: bool,
	pub constant_inputs: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub enum Implementation {
	Network(Network),
	Proto(String),
	Extract,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Node {
	pub inputs: Vec<Input>,
	pub call_argument: Type,
	pub implementation: Implementation,
	pub visible: bool,
	pub skip_deduplication: bool,
	pub compile_metadata: Option<CompileMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Network {
	pub exports: Vec<Input>,
	pub nodes: Vec<(u64, Node)>,
	pub scope_injections: Vec<(String, u64, Type)>,
	pub generated: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Signature {
	pub call_argument: Type,
	pub inputs: Vec<Type>,
	pub output: Type,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryEntry {
	pub identifier: String,
	pub implementations: Vec<Signature>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Substitution {
	pub identifier: String,
	pub node: Node,
	pub wrapped_node_id: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InjectScope {
	pub identifier: String,
	pub template: Node,
	pub ty: Type,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CompileMetadataEntry {
	pub identifier: String,
	pub metadata: CompileMetadata,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PreprocessorTables {
	pub substitutions: Vec<Substitution>,
	pub inject_scopes: Vec<InjectScope>,
	pub compile_metadata: Vec<CompileMetadataEntry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Expected {
	pub proto_nodes: usize,
	pub unique_stable_ids: usize,
	pub distinct_identifiers: usize,
	pub output_type: Option<Type>,
}

pub const DEMO_ART: [&str; 7] = [
	"changing-seasons",
	"painted-dreams",
	"red-dress",
	"valley-of-spires",
	"isometric-fountain",
	"procedural-string-lights",
	"parametric-dunescape",
];

pub fn workload_directory() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("workloads")
}

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
	let path = workload_directory().join(format!("{name}.json"));
	let content = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("Failed to read {}: {error}", path.display()));
	serde_json::from_str(&content).unwrap_or_else(|error| panic!("Failed to parse {}: {error}", path.display()))
}

pub fn load_network(name: &str) -> Network {
	load(name)
}

pub fn load_registry() -> Vec<RegistryEntry> {
	load("registry")
}

pub fn load_preprocessor_tables() -> PreprocessorTables {
	load("preprocessor")
}

pub fn load_expected() -> HashMap<String, Expected> {
	load("expected")
}
