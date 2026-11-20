use crate::document::value::TaggedValue;
use crate::document::*;
use core_types::{Cow, Type};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Every table entry, each listed after the entries its nodes name. O(entries + nodes).
fn definitions_children_first(definitions: &FxHashMap<NetworkId, NodeNetwork>) -> Vec<NetworkId> {
	fn visit_network(network: &NodeNetwork, definitions: &FxHashMap<NetworkId, NodeNetwork>, visited: &mut FxHashSet<NetworkId>, order: &mut Vec<NetworkId>) {
		for node in network.nodes.values() {
			match &node.implementation {
				DocumentNodeImplementation::Definition(id) => visit_definition(*id, definitions, visited, order),
				DocumentNodeImplementation::Network(nested) => visit_network(nested, definitions, visited, order),
				_ => {}
			}
		}
	}
	fn visit_definition(id: NetworkId, definitions: &FxHashMap<NetworkId, NodeNetwork>, visited: &mut FxHashSet<NetworkId>, order: &mut Vec<NetworkId>) {
		if !visited.insert(id) {
			return;
		}
		if let Some(definition) = definitions.get(&id) {
			visit_network(definition, definitions, visited, order);
		}
		order.push(id);
	}

	let mut ids: Vec<NetworkId> = definitions.keys().copied().collect();
	ids.sort();

	let mut visited = FxHashSet::default();
	let mut order = Vec::with_capacity(ids.len());
	for id in ids {
		visit_definition(id, definitions, &mut visited, &mut order);
	}
	order
}

impl NodeNetwork {
	/// Resolves each scope input to the node injecting it, adding imports to carry it into nested networks.
	///
	/// Each table entry is resolved once per scope context, children first: the keys an entry does not inject become its imports,
	/// and every node naming the entry gets a matching scope input that its own network then resolves, so the nearest injection wins.
	/// An entry reached from contexts that give a key different types is specialized into one entry per context.
	pub fn resolve_scope_inputs(&mut self) {
		let mut free_keys = FxHashMap::default();
		for id in definitions_children_first(&self.definitions) {
			let Some(definition) = self.definitions.get(&id) else { continue };
			let keys = definition.free_scope_keys(&free_keys);
			free_keys.insert(id, keys);
		}

		let originals = std::mem::take(&mut self.definitions);
		let mut specializer = ScopeSpecializer {
			originals: &originals,
			free_keys: &free_keys,
			base_signatures: FxHashMap::default(),
			contexts: FxHashMap::default(),
			specialized: FxHashMap::default(),
		};
		let root_chain = self.scope_injections.clone();
		specializer.specialize(self, &root_chain);
		let ScopeSpecializer {
			contexts,
			specialized: mut definitions,
			..
		} = specializer;

		let mut import_counts = FxHashMap::default();
		for network in std::iter::once(&*self).chain(definitions.values()) {
			network.collect_instance_input_counts(&mut import_counts);
		}

		let mut forwarded: FxHashMap<NetworkId, Vec<NodeInput>> = FxHashMap::default();
		for id in definitions_children_first(&definitions) {
			let (Some(definition), Some(context)) = (definitions.get_mut(&id), contexts.get(&id)) else {
				continue;
			};
			definition.append_forwarded_scopes(&forwarded);

			// Stands in for an instance's inputs, so the forwarded keys become imports after the existing ones
			let import_count = import_counts.get(&id).copied().unwrap_or_default();
			let mut instance_inputs = vec![NodeInput::value(TaggedValue::None, false); import_count];
			let outside = ScopeChain { scopes: context, parent: None };
			definition.resolve_scope_inputs_impl(Some(&outside), &mut instance_inputs);
			forwarded.insert(id, instance_inputs.split_off(import_count));
		}

		self.append_forwarded_scopes(&forwarded);
		self.definitions = definitions;

		let mut leftover = Vec::new();
		self.resolve_scope_inputs_impl(None, &mut leftover);
		assert!(leftover.is_empty(), "Unresolved scope keys at top level: {leftover:?}");
	}

	/// The scope keys this network reads, directly or through the networks it contains, without injecting them itself.
	fn free_scope_keys(&self, entry_keys: &FxHashMap<NetworkId, FxHashSet<String>>) -> FxHashSet<String> {
		let mut keys = FxHashSet::default();
		for node in self.nodes.values() {
			keys.extend(node.inputs.iter().filter_map(|input| match input {
				NodeInput::Scope(key) => Some(key.to_string()),
				_ => None,
			}));
			match &node.implementation {
				DocumentNodeImplementation::Definition(id) => keys.extend(entry_keys.get(id).into_iter().flatten().cloned()),
				DocumentNodeImplementation::Network(nested) => keys.extend(nested.free_scope_keys(entry_keys)),
				_ => {}
			}
		}

		keys.retain(|key| !self.scope_injections.contains_key(key));
		keys
	}

	fn collect_instance_input_counts(&self, counts: &mut FxHashMap<NetworkId, usize>) {
		for node in self.nodes.values() {
			match &node.implementation {
				DocumentNodeImplementation::Definition(id) => {
					counts.entry(*id).or_insert(node.inputs.len());
				}
				DocumentNodeImplementation::Network(nested) => nested.collect_instance_input_counts(counts),
				_ => {}
			}
		}
	}

	fn append_forwarded_scopes(&mut self, forwarded: &FxHashMap<NetworkId, Vec<NodeInput>>) {
		for node in self.nodes.values_mut() {
			match &mut node.implementation {
				DocumentNodeImplementation::Definition(id) => node.inputs.extend(forwarded.get(id).into_iter().flatten().cloned()),
				DocumentNodeImplementation::Network(nested) => nested.append_forwarded_scopes(forwarded),
				_ => {}
			}
		}
	}

	fn resolve_scope_inputs_impl(&mut self, parent: Option<&ScopeChain<'_>>, network_inputs: &mut Vec<NodeInput>) {
		let scope_chain = ScopeChain {
			scopes: &self.scope_injections,
			parent,
		};

		for node in self.nodes.values_mut() {
			let DocumentNodeImplementation::Network(network) = &mut node.implementation else { continue };
			network.resolve_scope_inputs_impl(Some(&scope_chain), &mut node.inputs);
		}

		let mut key_to_idx: FxHashMap<Cow<'static, str>, usize> = FxHashMap::default();
		for node in self.nodes.values_mut() {
			for input in node.inputs.iter_mut() {
				let NodeInput::Scope(key) = input else { continue };
				*input = match scope_chain.lookup(key) {
					ScopeChainLookup::Current(node_id, _ty) => NodeInput::node(*node_id, 0),
					ScopeChainLookup::Parent(_node_id, ty) => {
						let import_index = *key_to_idx.entry(key.clone()).or_insert_with(|| {
							let index = network_inputs.len();
							network_inputs.push(NodeInput::Scope(key.clone()));
							index
						});
						NodeInput::Import {
							import_type: ty.clone(),
							import_index,
						}
					}
					ScopeChainLookup::None => panic!("Scope key `{key}` not found in any ancestor scope_injections"),
				};
			}
		}
	}
}

type ScopeInjections = FxHashMap<String, (NodeId, Type)>;

/// Gives every instance the entry matching the scope types its context provides, copying an entry for each further context.
struct ScopeSpecializer<'a> {
	originals: &'a FxHashMap<NetworkId, NodeNetwork>,
	free_keys: &'a FxHashMap<NetworkId, FxHashSet<String>>,
	/// The signature of the first context reaching each original entry, which keeps the entry's ID.
	base_signatures: FxHashMap<NetworkId, Vec<(String, Type)>>,
	/// The injections each specialized entry resolves its free keys against.
	contexts: FxHashMap<NetworkId, ScopeInjections>,
	specialized: FxHashMap<NetworkId, NodeNetwork>,
}

impl ScopeSpecializer<'_> {
	/// Points every instance in `network` at its specialization, where `chain` holds the nearest injection of every key visible in `network`.
	fn specialize(&mut self, network: &mut NodeNetwork, chain: &ScopeInjections) {
		let mut node_ids: Vec<NodeId> = network.nodes.keys().copied().collect();
		node_ids.sort();

		for node_id in node_ids {
			let Some(node) = network.nodes.get_mut(&node_id) else { continue };
			match &mut node.implementation {
				DocumentNodeImplementation::Definition(id) => *id = self.specialization(*id, chain),
				DocumentNodeImplementation::Network(nested) => {
					let mut nested_chain = chain.clone();
					nested_chain.extend(nested.scope_injections.iter().map(|(key, injection)| (key.clone(), injection.clone())));
					self.specialize(nested, &nested_chain);
				}
				_ => {}
			}
		}
	}

	fn specialization(&mut self, id: NetworkId, chain: &ScopeInjections) -> NetworkId {
		let Some(original) = self.originals.get(&id) else { return id };

		let context: ScopeInjections = self.free_keys.get(&id).into_iter().flatten().filter_map(|key| Some((key.clone(), chain.get(key)?.clone()))).collect();
		let mut signature: Vec<(String, Type)> = context.iter().map(|(key, (_, ty))| (key.clone(), ty.clone())).collect();
		signature.sort_by(|(a, _), (b, _)| a.cmp(b));

		let base_signature = self.base_signatures.entry(id).or_insert_with(|| signature.clone());
		let specialized_id = if *base_signature == signature { id } else { specialized_network_id(id, &signature) };
		if self.contexts.contains_key(&specialized_id) {
			return specialized_id;
		}

		let mut body = original.clone();
		let mut inner_chain = context.clone();
		inner_chain.extend(body.scope_injections.iter().map(|(key, injection)| (key.clone(), injection.clone())));
		self.contexts.insert(specialized_id, context);

		self.specialize(&mut body, &inner_chain);
		self.specialized.insert(specialized_id, body);
		specialized_id
	}
}

fn specialized_network_id(id: NetworkId, signature: &[(String, Type)]) -> NetworkId {
	let mut hasher = DefaultHasher::new();
	"scope specialization".hash(&mut hasher);
	id.hash(&mut hasher);
	signature.hash(&mut hasher);
	NetworkId(hasher.finish())
}

struct ScopeChain<'a> {
	scopes: &'a ScopeInjections,
	parent: Option<&'a ScopeChain<'a>>,
}
enum ScopeChainLookup<'a> {
	Current(&'a NodeId, &'a Type),
	Parent(&'a NodeId, &'a Type),
	None,
}
impl ScopeChain<'_> {
	fn lookup(&'_ self, key: &str) -> ScopeChainLookup<'_> {
		self.scopes
			.get(key)
			.map(|(id, ty)| ScopeChainLookup::Current(id, ty))
			.or_else(|| {
				self.parent.and_then(|parent| match parent.lookup(key) {
					ScopeChainLookup::Current(id, ty) | ScopeChainLookup::Parent(id, ty) => Some(ScopeChainLookup::Parent(id, ty)),
					ScopeChainLookup::None => None,
				})
			})
			.unwrap_or(ScopeChainLookup::None)
	}
}
