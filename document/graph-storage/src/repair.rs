//! Repairs the graph after steps are replayed onto a line that changed since they were made, as bringing a step back
//! or removing one does. Replayed writes keep their stamps, so a slot written since keeps the newer value, which can
//! leave an input naming a removed node, two nodes on one, or a node nothing reaches any more.
//!
//! Nodes connected through their primary inputs form chains, like a stack of layers or a node feeding the next. Each
//! chain hangs off a head slot: an export, or an input past the primary one. The repair rebuilds every chain from the
//! order the line had before the replay and the order the replayed step left it in when it was made, so the step's
//! own changes land on the chains as they are now.

use crate::{NetworkId, NodeId, NodeInput, Registry, RegistryDelta};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Each node's network and newest primary input anywhere in history, for nodes this line no longer or never had.
pub(crate) type KnownInputs = HashMap<NodeId, (NetworkId, NodeInput)>;

/// A slot that can name a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Slot {
	Input(NodeId, u32),
	Export(NetworkId, u32),
}

/// A chain's nodes from its head down, and what the last one rests on.
struct Chain {
	head: Slot,
	nodes: Vec<NodeId>,
	bottom: Option<NodeInput>,
}

/// Which of a dropped step's ops the line still needs, and the nodes they add. `registry` is the line with the step.
/// A subgraph the step added stays, with what the step put in it, while a node it didn't add is built from it, as a
/// later layer reuses the one the first layer brought. Resources stay too, since what refers to them isn't tracked.
pub(crate) fn still_needed(registry: &Registry, dropped: &[RegistryDelta]) -> (Vec<usize>, HashSet<NodeId>) {
	let added_networks: HashSet<NetworkId> = dropped
		.iter()
		.filter_map(|op| match op {
			RegistryDelta::AddNetwork { id, .. } => Some(*id),
			_ => None,
		})
		.collect();
	let added_nodes: HashSet<NodeId> = dropped
		.iter()
		.filter_map(|op| match op {
			RegistryDelta::AddNode { id, .. } => Some(*id),
			_ => None,
		})
		.collect();
	let built_from = |node: &crate::Node| match node.implementation() {
		crate::Implementation::Network(network) if added_networks.contains(network) => Some(*network),
		_ => None,
	};

	let mut needed: HashSet<NetworkId> = registry
		.node_instances
		.iter()
		.filter(|(id, _)| !added_nodes.contains(id))
		.filter_map(|(_, node)| built_from(node))
		.collect();
	// A needed subgraph's own nodes need the subgraphs they are built from
	loop {
		let nested: Vec<NetworkId> = registry
			.node_instances
			.values()
			.filter(|node| needed.contains(&node.network()))
			.filter_map(|node| built_from(node))
			.filter(|network| !needed.contains(network))
			.collect();
		if nested.is_empty() {
			break;
		}
		needed.extend(nested);
	}

	let kept_nodes: HashSet<NodeId> = registry.node_instances.iter().filter(|(_, node)| needed.contains(&node.network())).map(|(&id, _)| id).collect();
	let kept = dropped
		.iter()
		.enumerate()
		.filter(|(_, op)| match op {
			RegistryDelta::AddNetwork { id, .. } | RegistryDelta::SetNetworkExport { id, .. } | RegistryDelta::ChangeNetworkAttribute { id, .. } => needed.contains(id),
			RegistryDelta::AddNode { id, .. }
			| RegistryDelta::ChangeNodeInput { id, .. }
			| RegistryDelta::SetNodeInputs { id, .. }
			| RegistryDelta::SetNodeImplementation { id, .. }
			| RegistryDelta::ChangeNodeAttribute { id, .. }
			| RegistryDelta::ChangeNodeInputAttribute { id, .. } => kept_nodes.contains(id),
			RegistryDelta::AddResource { .. } | RegistryDelta::SetResourceHash { .. } | RegistryDelta::AddSource { .. } => true,
			_ => false,
		})
		.map(|(index, _)| index)
		.collect();
	(kept, kept_nodes)
}

/// The ops that repair `registry`, which is `before` with `replayed` folded in. `intended` is the document as the
/// replayed step left it when it was made, and `gone` the nodes it must not have, as those the step deleted.
/// - A node in `gone`, or in neither `before` nor `intended`, is removed. A newer write to a node revives it over an
///   older removal, as one made on the line since does over a deletion brought back.
/// - Every chain keeps the nodes it had before, in their order, except those the replayed step moved, which take
///   their place from `intended`, after the node they followed there. A node the step added lands the same way.
/// - A node on no chain any more, which was on one, goes back where it was.
/// - An input naming a removed node takes that node's primary input instead, as deleting a node reconnects what it
///   sat between. A node this line never had counts as removed, with the primary input `known` gives it.
pub(crate) fn repair_ops(before: &Registry, registry: &Registry, intended: &Registry, replayed: &[RegistryDelta], gone: &HashSet<NodeId>, known: &KnownInputs) -> Vec<RegistryDelta> {
	let mut registry = registry.clone();
	let mut ops = Vec::new();

	let mut unwanted: Vec<NodeId> = registry
		.node_instances
		.keys()
		.filter(|id| gone.contains(id) || (!before.node_instances.contains_key(id) && !intended.node_instances.contains_key(id)))
		.copied()
		.collect();
	unwanted.sort();
	for id in unwanted {
		if let Some(snapshot) = registry.node_instances.remove(&id) {
			ops.push(RegistryDelta::RemoveNode { id, snapshot });
		}
	}
	restore_subgraphs(&mut registry, intended, &mut ops);

	let live = |id: &NodeId| registry.node_instances.contains_key(id);
	let placed_by_step: HashSet<NodeId> = replayed.iter().flat_map(placed_nodes).filter(live).collect();

	let heads: BTreeSet<Slot> = [before, &registry, intended].into_iter().flat_map(head_slots).filter(|&slot| owner_is_live(&registry, slot)).collect();
	let was_in = |registry: &Registry| -> HashMap<NodeId, Slot> { heads.iter().flat_map(|&head| chain(registry, head).nodes.into_iter().map(move |id| (id, head))).collect() };
	let (was_before, was_intended) = (was_in(before), was_in(intended));
	// Where each node goes: where the step put it if it placed it, else where it was
	let home = |id: &NodeId| match placed_by_step.contains(id) {
		true => was_intended.get(id).or(was_before.get(id)),
		false => was_before.get(id).or(was_intended.get(id)),
	};

	let mut chains = Vec::new();
	let mut taken = HashSet::new();
	for &head in &heads {
		let owner = match head {
			Slot::Input(owner, _) => Some(owner),
			Slot::Export(..) => None,
		};
		let fits = |id: &NodeId| live(id) && Some(*id) != owner && home(id) == Some(&head);
		let earlier = chain(before, head);
		let wanted = chain(intended, head);
		// A node the step placed counts as moved when it sits on the other side of a node both orders have
		let common: Vec<NodeId> = earlier.nodes.iter().copied().filter(|id| live(id) && wanted.nodes.contains(id)).collect();
		let rank = |order: &[NodeId], id: &NodeId| order.iter().position(|node| node == id);
		let moved = |id: &NodeId| {
			placed_by_step.contains(id)
				&& common.contains(id)
				&& common
					.iter()
					.any(|other| other != id && (rank(&earlier.nodes, id) < rank(&earlier.nodes, other)) != (rank(&wanted.nodes, id) < rank(&wanted.nodes, other)))
		};
		let mut nodes: Vec<NodeId> = earlier.nodes.iter().copied().filter(|id| fits(id) && !moved(id)).collect();
		for (position, id) in wanted.nodes.iter().enumerate() {
			if nodes.contains(id) || !fits(id) {
				continue;
			}
			let after = wanted.nodes[..position].iter().rev().find_map(|above| nodes.iter().position(|node| node == above));
			nodes.insert(after.map_or(0, |index| index + 1), *id);
		}
		// What the chain rests on, as long as nothing else sits on it. A chain of nodes rests on what the last node rested
		// on in an order that had nodes too, since an empty one holds the head slot's own value, of another type.
		let alike = |order: &Chain| order.nodes.is_empty() == nodes.is_empty();
		let mut orders = [&earlier, &wanted];
		orders.sort_by_key(|order| !alike(order));
		let bottom = orders.into_iter().filter_map(|order| order.bottom.clone()).find(|bottom| match bottom {
			NodeInput::Node { id, .. } => live(id) && home(id).is_none(),
			_ => true,
		});
		taken.extend(nodes.iter().copied());
		chains.push(Chain { head, nodes, bottom });
	}

	for chain in &chains {
		let mut slot = chain.head;
		for &id in &chain.nodes {
			set(&mut registry, &mut ops, slot, Some(on(id)));
			slot = Slot::Input(id, 0);
		}
		if let Some(bottom) = &chain.bottom {
			set(&mut registry, &mut ops, slot, Some(bottom.clone()));
		} else if let Some(NodeInput::Node { id, .. }) = slot_value(&registry, slot)
			&& (taken.contains(&id) || !registry.node_instances.contains_key(&id))
		{
			// Resting on a node another chain holds, or one gone, it rests on nothing instead
			let cleared = match slot {
				Slot::Export(..) => None,
				Slot::Input(..) => reconnect(&registry, id, slot_network(&registry, slot), known).filter(|input| !matches!(input, NodeInput::Node { .. })),
			};
			if cleared.is_some() || matches!(slot, Slot::Export(..)) {
				set(&mut registry, &mut ops, slot, cleared);
			}
		}
	}

	// Inputs off every chain still naming a removed node
	for (slot, named) in slots(&registry) {
		if registry.node_instances.contains_key(&named) {
			continue;
		}
		let reconnected = reconnect(&registry, named, slot_network(&registry, slot), known).or_else(|| slot_value(before, slot).filter(|earlier| !names_missing(&registry, earlier)));
		if reconnected.is_some() || matches!(slot, Slot::Export(..)) {
			set(&mut registry, &mut ops, slot, reconnected);
		}
	}
	ops
}

/// Gives a node built from a subgraph the line lacks that subgraph back as `intended` has it, with what is in it, as
/// a step brought back onto a line without the earlier step that added it needs.
fn restore_subgraphs(registry: &mut Registry, intended: &Registry, ops: &mut Vec<RegistryDelta>) {
	let lacking = |registry: &Registry, node: &crate::Node| match node.implementation() {
		crate::Implementation::Network(network) if !registry.networks.contains_key(network) && intended.networks.contains_key(network) => Some(*network),
		_ => None,
	};
	let mut missing: BTreeSet<NetworkId> = registry.node_instances.values().filter_map(|node| lacking(registry, node)).collect();
	while let Some(network) = missing.pop_first() {
		let content = intended.networks[&network].clone();
		registry.networks.insert(network, content.clone());
		ops.push(RegistryDelta::AddNetwork { id: network, network: content });

		let mut inside: Vec<(&NodeId, &crate::Node)> = intended
			.node_instances
			.iter()
			.filter(|(id, node)| node.network() == network && !registry.node_instances.contains_key(id))
			.collect();
		inside.sort_by_key(|(id, _)| **id);
		for (&id, node) in inside {
			missing.extend(lacking(registry, node));
			registry.node_instances.insert(id, node.clone());
			ops.push(RegistryDelta::AddNode { id, node: node.clone() });
		}
	}
}

fn on(id: NodeId) -> NodeInput {
	NodeInput::Node { id, index: 0 }
}

fn names_missing(registry: &Registry, input: &NodeInput) -> bool {
	matches!(input, NodeInput::Node { id, .. } if !registry.node_instances.contains_key(id))
}

/// The nodes a write places in a slot, or adds.
fn placed_nodes(op: &RegistryDelta) -> Vec<NodeId> {
	match op {
		RegistryDelta::AddNode { id, .. } => vec![*id],
		RegistryDelta::ChangeNodeInput { id, index, new_input } => {
			let placed = match new_input {
				NodeInput::Node { id: placed, .. } => Some(*placed),
				_ => None,
			};
			(*index == 0).then_some(*id).into_iter().chain(placed).collect()
		}
		RegistryDelta::SetNetworkExport {
			export: Some(NodeInput::Node { id, .. }),
			..
		} => vec![*id],
		_ => Vec::new(),
	}
}

/// Every slot naming a node, in a fixed order.
fn slots(registry: &Registry) -> Vec<(Slot, NodeId)> {
	let mut slots = Vec::new();
	for (&id, node) in &registry.node_instances {
		for (index, slot) in node.inputs.iter().enumerate() {
			if let NodeInput::Node { id: named, .. } = slot.input {
				slots.push((Slot::Input(id, index as u32), named));
			}
		}
	}
	for (&id, network) in &registry.networks {
		for (index, slot) in network.exports.iter().enumerate() {
			if let Some(NodeInput::Node { id: named, .. }) = slot.target {
				slots.push((Slot::Export(id, index as u32), named));
			}
		}
	}
	slots.sort();
	slots
}

/// The slots heading a chain: exports and inputs past the primary one, naming a node.
fn head_slots(registry: &Registry) -> impl Iterator<Item = Slot> + '_ {
	slots(registry).into_iter().map(|(slot, _)| slot).filter(|slot| !matches!(slot, Slot::Input(_, 0)))
}

fn owner_is_live(registry: &Registry, slot: Slot) -> bool {
	match slot {
		Slot::Input(id, _) => registry.node_instances.contains_key(&id),
		Slot::Export(id, _) => registry.networks.contains_key(&id),
	}
}

/// The chain hanging off `head` in `registry`.
fn chain(registry: &Registry, head: Slot) -> Chain {
	let mut nodes = Vec::new();
	let mut below = slot_value(registry, head);
	while let Some(NodeInput::Node { id, .. }) = below {
		if nodes.contains(&id) || !registry.node_instances.contains_key(&id) {
			return Chain { head, nodes, bottom: None };
		}
		nodes.push(id);
		below = slot_value(registry, Slot::Input(id, 0));
	}
	Chain { head, nodes, bottom: below }
}

fn slot_network(registry: &Registry, slot: Slot) -> Option<NetworkId> {
	match slot {
		Slot::Input(id, _) => registry.node_instances.get(&id).map(|node| node.network),
		Slot::Export(id, _) => Some(id),
	}
}

fn slot_value(registry: &Registry, slot: Slot) -> Option<NodeInput> {
	match slot {
		Slot::Input(id, index) => registry.node_instances.get(&id)?.inputs.get(index as usize).map(|slot| slot.input.clone()),
		Slot::Export(id, index) => registry.networks.get(&id)?.exports.get(index as usize)?.target.clone(),
	}
}

/// What a removed node's primary input led to, through removed nodes, staying in `network`. None when the chain ends
/// at a node never seen or in another network.
fn reconnect(registry: &Registry, removed: NodeId, network: Option<NetworkId>, known: &KnownInputs) -> Option<NodeInput> {
	let mut visited = HashSet::new();
	let mut current = removed;
	loop {
		if !visited.insert(current) {
			return None;
		}
		let (in_network, input) = match registry.removed_nodes.get(&current) {
			Some(tombstone) => (tombstone.content.network, tombstone.content.inputs.first()?.input.clone()),
			None => known.get(&current).cloned()?,
		};
		if Some(in_network) != network {
			return None;
		}
		match input {
			NodeInput::Node { id, .. } if !registry.node_instances.contains_key(&id) => current = id,
			NodeInput::Node { id, .. } if Some(registry.node_instances[&id].network) != network => return None,
			input => return Some(input),
		}
	}
}

/// Records the op setting `slot` to `input`, None clearing an export, and applies it to the scratch `registry`. Writes
/// nothing when the slot holds it already.
fn set(registry: &mut Registry, ops: &mut Vec<RegistryDelta>, slot: Slot, input: Option<NodeInput>) {
	if slot_value(registry, slot) == input {
		return;
	}
	match slot {
		Slot::Input(id, index) => {
			let Some(input) = input else { return };
			if let Some(slot) = registry.node_instances.get_mut(&id).and_then(|node| node.inputs.get_mut(index as usize)) {
				slot.input = input.clone();
			}
			ops.push(RegistryDelta::ChangeNodeInput { id, index, new_input: input });
		}
		Slot::Export(id, index) => {
			if let Some(slot) = registry.networks.get_mut(&id).and_then(|network| network.exports.get_mut(index as usize)) {
				slot.target = input.clone();
			}
			ops.push(RegistryDelta::SetNetworkExport { id, index, export: input });
		}
	}
}
