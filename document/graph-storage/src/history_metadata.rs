//! What people say about a document's history, kept beside it rather than in it: user names, step labels
//! and tags. None of it is folded into the document, so moving the head never changes it and stating it
//! never rewrites a delta. Each [`MetadataFact`] carries a wall-clock stamp and the newer of two about the
//! same thing wins, so the record merges the same in any order and any number of times.

use crate::ids::{PeerId, Rev, UserId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// When a fact was stated and by whom: milliseconds since the Unix epoch, the peer breaking ties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WallStamp {
	pub ms: u64,
	pub peer: PeerId,
}

/// What a fact is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Subject {
	User(UserId),
	Rev(Rev),
}

/// One statement: an attribute of a user or of a rev, with its stamp. The unit of the file and of a merge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadataFact {
	pub subject: Subject,
	pub key: String,
	pub value: serde_json::Value,
	pub stamp: WallStamp,
}

/// One stated value with its stamp, as the folded record holds it.
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

/// Everything on record about one rev, which stands for the interaction it closes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RevRecord {
	pub rev: Rev,
	pub attributes: BTreeMap<String, Fact>,
}

/// The attribute keys the editor writes for a user.
pub mod user_attr {
	/// The display name the user chose. Absent or empty means unnamed.
	pub const NAME: &str = "name";
}

/// The attribute keys the editor writes for a rev.
pub mod rev_attr {
	/// The name a person gave the interaction the rev closes. Absent or empty means unnamed.
	pub const LABEL: &str = "label";
	/// A tag on the rev, one key per tag name after this prefix; `true` while it stands, `false` once removed.
	pub const TAG_PREFIX: &str = "tag:";
}

/// The folded record. Users and revs are kept sorted so two copies holding the same facts serialize alike.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryMetadata {
	#[serde(default)]
	pub users: Vec<UserRecord>,
	#[serde(default)]
	pub revs: Vec<RevRecord>,
}

impl HistoryMetadata {
	pub fn is_empty(&self) -> bool {
		self.users.is_empty() && self.revs.is_empty()
	}

	/// The record folded from facts in file order: each fact lands unless a newer one about the same thing stands.
	pub fn fold(facts: impl IntoIterator<Item = MetadataFact>) -> Self {
		let mut record = Self::default();
		for fact in facts {
			record.absorb(fact);
		}
		record
	}

	/// Every fact the record holds, users first then revs, for writing it out whole.
	pub fn facts(&self) -> Vec<MetadataFact> {
		let users = self.users.iter().flat_map(|record| facts_about(Subject::User(record.user), &record.attributes));
		let revs = self.revs.iter().flat_map(|record| facts_about(Subject::Rev(record.rev), &record.attributes));
		users.chain(revs).collect()
	}

	pub fn user(&self, user: UserId) -> Option<&UserRecord> {
		self.users.binary_search_by_key(&user, |record| record.user).ok().map(|index| &self.users[index])
	}

	fn user_attribute(&self, user: UserId, key: &str) -> Option<&Fact> {
		self.user(user)?.attributes.get(key)
	}

	/// The user's display name, if one is on record and not empty.
	pub fn user_name(&self, user: UserId) -> Option<&str> {
		self.user_attribute(user, user_attr::NAME)?.value.as_str().filter(|name| !name.is_empty())
	}

	pub fn rev(&self, rev: Rev) -> Option<&RevRecord> {
		self.revs.binary_search_by_key(&rev, |record| record.rev).ok().map(|index| &self.revs[index])
	}

	fn rev_attribute(&self, rev: Rev, key: &str) -> Option<&Fact> {
		self.rev(rev)?.attributes.get(key)
	}

	/// The name a person gave the interaction `rev` closes, if any and not empty.
	pub fn rev_label(&self, rev: Rev) -> Option<&str> {
		self.rev_attribute(rev, rev_attr::LABEL)?.value.as_str().filter(|label| !label.is_empty())
	}

	/// The tags standing on `rev`, in name order.
	pub fn rev_tags(&self, rev: Rev) -> Vec<&str> {
		let Some(record) = self.rev(rev) else { return Vec::new() };
		record
			.attributes
			.iter()
			.filter(|(_, fact)| fact.value.as_bool() == Some(true))
			.filter_map(|(key, _)| key.strip_prefix(rev_attr::TAG_PREFIX))
			.collect()
	}

	/// State a fact about a user. A no-op when the same value already stands; otherwise stamped after the fact it
	/// replaces, so a clock behind a peer's still wins. Returns the fact as stated, for appending and broadcasting.
	pub fn set_user_attribute(&mut self, user: UserId, key: &str, value: serde_json::Value, stamp: WallStamp) -> Option<MetadataFact> {
		self.state(Subject::User(user), key, value, stamp)
	}

	/// State a fact about a rev; see [`set_user_attribute`](Self::set_user_attribute).
	pub fn set_rev_attribute(&mut self, rev: Rev, key: &str, value: serde_json::Value, stamp: WallStamp) -> Option<MetadataFact> {
		self.state(Subject::Rev(rev), key, value, stamp)
	}

	fn state(&mut self, subject: Subject, key: &str, value: serde_json::Value, mut stamp: WallStamp) -> Option<MetadataFact> {
		let attributes = self.attributes_mut(subject);
		if let Some(existing) = attributes.get(key) {
			if existing.value == value {
				return None;
			}
			if stamp <= existing.stamp {
				// A stamp at the end of time can't be outdone, so the change could never land elsewhere.
				stamp.ms = existing.stamp.ms.checked_add(1)?;
			}
		}
		attributes.insert(key.to_string(), Fact { value: value.clone(), stamp });
		Some(MetadataFact {
			subject,
			key: key.to_string(),
			value,
			stamp,
		})
	}

	/// Take on a fact stated elsewhere unless the one standing for the same thing is newer, or as new with the greater
	/// value, so even facts sharing a stamp settle alike everywhere. Returns whether it landed.
	fn absorb(&mut self, fact: MetadataFact) -> bool {
		let attributes = self.attributes_mut(fact.subject);
		let stands = |mine: &Fact| match fact.stamp.cmp(&mine.stamp) {
			std::cmp::Ordering::Less => true,
			std::cmp::Ordering::Greater => false,
			std::cmp::Ordering::Equal => fact.value.to_string().cmp(&mine.value.to_string()).is_le(),
		};
		if attributes.get(&fact.key).is_some_and(stands) {
			return false;
		}
		attributes.insert(fact.key, Fact { value: fact.value, stamp: fact.stamp });
		true
	}

	/// Take on another copy's record: for every key, the newer stamp wins. Returns the facts that landed, for
	/// appending to the file.
	pub fn merge(&mut self, other: &Self) -> Vec<MetadataFact> {
		other.facts().into_iter().filter(|fact| self.absorb(fact.clone())).collect()
	}

	fn attributes_mut(&mut self, subject: Subject) -> &mut BTreeMap<String, Fact> {
		match subject {
			Subject::User(user) => {
				let index = match self.users.binary_search_by_key(&user, |record| record.user) {
					Ok(index) => index,
					Err(index) => {
						self.users.insert(index, UserRecord { user, attributes: BTreeMap::new() });
						index
					}
				};
				&mut self.users[index].attributes
			}
			Subject::Rev(rev) => {
				let index = match self.revs.binary_search_by_key(&rev, |record| record.rev) {
					Ok(index) => index,
					Err(index) => {
						self.revs.insert(index, RevRecord { rev, attributes: BTreeMap::new() });
						index
					}
				};
				&mut self.revs[index].attributes
			}
		}
	}
}

fn facts_about(subject: Subject, attributes: &BTreeMap<String, Fact>) -> impl Iterator<Item = MetadataFact> + '_ {
	attributes.iter().map(move |(key, fact)| MetadataFact {
		subject,
		key: key.clone(),
		value: fact.value.clone(),
		stamp: fact.stamp,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn stamp(ms: u64, peer: u64) -> WallStamp {
		WallStamp { ms, peer: PeerId(peer) }
	}

	fn rev(n: u128) -> Rev {
		Rev::new(n).expect("non-zero")
	}

	#[test]
	fn the_newer_fact_wins_whatever_order_the_copies_meet_in() {
		let mut a = HistoryMetadata::default();
		let mut b = HistoryMetadata::default();
		assert!(a.set_user_attribute(UserId(1), user_attr::NAME, "Ada".into(), stamp(10, 1)).is_some());
		assert!(b.set_user_attribute(UserId(1), user_attr::NAME, "Ada L.".into(), stamp(20, 2)).is_some());

		let mut ab = a.clone();
		assert_eq!(ab.merge(&b).len(), 1);
		let mut ba = b.clone();
		assert!(ba.merge(&a).is_empty());
		assert_eq!(ab, ba);
		assert_eq!(ab.user_name(UserId(1)), Some("Ada L."));
		assert!(ab.merge(&b).is_empty(), "merging again changes nothing");
	}

	#[test]
	fn a_clock_behind_a_peers_still_gets_its_say() {
		let mut record = HistoryMetadata::default();
		record.set_user_attribute(UserId(1), user_attr::NAME, "Ada".into(), stamp(100, 2));
		let stated = record.set_user_attribute(UserId(1), user_attr::NAME, "Countess".into(), stamp(5, 1)).expect("a change");
		assert_eq!(stated.stamp.ms, 101);
		assert_eq!(record.user_name(UserId(1)), Some("Countess"));
		assert!(
			record.set_user_attribute(UserId(1), user_attr::NAME, "Countess".into(), stamp(500, 1)).is_none(),
			"the same value is no change"
		);
	}

	#[test]
	fn the_fold_of_the_facts_is_the_record_and_back() {
		let mut record = HistoryMetadata::default();
		record.set_user_attribute(UserId(9), user_attr::NAME, "".into(), stamp(1, 1));
		record.set_user_attribute(UserId(3), user_attr::NAME, "Bea".into(), stamp(1, 1));
		record.set_rev_attribute(rev(7), rev_attr::LABEL, "Fixed the eye".into(), stamp(2, 1));
		record.set_rev_attribute(rev(7), "tag:v1", true.into(), stamp(3, 1));
		record.set_rev_attribute(rev(7), "tag:draft", false.into(), stamp(4, 1));
		assert_eq!(record.user_name(UserId(9)), None);
		assert_eq!(record.users.iter().map(|record| record.user).collect::<Vec<_>>(), vec![UserId(3), UserId(9)]);
		assert_eq!(record.rev_label(rev(7)), Some("Fixed the eye"));
		assert_eq!(record.rev_tags(rev(7)), vec!["v1"]);

		let facts = record.facts();
		assert_eq!(facts.len(), 5);
		assert_eq!(HistoryMetadata::fold(facts), record);
	}

	#[test]
	fn facts_sharing_a_stamp_settle_alike_whatever_order_they_meet_in() {
		let fact = |value: &str| MetadataFact {
			subject: Subject::User(UserId(1)),
			key: user_attr::NAME.into(),
			value: value.into(),
			stamp: stamp(10, 1),
		};
		let ab = HistoryMetadata::fold([fact("Ada"), fact("Bea")]);
		let ba = HistoryMetadata::fold([fact("Bea"), fact("Ada")]);
		assert_eq!(ab, ba);
	}

	#[test]
	fn a_fact_stamped_at_the_end_of_time_cannot_be_outdone() {
		let mut record = HistoryMetadata::fold([MetadataFact {
			subject: Subject::User(UserId(1)),
			key: user_attr::NAME.into(),
			value: "Ada".into(),
			stamp: stamp(u64::MAX, 2),
		}]);
		assert!(record.set_user_attribute(UserId(1), user_attr::NAME, "Bea".into(), stamp(5, 1)).is_none());
		assert_eq!(record.user_name(UserId(1)), Some("Ada"));
	}
}
