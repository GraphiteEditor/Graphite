//! What people say about a document's history, kept beside it rather than in it: user names, step labels
//! and tags. None of it is folded into the document, so moving the head never changes it and stating it
//! never rewrites a delta.

use crate::{PeerId, Rev, UserId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// When a fact was stated and by whom: milliseconds since the Unix epoch, the peer breaking ties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WallStamp {
	pub ms: u64,
	pub peer: PeerId,
}

/// What a fact is about. A rev stands for the interaction it closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Subject {
	User(UserId),
	Rev(Rev),
}

/// One statement about a subject, and one line of the metadata file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetadataFact {
	pub subject: Subject,
	pub key: String,
	pub value: serde_json::Value,
	pub stamp: WallStamp,
}

/// The attribute keys the editor writes for a user.
pub mod user_attr {
	/// The display name the user chose. Absent or empty means unnamed.
	pub const NAME: &str = "name";
}

/// The attribute keys the editor writes for a rev.
pub mod rev_attr {
	/// The name a person gave the interaction. Absent or empty means unnamed.
	pub const LABEL: &str = "label";
	/// Prefixes each tag's name to form its key. The value is `true` while the tag stands and `false` once removed.
	pub const TAG_PREFIX: &str = "tag:";
}

/// The standing fact for each subject and key. Folding the same facts in any order gives the same record, so it travels as its facts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<MetadataFact>", into = "Vec<MetadataFact>")]
pub struct HistoryMetadata {
	facts: BTreeMap<Subject, BTreeMap<String, MetadataFact>>,
}

impl From<Vec<MetadataFact>> for HistoryMetadata {
	fn from(facts: Vec<MetadataFact>) -> Self {
		Self::fold(facts)
	}
}

impl From<HistoryMetadata> for Vec<MetadataFact> {
	fn from(record: HistoryMetadata) -> Self {
		record.facts().cloned().collect()
	}
}

impl HistoryMetadata {
	pub fn is_empty(&self) -> bool {
		self.facts.is_empty()
	}

	pub fn fold(facts: impl IntoIterator<Item = MetadataFact>) -> Self {
		let mut record = Self::default();
		for fact in facts {
			record.absorb(fact);
		}
		record
	}

	pub fn facts(&self) -> impl Iterator<Item = &MetadataFact> {
		self.facts.values().flat_map(BTreeMap::values)
	}

	fn standing(&self, subject: Subject, key: &str) -> Option<&MetadataFact> {
		self.facts.get(&subject)?.get(key)
	}

	fn text(&self, subject: Subject, key: &str) -> Option<&str> {
		self.standing(subject, key)?.value.as_str().filter(|text| !text.is_empty())
	}

	pub fn user_name(&self, user: UserId) -> Option<&str> {
		self.text(Subject::User(user), user_attr::NAME)
	}

	pub fn rev_label(&self, rev: Rev) -> Option<&str> {
		self.text(Subject::Rev(rev), rev_attr::LABEL)
	}

	/// The tags standing on `rev`, in name order.
	pub fn rev_tags(&self, rev: Rev) -> Vec<&str> {
		let Some(attributes) = self.facts.get(&Subject::Rev(rev)) else { return Vec::new() };
		attributes
			.values()
			.filter(|fact| fact.value == true)
			.filter_map(|fact| fact.key.strip_prefix(rev_attr::TAG_PREFIX))
			.collect()
	}

	/// The fact that would state `value`, stamped after the one it replaces so a clock behind a peer's still wins.
	/// `None` when the value already stands, or when the standing stamp is at the end of time and can't be outdone.
	pub fn statement(&self, subject: Subject, key: &str, value: serde_json::Value, mut stamp: WallStamp) -> Option<MetadataFact> {
		if let Some(standing) = self.standing(subject, key) {
			if standing.value == value {
				return None;
			}
			if stamp <= standing.stamp {
				stamp.ms = standing.stamp.ms.checked_add(1)?;
			}
		}
		let key = key.to_string();
		Some(MetadataFact { subject, key, value, stamp })
	}

	/// Take on a fact unless the standing one has a later stamp, or the same stamp and a value that encodes no lower. Returns whether it landed.
	pub fn absorb(&mut self, fact: MetadataFact) -> bool {
		let attributes = self.facts.entry(fact.subject).or_default();
		let stands = |mine: &MetadataFact| mine.stamp.cmp(&fact.stamp).then_with(|| mine.value.to_string().cmp(&fact.value.to_string())).is_ge();
		if attributes.get(&fact.key).is_some_and(stands) {
			return false;
		}
		attributes.insert(fact.key.clone(), fact);
		true
	}

	/// Take on another copy's record. Returns the facts that landed.
	pub fn merge(&mut self, other: &Self) -> Vec<MetadataFact> {
		other.facts().filter(|fact| self.absorb((*fact).clone())).cloned().collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const ADA: Subject = Subject::User(UserId(1));

	fn stamp(ms: u64) -> WallStamp {
		WallStamp { ms, peer: PeerId(1) }
	}

	fn fact(subject: Subject, key: &str, value: impl Into<serde_json::Value>, ms: u64) -> MetadataFact {
		MetadataFact {
			subject,
			key: key.into(),
			value: value.into(),
			stamp: stamp(ms),
		}
	}

	#[test]
	fn facts_settle_alike_whatever_order_they_meet_in() {
		let name = |value, ms| fact(ADA, user_attr::NAME, value, ms);
		let later = HistoryMetadata::fold([name("Bea", 20), name("Ada", 10)]);
		assert_eq!(later, HistoryMetadata::fold([name("Ada", 10), name("Bea", 20)]));
		assert_eq!(later.user_name(UserId(1)), Some("Bea"));
		assert_eq!(HistoryMetadata::fold([name("Ada", 10), name("Bea", 10)]), HistoryMetadata::fold([name("Bea", 10), name("Ada", 10)]));
	}

	#[test]
	fn merging_copies_agrees_either_way_and_only_once() {
		let a = HistoryMetadata::fold([fact(ADA, user_attr::NAME, "Ada", 10)]);
		let b = HistoryMetadata::fold([fact(ADA, user_attr::NAME, "Bea", 20)]);
		let (mut ab, mut ba) = (a.clone(), b.clone());
		assert_eq!(ab.merge(&b).len(), 1);
		assert!(ba.merge(&a).is_empty());
		assert_eq!(ab, ba);
		assert!(ab.merge(&b).is_empty(), "merging again changes nothing");
	}

	#[test]
	fn a_clock_behind_a_peers_still_gets_its_say() {
		let record = HistoryMetadata::fold([fact(ADA, user_attr::NAME, "Ada", 100)]);
		let stated = record.statement(ADA, user_attr::NAME, "Countess".into(), stamp(5)).expect("a change");
		assert_eq!(stated.stamp.ms, 101);
		assert!(record.statement(ADA, user_attr::NAME, "Ada".into(), stamp(500)).is_none(), "the same value is no change");
	}

	#[test]
	fn empty_names_and_removed_tags_do_not_read_back() {
		let step = Rev::new(7).expect("non-zero");
		let subject = Subject::Rev(step);
		let record = HistoryMetadata::fold([fact(ADA, user_attr::NAME, "", 1), fact(subject, "tag:v1", true, 1), fact(subject, "tag:draft", false, 1)]);
		assert_eq!(record.user_name(UserId(1)), None);
		assert_eq!(record.rev_tags(step), vec!["v1"]);
	}
}
