use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};
use graphene_std::{CacheHash, simplify_identifier_name};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::error::Error;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Format {
	Text,
	Dot,
}

pub type Document = HashMap<Vec<NodeId>, (String, Option<ProtoNodeIdentifier>)>;

pub fn document(network: &NodeNetwork, mut names: HashMap<Vec<NodeId>, String>) -> Document {
	network
		.recursive_nodes()
		.map(|(id, node, mut path)| {
			path.push(*id);
			let identifier = node.implementation.get_proto_node().cloned();
			let name = names.remove(&path).or_else(|| identifier.as_ref().map(|identifier| simplify_identifier_name(identifier.as_str())));
			(path, (name.unwrap_or_else(|| id.to_string()), identifier))
		})
		.collect()
}

pub fn legacy_names(document: &str) -> HashMap<Vec<NodeId>, String> {
	fn collect(network: &serde_json::Value, path: &[NodeId], names: &mut HashMap<Vec<NodeId>, String>) {
		for entry in network["persistent_metadata"]["node_metadata"].as_array().into_iter().flatten() {
			let Some(id) = entry[0].as_u64() else { continue };
			let path = [path, &[NodeId(id)]].concat();
			let metadata = &entry[1]["persistent_metadata"];
			let nested = &metadata["network_metadata"];
			collect(nested, &path, names);
			let name = metadata["display_name"].as_str().filter(|name| !name.is_empty());
			if let Some(name) = name.or(nested["persistent_metadata"]["reference"].as_str()) {
				names.insert(path, name.to_string());
			}
		}
	}

	let mut names = HashMap::new();
	if let Ok(document) = serde_json::from_str::<serde_json::Value>(document) {
		collect(&document["network_interface"]["network_metadata"], &[], &mut names);
	}
	names
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
	Document,
	Value,
	Convert,
	Nullify,
	Expand,
	Scope,
}

impl Kind {
	fn tag(self) -> &'static str {
		match self {
			Kind::Document => "",
			Kind::Value => "+value",
			Kind::Convert => "+convert",
			Kind::Nullify => "+nullify",
			Kind::Expand => "+expand",
			Kind::Scope => "+scope",
		}
	}
}

struct Node<'a> {
	proto: &'a ProtoNode,
	kind: Kind,
	origins: Vec<&'a [NodeId]>,
	inputs: Vec<usize>,
	consumers: Vec<usize>,
	constant: bool,
	id: u64,
}

pub fn print(network: &ProtoNetwork, document: &Document, format: Format, upstream: Option<&str>, downstream: Option<&str>, depth: Option<usize>) -> Result<(), Box<dyn Error>> {
	let mut protos: HashMap<NodeId, (&ProtoNode, Vec<&[NodeId]>)> = HashMap::new();
	for (id, proto) in &network.nodes {
		let path = proto.original_location.path.as_deref().unwrap_or_default();
		let origin = (1..=path.len()).rev().map(|n| &path[..n]).find(|prefix| document.contains_key(*prefix)).unwrap_or_default();
		let (_, origins) = protos.entry(*id).or_insert((proto, Vec::new()));
		if !origins.contains(&origin) {
			origins.push(origin);
		}
	}

	let order = post_order(std::iter::once(network.output).chain(network.nodes.iter().map(|(id, _)| *id)), |id| inputs(protos[&id].0));
	let position: HashMap<_, _> = order.iter().enumerate().map(|(position, id)| (*id, position)).collect();
	let mut nodes: Vec<Node> = Vec::with_capacity(order.len());
	for (n, id) in order.iter().enumerate() {
		let (proto, origins) = protos.remove(id).unwrap();
		let inputs: Vec<_> = inputs(proto).iter().map(|input| position[input]).collect();
		for &input in &inputs {
			nodes[input].consumers.push(n);
		}
		let kind = kind(proto, origins[0], document);
		let constant = matches!(kind, Kind::Value | Kind::Convert | Kind::Nullify) && inputs.iter().all(|&input| nodes[input].constant);
		nodes.push(Node {
			proto,
			kind,
			origins,
			inputs,
			consumers: Vec::new(),
			constant,
			id: 0,
		});
	}

	let mut occurrences = HashMap::new();
	for n in 0..nodes.len() {
		let node = &nodes[n];
		let mut hasher = DefaultHasher::new();
		node.proto.identifier.as_str().hash(&mut hasher);
		node.proto.call_argument.to_string().hash(&mut hasher);
		match &node.proto.construction_args {
			_ if node.origins.len() == 1 && node.kind != Kind::Nullify => {
				node.origins[0].hash(&mut hasher);
				node.proto.original_location.auto_convert_index.hash(&mut hasher);
				if let Some(&consumer) = node.consumers.first().filter(|_| node.kind == Kind::Value) {
					nodes[consumer].proto.original_location.auto_convert_index.hash(&mut hasher);
					nodes[consumer].inputs.iter().position(|&input| input == n).hash(&mut hasher);
				}
			}
			ConstructionArgs::Nodes(_) => node.inputs.iter().for_each(|&input| nodes[input].id.hash(&mut hasher)),
			ConstructionArgs::Value(value) if matches!(**value, TaggedValue::EditorApi(_) | TaggedValue::Runtime(_)) => {}
			ConstructionArgs::Value(value) => value.cache_hash(&mut hasher),
			ConstructionArgs::Inline(inline) => inline.expr.hash(&mut hasher),
		}
		let key = hasher.finish();
		let occurrence = occurrences.entry(key).or_insert(0_u64);
		let mut hasher = DefaultHasher::new();
		(key, *occurrence).hash(&mut hasher);
		*occurrence += 1;
		nodes[n].id = hasher.finish();
	}

	let output = position[&network.output];
	let width = (8..16)
		.find(|&width| nodes.iter().map(|node| node.id >> (64 - 4 * width)).collect::<HashSet<_>>().len() == nodes.len())
		.unwrap_or(16);
	let short = |n: usize| format!("{:0width$x}", nodes[n].id >> (64 - 4 * width));
	let location = |path: &[NodeId]| (1..=path.len()).map(|n| document[&path[..n]].0.as_str()).collect::<Vec<_>>().join("/");

	let matching = |query: &str| {
		let located = |origin: &&[NodeId]| !origin.is_empty() && location(origin).contains(query);
		[false, true]
			.into_iter()
			.map(|shared| {
				(0..nodes.len())
					.filter(|&n| {
						let node = &nodes[n];
						format!("{:016x}", node.id).starts_with(query)
							|| node.proto.identifier.as_str().contains(query)
							|| if shared { node.origins.iter().any(located) } else { node.origins.iter().all(located) }
					})
					.collect::<Vec<_>>()
			})
			.find(|matches| !matches.is_empty())
			.ok_or_else(|| format!("No proto node matches \"{query}\""))
	};
	let mut selected = vec![upstream.is_none() && downstream.is_none(); nodes.len()];
	if let Some(query) = upstream {
		reach(&nodes, matching(query)?, depth, |node| &node.inputs, &mut selected);
	}
	if let Some(query) = downstream {
		reach(&nodes, matching(query)?, depth, |node| &node.consumers, &mut selected);
	}

	let mut out = std::io::BufWriter::new(std::io::stdout().lock());
	match format {
		Format::Text => {
			for (n, node) in nodes.iter().enumerate().filter(|(n, _)| selected[*n]) {
				let arguments = match &node.proto.construction_args {
					ConstructionArgs::Nodes(_) => format!("({})", node.inputs.iter().map(|&input| short(input)).collect::<Vec<_>>().join(", ")),
					ConstructionArgs::Value(value) => format!("{{{}}}", summary(value, 80)),
					ConstructionArgs::Inline(inline) => format!("{{{}}}", inline.expr),
				};
				let shared = if node.origins.len() > 1 { "(shared)" } else { "" };
				let provenance = [node.kind.tag(), &location(node.origins[0]), shared]
					.into_iter()
					.filter(|part| !part.is_empty())
					.collect::<Vec<_>>()
					.join(" ");
				let identifier = simplify_identifier_name(node.proto.identifier.as_str());
				writeln!(out, "{} = {identifier}{arguments} : {}  # {provenance}", short(n), node.proto.call_argument)?;
			}
		}
		Format::Dot => {
			let metadata = graphene_std::registry::NODE_METADATA.lock().unwrap();
			let field = |node: &Node, index: usize| {
				let field = metadata.get(&node.proto.identifier).and_then(|metadata| metadata.fields.get(index));
				field.map_or_else(|| index.to_string(), |field| field.name.trim().to_string())
			};
			let drawn = |n: usize| selected[n] && !nodes[n].constant;

			let mut networks: BTreeMap<&[NodeId], Vec<usize>> = BTreeMap::new();
			for n in (0..nodes.len()).filter(|&n| drawn(n)) {
				let network = match nodes[n].origins.as_slice() {
					[origin] => origin.split_last().map_or(&[][..], |(_, parent)| parent),
					_ => &[],
				};
				networks.entry(network).or_default().push(n);
			}

			writeln!(out, "digraph proto {{")?;
			writeln!(out, "\trankdir=LR")?;
			writeln!(out, "\tnode [shape=box fontname=monospace]")?;
			for (cluster, (path, members)) in networks.iter().enumerate() {
				let indent = if path.is_empty() { "\t" } else { "\t\t" };
				if !path.is_empty() {
					writeln!(out, "\tsubgraph cluster_{cluster} {{")?;
					writeln!(out, "\t\tlabel={:?}", location(path))?;
				}
				for &n in members {
					let node = &nodes[n];
					let identifier = simplify_identifier_name(node.proto.identifier.as_str());
					let name = document.get(node.origins[0]).map_or("", |(name, _)| name.as_str());
					let name = if name == identifier { "" } else { name };
					let mut label = vec![identifier, format!("{} : {}", short(n), node.proto.call_argument)];
					let provenance = [node.kind.tag(), name].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ");
					label.extend((!provenance.is_empty()).then_some(provenance));
					for (index, &input) in node.inputs.iter().enumerate().filter(|(_, input)| nodes[**input].constant) {
						label.extend(constant(&nodes, input).map(|value| format!("{}: {}", field(node, index), summary(value, 40))));
					}
					let inserted = if node.kind == Kind::Document { "" } else { " style=\"filled,dashed\" fillcolor=lightgrey" };
					let output = if n == output { " peripheries=2" } else { "" };
					writeln!(out, "{indent}{:?} [label={:?}{inserted}{output}]", short(n), label.join("\n"))?;
				}
				if !path.is_empty() {
					writeln!(out, "\t}}")?;
				}
			}

			let mut stubs = HashSet::new();
			for &n in networks.values().flatten() {
				for (index, &input) in nodes[n].inputs.iter().enumerate().filter(|(_, input)| !nodes[**input].constant) {
					if !selected[input] && stubs.insert(input) {
						let label = format!("{}\n{}", simplify_identifier_name(nodes[input].proto.identifier.as_str()), short(input));
						writeln!(out, "\t{:?} [label={label:?} style=dotted]", short(input))?;
					}
					writeln!(out, "\t{:?} -> {:?} [headlabel={:?}]", short(input), short(n), field(&nodes[n], index))?;
				}
			}
			writeln!(out, "}}")?;
		}
	}
	out.flush()?;
	Ok(())
}

fn kind(proto: &ProtoNode, origin: &[NodeId], document: &Document) -> Kind {
	let path = proto.original_location.path.as_deref().unwrap_or_default();
	let appended = path.len() > origin.len();
	match &proto.construction_args {
		ConstructionArgs::Value(value) if matches!(**value, TaggedValue::ContextModification(_)) => Kind::Nullify,
		_ if appended && path.last() == Some(&NodeId(10)) => Kind::Nullify,
		_ if proto.original_location.auto_convert_index.is_some() => Kind::Convert,
		ConstructionArgs::Value(_) => Kind::Value,
		_ if origin.is_empty() => Kind::Scope,
		_ if !appended && document[origin].1.as_ref() == Some(&proto.identifier) => Kind::Document,
		_ => Kind::Expand,
	}
}

fn inputs(node: &ProtoNode) -> &[NodeId] {
	match &node.construction_args {
		ConstructionArgs::Nodes(ids) => ids,
		_ => &[],
	}
}

fn constant<'a>(nodes: &[Node<'a>], mut n: usize) -> Option<&'a TaggedValue> {
	loop {
		match &nodes[n].proto.construction_args {
			ConstructionArgs::Value(value) => return Some(&**value),
			ConstructionArgs::Nodes(_) => n = *nodes[n].inputs.first()?,
			ConstructionArgs::Inline(_) => return None,
		}
	}
}

fn post_order<'a>(roots: impl IntoIterator<Item = NodeId>, inputs: impl Fn(NodeId) -> &'a [NodeId]) -> Vec<NodeId> {
	let mut order = Vec::new();
	let mut visited = HashSet::new();
	for root in roots {
		if !visited.insert(root) {
			continue;
		}
		let mut stack = vec![(root, 0)];
		while let Some((id, next)) = stack.last_mut() {
			match inputs(*id).get(*next) {
				Some(&input) => {
					*next += 1;
					if visited.insert(input) {
						stack.push((input, 0));
					}
				}
				None => {
					order.push(*id);
					stack.pop();
				}
			}
		}
	}
	order
}

fn reach<'a>(nodes: &'a [Node], seeds: Vec<usize>, depth: Option<usize>, edges: impl Fn(&'a Node) -> &'a [usize], selected: &mut [bool]) {
	let mut distances = vec![None; nodes.len()];
	let mut queue = VecDeque::new();
	for n in seeds {
		distances[n] = Some(0);
		queue.push_back((n, 0));
	}
	while let Some((n, distance)) = queue.pop_front() {
		selected[n] = true;
		if distances[n] < Some(distance) {
			continue;
		}
		for &next in edges(&nodes[n]) {
			let cost = usize::from(matches!(nodes[next].kind, Kind::Document | Kind::Scope));
			let distance = distance + cost;
			if depth.is_some_and(|depth| distance > depth) || distances[next].is_some_and(|known| known <= distance) {
				continue;
			}
			distances[next] = Some(distance);
			if cost == 0 { queue.push_front((next, distance)) } else { queue.push_back((next, distance)) }
		}
	}
}

fn summary(value: &TaggedValue, limit: usize) -> String {
	let text = match value {
		TaggedValue::ContextModification(modification) if modification.features.is_empty() => "keep nothing".to_string(),
		TaggedValue::ContextModification(modification) => format!("keep {}", modification.features.iter_names().map(|(name, _)| name).collect::<Vec<_>>().join(" | ")),
		value => format!("{value:?}"),
	};
	let text = match text.split_once('(') {
		Some((variant, inner)) if inner.strip_prefix(variant).is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_')) => inner.strip_suffix(')').unwrap_or(inner).to_string(),
		_ => text,
	};
	match text.char_indices().nth(limit) {
		Some((index, _)) => format!("{}…", &text[..index]),
		None => text,
	}
}
