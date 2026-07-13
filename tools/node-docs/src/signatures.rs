use graphene_std::core_types::{self, Type};
use graphene_std::registry::{FieldMetadata, RegistryValueSource};

/// The rank/shape classification of a single input or output type, after peeling `Fn`/`Future` wrappers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rank {
	/// A rank-0 `Item<T>` ranked wire.
	Item,
	/// A rank-1 `List<T>` or type-erased `ListDyn` ranked wire.
	List,
	/// The unit type `()`, marking a generator's absent primary input.
	Unit,
	/// A generic type variable `T`, resolved by inference (adapters and passthroughs).
	Generic,
	/// A bare scalar element like `f64`, `DVec2`, or `Color` on an unranked wire.
	Bare,
}

impl Rank {
	fn tag(self) -> &'static str {
		match self {
			Rank::Item => "Item",
			Rank::List => "List",
			Rank::Unit => "Unit",
			Rank::Generic => "Generic",
			Rank::Bare => "BARE",
		}
	}
}

/// Peels to the nested type and classifies its rank wrapper by the wrapper-preserving `identifier_name`.
fn classify(ty: &Type) -> Rank {
	let nested = ty.nested_type();
	if matches!(nested, Type::Generic(_)) {
		return Rank::Generic;
	}
	let name = nested.identifier_name();
	if name.starts_with("Item<") {
		Rank::Item
	} else if name.starts_with("List<") || name.starts_with("ListDyn") {
		Rank::List
	} else if name == "()" {
		Rank::Unit
	} else {
		Rank::Bare
	}
}

/// A ranked connector registers both its `Item` and `List` monomorphizations, so the wrapper is whichever `Item` form is present; ordering across the registry is unstable, so aggregate rather than trusting the first.
fn aggregate_rank<'a>(types: impl Iterator<Item = &'a Type>) -> Rank {
	let mut rank = None;
	for ty in types {
		let candidate = classify(ty);
		// Prefer Item, then List, then Unit, then Generic, then Bare
		rank = Some(match (rank, candidate) {
			(Some(Rank::Item), _) | (_, Rank::Item) => Rank::Item,
			(Some(Rank::List), _) | (_, Rank::List) => Rank::List,
			(Some(Rank::Unit), _) | (_, Rank::Unit) => Rank::Unit,
			(Some(Rank::Generic), _) | (_, Rank::Generic) => Rank::Generic,
			_ => Rank::Bare,
		});
	}
	rank.unwrap_or(Rank::Bare)
}

/// The wrapper-preserving element name (e.g. `Item<f64>`, `List<Vector>`, `f64`), unlike `Display` which strips the rank wrapper.
fn nested_name(ty: &Type) -> String {
	ty.nested_type().identifier_name()
}

struct InputSignature {
	name: String,
	is_primary: bool,
	hidden: bool,
	types: Vec<String>,
	rank: Rank,
}

struct NodeSignature {
	identifier: String,
	display_name: String,
	category: String,
	inputs: Vec<InputSignature>,
	output_types: Vec<String>,
	output_rank: Rank,
	/// An `inject_scope` provider's output feeds only `#[scope]` params, so it is not a rankable document wire.
	inject_scope: bool,
}

impl NodeSignature {
	/// A node is impure when a non-context input or its output rides an unranked wire that a migration would lift to `Item`.
	/// A generator's `()` primary is a `Unit` input (skipped); generic passthrough inputs resolve by inference; `List` is reported so intended reducers/expanders can be judged by hand.
	fn impure_reasons(&self) -> Vec<String> {
		let mut reasons = Vec::new();

		if self.inject_scope {
			return reasons;
		}

		for input in &self.inputs {
			match input.rank {
				Rank::Bare => reasons.push(format!("bare input `{}`", input.name)),
				Rank::List => reasons.push(format!("List input `{}`", input.name)),
				Rank::Item | Rank::Unit | Rank::Generic => {}
			}
		}

		match self.output_rank {
			Rank::Bare => reasons.push("bare output".to_string()),
			Rank::List => reasons.push("List output".to_string()),
			Rank::Unit => reasons.push("unit output".to_string()),
			Rank::Item | Rank::Generic => {}
		}

		reasons
	}
}

/// Collects the input and output type signatures of every registered proto node, joining metadata with the registry's monomorphizations.
fn collect_signatures() -> Vec<NodeSignature> {
	let nodes = graphene_std::registry::NODE_METADATA.lock().unwrap();
	let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();

	let mut signatures = Vec::new();
	for (id, metadata) in nodes.iter() {
		let Some(implementations) = node_registry.get(id) else { continue };

		// Gather the distinct type of each input across every monomorphization, preserving the rank wrapper
		let mut input_types = vec![Vec::new(); metadata.fields.len()];
		for (_, node_io) in implementations.iter() {
			for (index, ty) in node_io.inputs.iter().enumerate() {
				let name = nested_name(ty);
				if !input_types[index].contains(&name) {
					input_types[index].push(name);
				}
			}
		}

		// The primary is the first argument field; `#[scope]` environment fields may precede it
		let is_scope = |field: &&FieldMetadata| matches!(field.value_source, RegistryValueSource::Scope(_));
		let primary_index = metadata.fields.iter().position(|field| !is_scope(&field));

		let inputs = metadata
			.fields
			.iter()
			.enumerate()
			// A `#[scope]` field is environment plumbing injected by the compiler, not a rankable document wire
			.filter(|(_, field)| !is_scope(field))
			.map(|(index, field)| {
				let types = input_types.get(index).cloned().unwrap_or_default();
				let rank = aggregate_rank(implementations.iter().filter_map(|(_, node_io)| node_io.inputs.get(index)));
				InputSignature {
					name: field.name.to_string(),
					is_primary: Some(index) == primary_index,
					hidden: field.hidden,
					types,
					rank,
				}
			})
			.collect::<Vec<_>>();

		let mut output_types = Vec::new();
		for (_, node_io) in implementations.iter() {
			let name = nested_name(&node_io.return_value);
			if !output_types.contains(&name) {
				output_types.push(name);
			}
		}
		let output_rank = aggregate_rank(implementations.iter().map(|(_, node_io)| &node_io.return_value));

		signatures.push(NodeSignature {
			identifier: id.as_str().to_string(),
			display_name: metadata.display_name.to_string(),
			category: metadata.category.to_string(),
			inputs,
			output_types,
			output_rank,
			inject_scope: metadata.inject_scope,
		});
	}

	signatures.sort_by_key(|sig| (sig.category.clone(), sig.display_name.clone()));
	signatures
}

fn format_types(types: &[String]) -> String {
	if types.is_empty() { "*none*".to_string() } else { types.join(" | ") }
}

/// Prints a full signature listing of every node followed by a report of every node that is not purely `Item`-ranked.
pub fn print_signature_report() {
	let signatures = collect_signatures();

	println!("# FULL NODE SIGNATURE LISTING ({} nodes)\n", signatures.len());
	for sig in &signatures {
		let category = if sig.category.is_empty() { "<hidden>" } else { &sig.category };
		println!("[{}] {} :: {}", category, sig.display_name, sig.identifier);
		for input in &sig.inputs {
			let primary = if input.is_primary { " (primary)" } else { "" };
			let hidden = if input.hidden { " (hidden)" } else { "" };
			println!("    in  [{}]{}{} {} = {}", input.rank.tag(), primary, hidden, input.name, format_types(&input.types));
		}
		println!("    out [{}] {}", sig.output_rank.tag(), format_types(&sig.output_types));
	}

	let impure = signatures.iter().filter(|sig| !sig.impure_reasons().is_empty()).collect::<Vec<_>>();

	println!("\n\n# NON-PURELY-Item NODES ({} of {})\n", impure.len(), signatures.len());
	let mut current_category = None;
	for sig in &impure {
		let category = if sig.category.is_empty() { "<hidden>".to_string() } else { sig.category.clone() };
		if current_category.as_ref() != Some(&category) {
			println!("\n## {category}\n");
			current_category = Some(category);
		}

		println!("- {} :: {}", sig.display_name, sig.identifier);
		println!("  reasons: {}", sig.impure_reasons().join(", "));
		for input in &sig.inputs {
			let flag = matches!(input.rank, Rank::Bare | Rank::List);
			let marker = if flag { " <==" } else { "" };
			let primary = if input.is_primary { " (primary)" } else { "" };
			println!("    in  [{}]{} {} = {}{}", input.rank.tag(), primary, input.name, format_types(&input.types), marker);
		}
		let output_flag = matches!(sig.output_rank, Rank::Bare | Rank::List | Rank::Unit);
		let marker = if output_flag { " <==" } else { "" };
		println!("    out [{}] {}{}", sig.output_rank.tag(), format_types(&sig.output_types), marker);
	}
}
