use crate::{Registry, RetiredMarks, SessionState};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use std::collections::HashMap;

/// Serializes a stored value with its hash maps in key order, so equal values write equal bytes. It decodes as the value itself.
pub struct Sorted<'a, T>(pub &'a T);

struct ByKey<'a, K, V>(&'a HashMap<K, V>);

impl<K: Ord + Serialize, V: Serialize> Serialize for ByKey<'_, K, V> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let mut entries: Vec<_> = self.0.iter().collect();
		entries.sort_unstable_by_key(|&(key, _)| key);
		serializer.collect_map(entries)
	}
}

// Each impl destructures the type, so a new stored field does not compile until it is written here too.

impl Serialize for Sorted<'_, Registry> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let Registry {
			node_instances,
			networks,
			resources,
			peer_users,
			attributes,
			removed_nodes,
			removed_networks,
			removed_resources,
		} = self.0;
		let mut out = serializer.serialize_struct("Registry", 8)?;
		out.serialize_field("node_instances", &ByKey(node_instances))?;
		out.serialize_field("networks", &ByKey(networks))?;
		out.serialize_field("resources", &ByKey(resources))?;
		out.serialize_field("peer_users", &ByKey(peer_users))?;
		out.serialize_field("attributes", attributes)?;
		out.serialize_field("removed_nodes", &ByKey(removed_nodes))?;
		out.serialize_field("removed_networks", &ByKey(removed_networks))?;
		out.serialize_field("removed_resources", &ByKey(removed_resources))?;
		out.end()
	}
}

impl Serialize for Sorted<'_, RetiredMarks> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let RetiredMarks {
			retired_up_to,
			retired_runs,
			taken_back,
		} = self.0;
		let mut out = serializer.serialize_struct("RetiredMarks", 3)?;
		out.serialize_field("retired_up_to", &ByKey(retired_up_to))?;
		out.serialize_field("retired_runs", &ByKey(retired_runs))?;
		out.serialize_field("taken_back", &ByKey(taken_back))?;
		out.end()
	}
}

impl Serialize for Sorted<'_, SessionState> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let SessionState {
			peer_id,
			user_id,
			head_rev,
			last_broadcast_rev,
			redo_stack,
			next_node_counter,
			last_hot_sequence,
			view_settings,
			network_view_settings,
			retired_marks,
			clock_counter,
			shared,
		} = self.0;
		let mut out = serializer.serialize_struct("SessionState", 12)?;
		out.serialize_field("peer_id", peer_id)?;
		out.serialize_field("user_id", user_id)?;
		out.serialize_field("head_rev", head_rev)?;
		out.serialize_field("last_broadcast_rev", last_broadcast_rev)?;
		out.serialize_field("redo_stack", redo_stack)?;
		out.serialize_field("next_node_counter", next_node_counter)?;
		out.serialize_field("last_hot_sequence", last_hot_sequence)?;
		out.serialize_field("view_settings", view_settings)?;
		out.serialize_field("network_view_settings", network_view_settings)?;
		out.serialize_field("retired_marks", &Sorted(retired_marks))?;
		out.serialize_field("clock_counter", clock_counter)?;
		out.serialize_field("shared", shared)?;
		out.end()
	}
}

#[cfg(test)]
mod tests {
	use super::Sorted;
	use crate::{
		HotSequence, Implementation, Network, NetworkId, Node, NodeId, PeerId, PeerRegistration, Registry, ResourceEntry, ResourceId, RetiredMarks, SessionState, TimeStamp, Tombstone, UserId,
	};
	use std::collections::HashMap;
	use std::hash::Hash;

	/// The same entries, inserted forwards into a small table or backwards into a large one, so they iterate differently.
	fn map<K: Eq + Hash, V>(reversed: bool, entry: impl Fn(u64) -> (K, V)) -> HashMap<K, V> {
		let keys: Vec<u64> = (0..200).map(|key| key * 7919 % 1000).collect();
		let mut map = HashMap::with_capacity(if reversed { 1024 } else { 0 });
		match reversed {
			true => map.extend(keys.into_iter().rev().map(entry)),
			false => map.extend(keys.into_iter().map(entry)),
		}
		map
	}

	fn tombstone<T>(content: T) -> Tombstone<T> {
		Tombstone {
			content,
			timestamp: TimeStamp::ORIGIN,
			placeholder: false,
		}
	}

	fn registry(reversed: bool) -> Registry {
		let node = || Node::new(NetworkId(1), Implementation::ProtoNode(ResourceId::from(1)), 0);
		Registry {
			node_instances: map(reversed, |id| (NodeId(id), node())),
			networks: map(reversed, |id| (NetworkId(id), Network::default())),
			resources: map(reversed, |id| (ResourceId::from(id), ResourceEntry::default())),
			peer_users: map(reversed, |id| {
				let registration = PeerRegistration {
					user: UserId(id),
					timestamp: TimeStamp::ORIGIN,
				};
				(PeerId(id), registration)
			}),
			removed_nodes: map(reversed, |id| (NodeId(id), tombstone(node()))),
			removed_networks: map(reversed, |id| (NetworkId(id), tombstone(Network::default()))),
			removed_resources: map(reversed, |id| (ResourceId::from(id), tombstone(ResourceEntry::default()))),
			..Default::default()
		}
	}

	fn session_state(reversed: bool) -> SessionState {
		let runs = |peer| (PeerId(peer), vec![(HotSequence(peer), HotSequence(peer))]);
		SessionState {
			retired_marks: RetiredMarks {
				retired_up_to: map(reversed, |peer| (PeerId(peer), HotSequence(peer))),
				retired_runs: map(reversed, runs),
				taken_back: map(reversed, runs),
			},
			..Default::default()
		}
	}

	#[test]
	fn equal_values_serialize_alike_whatever_their_insertion_order() {
		let (forward, backward) = (registry(false), registry(true));
		assert_eq!(forward, backward);
		assert_eq!(postcard::to_stdvec(&Sorted(&forward)).unwrap(), postcard::to_stdvec(&Sorted(&backward)).unwrap());

		let (forward, backward) = (session_state(false), session_state(true));
		assert_eq!(serde_json::to_string(&Sorted(&forward)).unwrap(), serde_json::to_string(&Sorted(&backward)).unwrap());
	}

	#[test]
	fn a_sorted_value_decodes_as_the_value() {
		let registry = registry(false);
		assert_eq!(postcard::from_bytes::<Registry>(&postcard::to_stdvec(&Sorted(&registry)).unwrap()).unwrap(), registry);

		let state = session_state(false);
		let decoded: SessionState = serde_json::from_str(&serde_json::to_string(&Sorted(&state)).unwrap()).unwrap();
		assert_eq!(decoded.retired_marks, state.retired_marks);
	}
}
