//! What people say about a document's history, kept beside it rather than in it: who its users are, and
//! later what its steps are called and which are tagged. None of it is folded, so moving the head never
//! changes it and stating it never rewrites a delta. Every fact carries a wall-clock stamp and the last
//! writer wins, which makes the whole record a state that merges by taking the newer fact per key, in any
//! order and any number of times.

use crate::ids::{PeerId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// When a fact was stated and by whom: milliseconds since the Unix epoch, the peer breaking ties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WallStamp {
	pub ms: u64,
	pub peer: PeerId,
}

/// One stated value with its stamp.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fact {
	pub value: serde_json::Value,
	pub stamp: WallStamp,
}

/// Everything on record about one user.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserRecord {
	pub user: UserId,
	pub attributes: BTreeMap<String, Fact>,
}

/// The attribute keys the editor writes for a user.
pub mod user_attr {
	/// The display name the user chose. Absent or empty means unnamed.
	pub const NAME: &str = "name";
}

/// The record kept beside a document's history. Users are kept sorted by id so two copies holding the
/// same facts serialize alike.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryMetadata {
	#[serde(default)]
	pub users: Vec<UserRecord>,
}

impl HistoryMetadata {
	pub fn is_empty(&self) -> bool {
		self.users.is_empty()
	}

	pub fn user(&self, user: UserId) -> Option<&UserRecord> {
		self.users.binary_search_by_key(&user, |record| record.user).ok().map(|index| &self.users[index])
	}

	pub fn user_attribute(&self, user: UserId, key: &str) -> Option<&Fact> {
		self.user(user)?.attributes.get(key)
	}

	/// The user's display name, if one is on record and not empty.
	pub fn user_name(&self, user: UserId) -> Option<&str> {
		self.user_attribute(user, user_attr::NAME)?.value.as_str().filter(|name| !name.is_empty())
	}

	/// State a fact about a user. Nothing changes when the same value already stands, whatever its stamp;
	/// otherwise the fact is stamped no earlier than the one it replaces, so a clock behind a peer's still
	/// gets its say. Returns whether anything changed.
	pub fn set_user_attribute(&mut self, user: UserId, key: &str, value: serde_json::Value, mut stamp: WallStamp) -> bool {
		let record = self.user_record_mut(user);
		if let Some(existing) = record.attributes.get(key) {
			if existing.value == value {
				return false;
			}
			if stamp <= existing.stamp {
				stamp.ms = existing.stamp.ms + 1;
			}
		}
		record.attributes.insert(key.to_string(), Fact { value, stamp });
		true
	}

	/// Take on another copy's facts: for every key, the newer stamp wins. Returns whether anything changed.
	pub fn merge(&mut self, other: &Self) -> bool {
		let mut changed = false;
		for theirs in &other.users {
			let record = self.user_record_mut(theirs.user);
			for (key, fact) in &theirs.attributes {
				if record.attributes.get(key).is_none_or(|mine| fact.stamp > mine.stamp) {
					record.attributes.insert(key.clone(), fact.clone());
					changed = true;
				}
			}
		}
		changed
	}

	fn user_record_mut(&mut self, user: UserId) -> &mut UserRecord {
		let index = match self.users.binary_search_by_key(&user, |record| record.user) {
			Ok(index) => index,
			Err(index) => {
				self.users.insert(index, UserRecord { user, attributes: BTreeMap::new() });
				index
			}
		};
		&mut self.users[index]
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn stamp(ms: u64, peer: u64) -> WallStamp {
		WallStamp { ms, peer: PeerId(peer) }
	}

	#[test]
	fn the_newer_fact_wins_whatever_order_the_copies_meet_in() {
		let mut a = HistoryMetadata::default();
		let mut b = HistoryMetadata::default();
		assert!(a.set_user_attribute(UserId(1), user_attr::NAME, "Ada".into(), stamp(10, 1)));
		assert!(b.set_user_attribute(UserId(1), user_attr::NAME, "Ada L.".into(), stamp(20, 2)));

		let mut ab = a.clone();
		assert!(ab.merge(&b));
		let mut ba = b.clone();
		assert!(!ba.merge(&a));
		assert_eq!(ab, ba);
		assert_eq!(ab.user_name(UserId(1)), Some("Ada L."));
		assert!(!ab.merge(&b), "merging again changes nothing");
	}

	#[test]
	fn a_clock_behind_a_peers_still_gets_its_say() {
		let mut record = HistoryMetadata::default();
		record.set_user_attribute(UserId(1), user_attr::NAME, "Ada".into(), stamp(100, 2));
		assert!(record.set_user_attribute(UserId(1), user_attr::NAME, "Countess".into(), stamp(5, 1)));
		assert_eq!(record.user_name(UserId(1)), Some("Countess"));
		assert_eq!(record.user_attribute(UserId(1), user_attr::NAME).unwrap().stamp.ms, 101);
		assert!(!record.set_user_attribute(UserId(1), user_attr::NAME, "Countess".into(), stamp(500, 1)), "the same value is no change");
	}

	#[test]
	fn an_empty_name_counts_as_unnamed_and_users_stay_sorted() {
		let mut record = HistoryMetadata::default();
		record.set_user_attribute(UserId(9), user_attr::NAME, "".into(), stamp(1, 1));
		record.set_user_attribute(UserId(3), user_attr::NAME, "Bea".into(), stamp(1, 1));
		assert_eq!(record.user_name(UserId(9)), None);
		assert_eq!(record.users.iter().map(|record| record.user).collect::<Vec<_>>(), vec![UserId(3), UserId(9)]);
	}
}
