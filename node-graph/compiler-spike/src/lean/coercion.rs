//! Automatic conversions between data types through `Convert` and `ConvertAsync` implementations, chosen by a purity relation
//! instead of hardcoded in the compiler.
//!
//! A conversion from `A` to `B` is applied automatically when `A` is purer than `B` (data loses purity but never regains it by
//! discarding information, which needs an explicit node) or when both are representations of the same data, like CPU and GPU
//! rasters. At most one conversion is applied per input. Among the implementations a node could use after converting, the one
//! needing the fewest purity-losing conversions, then the fewest representation changes, then receiving the purest data wins.

use super::document::{IdentifierId, Strings};
use super::registry::Registry;
use super::types::{TypeData, TypeId, TypeInterner};
use crate::workload::CompileMetadata;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::path::Path;

const FOOTPRINT: u32 = 1 << 0;

/// A `Convert<To, Converter>` (or `ConvertAsync`) implementation for `From`.
#[derive(Deserialize)]
struct ConvertImplementation {
	from: String,
	to: String,
	converter: Option<ConverterDeclaration>,
	#[serde(default)]
	asynchronous: bool,
	#[serde(default)]
	reads_footprint: bool,
}

#[derive(Deserialize)]
struct ConverterDeclaration {
	#[serde(rename = "type")]
	ty: String,
	scope: String,
}

#[derive(Deserialize)]
struct NodeDeclaration {
	identifier: String,
	/// Concrete type names, or `generic:T` for a generic parameter.
	element_types: Vec<String>,
	#[serde(default = "both_ranks")]
	ranks: Vec<Rank>,
}

fn both_ranks() -> Vec<Rank> {
	vec![Rank::Item, Rank::List]
}

/// The spike's stand-in for purity declarations on Graphite's types, the `Convert` implementations, and test nodes.
#[derive(Deserialize)]
struct CoercionData {
	purity: Vec<(String, String)>,
	equivalent: Vec<(String, String)>,
	conversions: Vec<ConvertImplementation>,
	nodes: Vec<NodeDeclaration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum Rank {
	Item,
	List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConversionKind {
	/// Between representations of the same data, which is semantically invisible.
	Representation,
	/// From purer data to less pure data.
	PurityLosing,
	/// A registered conversion on a cycle of registered conversions, like `f64` and `i64`, so neither side is purer.
	Lossy,
}

/// The conversion node turning one element type into another at a rank, like `convert_item` over `Convert<To, Converter>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConversionNode {
	pub identifier: IdentifierId,
	/// The scope key providing the `Converter` the implementation needs as a second input, like the `WgpuExecutor`.
	pub converter_scope: Option<u32>,
	pub kind: ConversionKind,
	pub reads_footprint: bool,
	pub forwards_input: bool,
}

impl ConversionNode {
	pub fn compile_metadata(&self) -> CompileMetadata {
		CompileMetadata {
			extract: if self.reads_footprint { FOOTPRINT } else { 0 },
			forwards_input: self.forwards_input,
			..Default::default()
		}
	}
}

#[derive(Default)]
pub struct Coercions {
	/// Pairs `(purer, less pure)`, transitively closed.
	purer: FxHashSet<(TypeId, TypeId)>,
	equivalent: FxHashSet<(TypeId, TypeId)>,
	conversions: FxHashMap<(TypeId, TypeId, Rank), ConversionNode>,
	/// Registered conversions that change a wire's whole type rather than its element, like `List<T>` into `ListDyn`.
	wire_conversions: FxHashMap<(TypeId, TypeId), ConversionNode>,
}

impl Coercions {
	pub fn wire_conversion(&self, from: TypeId, to: TypeId) -> Option<ConversionNode> {
		self.wire_conversions.get(&(from, to)).copied()
	}

	pub fn is_purer(&self, a: TypeId, b: TypeId) -> bool {
		self.purer.contains(&(a, b))
	}

	/// The automatic conversion from one element type to another, if `Convert` implements it and purity allows applying it.
	pub fn automatic_conversion(&self, from: TypeId, to: TypeId, rank: Rank) -> Option<ConversionNode> {
		let conversion = self.conversions.get(&(from, to, rank))?;
		let allowed = match conversion.kind {
			ConversionKind::Representation => true,
			ConversionKind::PurityLosing => self.is_purer(from, to),
			ConversionKind::Lossy => true,
		};
		allowed.then_some(*conversion)
	}

	/// The rank and element of a wire type `Fn(_, Item<T>)` or `Fn(_, List<T>)`.
	pub fn ranked_element(types: &TypeInterner, wire: TypeId) -> Option<(Rank, TypeId)> {
		match types.get(types.nested(wire)) {
			TypeData::Item(element) => Some((Rank::Item, element)),
			TypeData::List(element) => Some((Rank::List, element)),
			_ => None,
		}
	}
}

/// Loads the spike's coercion data, registering the conversion nodes' implementations and the test nodes.
pub fn load(types: &mut TypeInterner, strings: &mut Strings, registry: &mut Registry, context: TypeId) {
	let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("spike-data/coercions.json");
	let content = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("Failed to read {}: {error}", path.display()));
	let data: CoercionData = serde_json::from_str(&content).unwrap_or_else(|error| panic!("Failed to parse {}: {error}", path.display()));

	let equivalent = data
		.equivalent
		.iter()
		.flat_map(|(a, b)| {
			let (a, b) = (types.concrete(a), types.concrete(b));
			[(a, b), (b, a)]
		})
		.collect();
	let mut coercions = Coercions { equivalent, ..Default::default() };

	for conversion in &data.conversions {
		let (from, to) = (types.concrete(&conversion.from), types.concrete(&conversion.to));
		let converter = conversion.converter.as_ref().map(|converter| (types.concrete(&converter.ty), strings.intern(&converter.scope)));
		let kind = if coercions.equivalent.contains(&(from, to)) {
			ConversionKind::Representation
		} else {
			ConversionKind::PurityLosing
		};

		for (rank, wrap) in [(Rank::Item, TypeData::Item as fn(TypeId) -> TypeData), (Rank::List, TypeData::List)] {
			// Monomorphized over the target like `convert_item`'s `PhantomData<E>`, so typing the inserted node is unambiguous
			let node = match (rank, conversion.asynchronous) {
				(Rank::Item, false) => "graphene_core::ops::ConvertItemNode",
				(Rank::List, false) => "graphene_core::ops::ConvertListNode",
				(Rank::Item, true) => "graphene_core::ops::ConvertAsyncItemNode",
				(Rank::List, true) => "graphene_core::ops::ConvertAsyncListNode",
			};
			let identifier = IdentifierId(strings.intern(&format!("{node}<{}>", conversion.to)));

			let (from_wire, to_wire) = (types.intern(wrap(from)), types.intern(wrap(to)));
			let mut inputs = vec![types.function(context, from_wire)];
			if let Some((converter, _)) = converter {
				let converter_wire = types.intern(TypeData::Item(converter));
				inputs.push(types.function(context, converter_wire));
			}
			registry.add(types, identifier, context, inputs, to_wire);
			coercions.conversions.insert(
				(from, to, rank),
				ConversionNode {
					identifier,
					converter_scope: converter.map(|(_, scope)| scope),
					kind,
					reads_footprint: conversion.reads_footprint,
					forwards_input: false,
				},
			);
		}
	}

	// Graphite's input adapter rows are the conversions node authors registered, each into the adapter's element type
	let mut registered = Vec::new();
	for (&identifier, signatures) in &registry.implementations {
		if !strings.name(identifier.0).starts_with("input_adapter<") {
			continue;
		}
		for signature in signatures {
			let [input] = signature.inputs.as_slice() else { continue };
			match (Coercions::ranked_element(types, *input), Coercions::ranked_element(types, signature.output)) {
				(Some((from_rank, from)), Some((to_rank, to))) if from_rank == to_rank => {
					if from != to {
						registered.push((from, to, from_rank, identifier));
					}
				}
				_ => {
					let wire_conversion = ConversionNode {
						identifier,
						converter_scope: None,
						kind: ConversionKind::PurityLosing,
						reads_footprint: false,
						forwards_input: true,
					};
					coercions.wire_conversions.insert((types.nested(*input), signature.output), wire_conversion);
				}
			}
		}
	}
	let reachable = transitive_closure(registered.iter().map(|&(from, to, _, _)| (from, to)).collect());
	let mut registered_purity = Vec::new();
	for (from, to, rank, identifier) in registered {
		let lossy = reachable.contains(&(to, from));
		if !lossy {
			registered_purity.push((from, to));
		}
		coercions.conversions.entry((from, to, rank)).or_insert(ConversionNode {
			identifier,
			converter_scope: None,
			kind: if lossy { ConversionKind::Lossy } else { ConversionKind::PurityLosing },
			reads_footprint: false,
			forwards_input: true,
		});
	}

	for node in &data.nodes {
		let identifier = IdentifierId(strings.intern(&node.identifier));
		for element in &node.element_types {
			let element = match element.strip_prefix("generic:") {
				Some(name) => types.convert(&crate::workload::Type::Generic(name.to_string())),
				None => types.concrete(element),
			};
			for &rank in &node.ranks {
				let wire = types.intern(match rank {
					Rank::Item => TypeData::Item(element),
					Rank::List => TypeData::List(element),
				});
				let input = types.function(context, wire);
				registry.add(types, identifier, context, vec![input], wire);
			}
		}
	}

	let declared = data.purity.iter().map(|(a, b)| (types.concrete(a), types.concrete(b)));
	let purer = transitive_closure(declared.chain(registered_purity).collect());
	if let Some(&(cycle, _)) = purer.iter().find(|&&(a, b)| a == b) {
		panic!("The purity relation has a cycle through {}", types.concrete_name(cycle).unwrap_or("a type"));
	}
	coercions.purer = purer;
	registry.coercions = coercions;
}

/// O(n³) in the worst case over the few dozen convertible types.
fn transitive_closure(mut pairs: FxHashSet<(TypeId, TypeId)>) -> FxHashSet<(TypeId, TypeId)> {
	loop {
		let derived: Vec<_> = pairs
			.iter()
			.flat_map(|&(a, b)| pairs.iter().filter(move |&&(c, _)| c == b).map(move |&(_, d)| (a, d)))
			.filter(|pair| !pairs.contains(pair))
			.collect();
		if derived.is_empty() {
			return pairs;
		}
		pairs.extend(derived);
	}
}
