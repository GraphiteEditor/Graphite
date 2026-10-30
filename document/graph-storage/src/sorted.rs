use serde::{Serialize, Serializer};
use std::collections::HashMap;

/// Serializes a hash map as a map in ascending key order, so equal maps write equal bytes whatever their iteration order.
pub(crate) fn serialize_sorted<K: Ord + Serialize, V: Serialize, H, S: Serializer>(map: &HashMap<K, V, H>, serializer: S) -> Result<S::Ok, S::Error> {
	let mut entries: Vec<_> = map.iter().collect();
	entries.sort_unstable_by_key(|&(key, _)| key);
	serializer.collect_map(entries)
}

#[cfg(test)]
mod tests {
	use super::serialize_sorted;
	use serde::{Deserialize, Serialize};
	use std::collections::{BTreeMap, HashMap};

	#[derive(Serialize, Deserialize, PartialEq, Debug)]
	struct Sorted(#[serde(serialize_with = "serialize_sorted")] HashMap<u64, String>);

	fn entries() -> impl Iterator<Item = (u64, String)> {
		(0..200).map(|key| (key * 7919 % 1000, format!("value {key}")))
	}

	#[test]
	fn maps_built_in_any_order_serialize_alike() {
		let forward = Sorted(entries().collect());
		let mut backward = Sorted(HashMap::with_capacity(1024));
		backward.0.extend(entries().collect::<Vec<_>>().into_iter().rev());

		assert_eq!(postcard::to_stdvec(&forward).unwrap(), postcard::to_stdvec(&backward).unwrap());
		assert_eq!(serde_json::to_string(&forward).unwrap(), serde_json::to_string(&backward).unwrap());
	}

	#[test]
	fn a_sorted_map_writes_what_a_btree_map_writes() {
		let sorted = Sorted(entries().collect());
		let btree: BTreeMap<u64, String> = entries().collect();

		assert_eq!(postcard::to_stdvec(&sorted).unwrap(), postcard::to_stdvec(&btree).unwrap());
		assert_eq!(serde_json::to_string(&sorted).unwrap(), serde_json::to_string(&btree).unwrap());
	}

	#[test]
	fn a_sorted_map_reads_back_as_the_same_map() {
		let sorted = Sorted(entries().collect());
		let bytes = postcard::to_stdvec(&sorted).unwrap();

		assert_eq!(postcard::from_bytes::<Sorted>(&bytes).unwrap(), sorted);
	}
}
