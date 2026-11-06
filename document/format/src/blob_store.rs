//! The working copy's blob records, one `blobs/<hash>` file each. A record holds the blob's bytes in full, or a copy
//! delta against the blob its node input held before. Values change in a few places per edit, so consecutive versions
//! of one input share nearly all of their bytes, which a delta stores once. Chains are unbounded, which costs nothing
//! while loading decodes every blob anyway. Records stay uncompressed, so an archive codec can compress across them.
//!
//! A record starts with its kind byte. A delta record follows it with its base's 32 hash bytes. The payload makes up the rest.

use std::collections::{HashMap, HashSet};

use document_container::{AnyContainer, AsyncContainer};
use document_graph_storage::blob::{BlobBytes, NewBlobs};
use document_graph_storage::{BlobHash, ResourceHash};

use crate::layout::Layout;

const FULL: u8 = 0;
const DELTA: u8 = 1;

/// The decoded bytes of every blob the document names, with the record each is stored as.
#[derive(Clone, Debug, Default)]
pub(crate) struct BlobStore {
	pub(crate) bytes: BlobBytes,
	records: HashMap<BlobHash, Vec<u8>>,
}

impl BlobStore {
	/// Hold a new blob, returning the record to write for it, which is a delta against `base` when that is held.
	/// A blob already held returns `None`.
	pub(crate) fn insert(&mut self, hash: BlobHash, bytes: Vec<u8>, base: Option<BlobHash>) -> Option<Vec<u8>> {
		if self.bytes.contains_key(&hash) {
			return None;
		}

		let record = match base.and_then(|base| Some((base, self.bytes.get(&base)?))) {
			Some((base, base_bytes)) => delta_record(&base, base_bytes, &bytes),
			None => full_record(&bytes),
		};

		self.bytes.insert(hash, bytes);
		self.records.insert(hash, record.clone());
		Some(record)
	}

	/// [`insert`](Self::insert) every blob in hash order, each base ahead of the blobs encoded against it.
	pub(crate) fn insert_all(&mut self, blobs: NewBlobs) -> Vec<(BlobHash, Vec<u8>)> {
		let NewBlobs { bytes: mut blobs, bases } = blobs;
		let mut hashes: Vec<_> = blobs.keys().copied().collect();
		hashes.sort_unstable();

		let mut records = Vec::new();
		for hash in hashes {
			for hash in chain_to(hash, |hash| bases.get(hash).filter(|base| blobs.contains_key(base)).copied()) {
				let Some(bytes) = blobs.remove(&hash) else { continue };
				if let Some(record) = self.insert(hash, bytes, bases.get(&hash).copied()) {
					records.push((hash, record));
				}
			}
		}
		records
	}

	/// The record of every held blob, in hash order but each after its base.
	#[cfg(feature = "network")]
	pub(crate) fn records(&self) -> Vec<(BlobHash, Vec<u8>)> {
		let mut hashes: Vec<_> = self.records.keys().copied().collect();
		hashes.sort_unstable();

		let mut listed = HashSet::new();
		let mut records = Vec::new();
		for hash in hashes {
			let unlisted_base = |hash: &BlobHash| record_base(self.records.get(hash)?).filter(|base| !listed.contains(base) && self.records.contains_key(base));
			for hash in chain_to(hash, unlisted_base) {
				if listed.insert(hash) {
					records.push((hash, self.records[&hash].clone()));
				}
			}
		}
		records
	}

	/// Hold the blobs of records from elsewhere, checking each against its hash, and return those held for writing.
	/// A record whose base isn't held here, or that is malformed, is skipped.
	#[cfg(feature = "network")]
	pub(crate) fn absorb(&mut self, records: Vec<(BlobHash, Vec<u8>)>) -> Vec<(BlobHash, Vec<u8>)> {
		let mut pending = records;
		let mut held = Vec::new();

		// Repeat while a pass holds more, since a record can arrive ahead of its base
		loop {
			let waiting = pending.len();
			pending.retain(|(hash, record)| {
				if self.bytes.contains_key(hash) {
					return false;
				}
				let bytes = match decode_record(record) {
					Some(Record::Full(bytes)) => bytes,
					Some(Record::Delta { base, delta }) => {
						let Some(base_bytes) = self.bytes.get(&base) else { return true };
						let Some(bytes) = apply_copy_delta(base_bytes, &delta) else { return false };
						bytes
					}
					None => return false,
				};

				match self.hold_checked(*hash, bytes, record.clone()) {
					Ok(()) => held.push((*hash, record.clone())),
					Err(error) => log::warn!("{error}"),
				}
				false
			});
			if pending.is_empty() || pending.len() == waiting {
				break;
			}
		}

		held
	}

	/// A record holding the held blob `hash` in full, standing on no base.
	pub(crate) fn full_record(&self, hash: &BlobHash) -> Option<Vec<u8>> {
		Some(full_record(self.bytes.get(hash)?))
	}

	/// Drop a blob whose record never got written.
	pub(crate) fn forget(&mut self, hash: &BlobHash) {
		self.bytes.remove(hash);
		self.records.remove(hash);
	}

	/// Read and decode the record of `hash` and those of the bases it stands on, checking each blob against its hash.
	pub(crate) async fn load(&mut self, hash: BlobHash, working: &AnyContainer, layout: &impl Layout) -> Result<(), String> {
		// Walk down the chain to a blob already held or a full record, then apply the deltas on the way back up
		let mut deltas = Vec::new();
		let mut visited = HashSet::new();
		let mut current = hash;
		let mut bytes = loop {
			if let Some(bytes) = self.bytes.get(&current) {
				break bytes.clone();
			}
			if !visited.insert(current) {
				return Err(format!("Blob {hash} sits on a chain that loops back to {current}"));
			}

			let record = working.read(&layout.blob_path(&current)).await.map_err(|error| format!("Blob {current} could not be read: {error}"))?;
			let record = record.as_slice().to_vec();
			match decode_record(&record).ok_or_else(|| format!("Blob {current} has a malformed record"))? {
				Record::Full(bytes) => {
					self.hold_checked(current, bytes.clone(), record)?;
					break bytes;
				}
				Record::Delta { base, delta } => {
					deltas.push((current, delta, record));
					current = base;
				}
			}
		};

		for (hash, delta, record) in deltas.into_iter().rev() {
			bytes = apply_copy_delta(&bytes, &delta).ok_or_else(|| format!("Blob {hash} has a malformed delta"))?;
			self.hold_checked(hash, bytes.clone(), record)?;
		}
		Ok(())
	}

	fn hold_checked(&mut self, hash: BlobHash, bytes: Vec<u8>, record: Vec<u8>) -> Result<(), String> {
		if BlobHash::of(&bytes) != hash {
			return Err(format!("Blob {hash} decodes to bytes of a different hash"));
		}

		self.bytes.insert(hash, bytes);
		self.records.insert(hash, record);
		Ok(())
	}
}

/// `hash` and the bases below it that `base_of` names, deepest first. Bounded in case bases ever form a cycle.
fn chain_to(hash: BlobHash, base_of: impl Fn(&BlobHash) -> Option<BlobHash>) -> Vec<BlobHash> {
	let mut chain = vec![hash];
	let mut visited = HashSet::from([hash]);
	while let Some(base) = chain.last().and_then(&base_of)
		&& visited.insert(base)
	{
		chain.push(base);
	}
	chain.reverse();
	chain
}

/// The base a delta record names.
fn record_base(record: &[u8]) -> Option<BlobHash> {
	let (&DELTA, rest) = record.split_first()? else { return None };
	Some(BlobHash(ResourceHash::from(<[u8; 32]>::try_from(rest.get(..32)?).ok()?)))
}

enum Record {
	Full(Vec<u8>),
	Delta { base: BlobHash, delta: Vec<u8> },
}

fn full_record(bytes: &[u8]) -> Vec<u8> {
	let mut record = vec![FULL];
	record.extend_from_slice(bytes);
	record
}

fn delta_record(base: &BlobHash, base_bytes: &[u8], bytes: &[u8]) -> Vec<u8> {
	let mut record = vec![DELTA];
	record.extend_from_slice(&<[u8; 32]>::from(&base.0));
	record.extend(copy_delta(base_bytes, bytes));
	record
}

fn decode_record(record: &[u8]) -> Option<Record> {
	let (&kind, rest) = record.split_first()?;
	match kind {
		FULL => Some(Record::Full(rest.to_vec())),
		DELTA => Some(Record::Delta {
			base: record_base(record)?,
			delta: rest.get(32..)?.to_vec(),
		}),
		_ => None,
	}
}

// A copy delta rebuilds the target from runs copied out of the base and literal runs it carries itself.
// It indexes the base's aligned blocks and matches them anywhere in the target, like git's delta encoding.
const COPY_BLOCK: usize = 16;
const COPY: u8 = 0;
const INSERT: u8 = 1;

/// O(base + target) expected: one index entry per base block, one lookup per unmatched target byte.
fn copy_delta(base: &[u8], target: &[u8]) -> Vec<u8> {
	let block_key = |bytes: &[u8]| -> u128 {
		let mut key = [0; COPY_BLOCK];
		key.copy_from_slice(&bytes[..COPY_BLOCK]);
		u128::from_le_bytes(key)
	};

	let mut index: HashMap<u128, usize> = HashMap::new();
	for offset in (0..base.len().saturating_sub(COPY_BLOCK - 1)).step_by(COPY_BLOCK) {
		index.entry(block_key(&base[offset..])).or_insert(offset);
	}

	let mut output = Vec::new();
	let mut literal_start = 0;
	let mut position = 0;
	while position + COPY_BLOCK <= target.len() {
		let Some(&base_offset) = index.get(&block_key(&target[position..])) else {
			position += 1;
			continue;
		};

		// Base blocks are aligned but target matches are not, so extend backward into the pending literal too
		let backward = base[..base_offset].iter().rev().zip(target[literal_start..position].iter().rev()).take_while(|(a, b)| a == b).count();
		let forward = base[base_offset..].iter().zip(&target[position..]).take_while(|(a, b)| a == b).count();

		push_insert(&mut output, &target[literal_start..position - backward]);
		output.push(COPY);
		push_varint(&mut output, base_offset - backward);
		push_varint(&mut output, backward + forward);

		position += forward;
		literal_start = position;
	}
	push_insert(&mut output, &target[literal_start..]);

	output
}

fn apply_copy_delta(base: &[u8], mut delta: &[u8]) -> Option<Vec<u8>> {
	let mut output = Vec::new();
	while let Some((&operation, rest)) = delta.split_first() {
		delta = rest;
		match operation {
			COPY => {
				let offset = read_varint(&mut delta)?;
				let length = read_varint(&mut delta)?;
				output.extend_from_slice(base.get(offset..offset.checked_add(length)?)?);
			}
			INSERT => {
				let length = read_varint(&mut delta)?;
				let (literal, rest) = delta.split_at_checked(length)?;
				output.extend_from_slice(literal);
				delta = rest;
			}
			_ => return None,
		}
	}
	Some(output)
}

fn push_insert(output: &mut Vec<u8>, literal: &[u8]) {
	if literal.is_empty() {
		return;
	}

	output.push(INSERT);
	push_varint(output, literal.len());
	output.extend_from_slice(literal);
}

fn push_varint(output: &mut Vec<u8>, mut value: usize) {
	while value >= 0x80 {
		output.push(value as u8 | 0x80);
		value >>= 7;
	}
	output.push(value as u8);
}

fn read_varint(delta: &mut &[u8]) -> Option<usize> {
	let mut value: usize = 0;
	for shift in (0..usize::BITS).step_by(7) {
		let (&byte, rest) = delta.split_first()?;
		*delta = rest;
		value |= ((byte & 0x7f) as usize).checked_shl(shift)?;
		if byte < 0x80 {
			return Some(value);
		}
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	fn hashed(bytes: &[u8]) -> (BlobHash, Vec<u8>) {
		(BlobHash::of(bytes), bytes.to_vec())
	}

	#[test]
	fn copy_delta_round_trips() {
		let base: Vec<u8> = (0..4096_u32).flat_map(|i| (i * 7).to_le_bytes()).collect();
		let mut target = base.clone();
		target.splice(100..108, [1, 2, 3]);
		target[9000] ^= 0xff;
		target.extend_from_slice(b"tail");

		let delta = copy_delta(&base, &target);
		assert!(delta.len() < 200, "a few edits encode as a small delta, got {} bytes", delta.len());
		assert_eq!(apply_copy_delta(&base, &delta).unwrap(), target);
		assert_eq!(apply_copy_delta(&[], &copy_delta(&[], &target)).unwrap(), target);
	}

	#[test]
	fn records_decode_to_what_was_inserted() {
		let mut store = BlobStore::default();
		let (base_hash, base) = hashed(&[7; 2048]);
		let mut changed = base.clone();
		changed[1000] = 0;
		let (changed_hash, changed) = hashed(&changed);

		let full = store.insert(base_hash, base.clone(), None).unwrap();
		let delta = store.insert(changed_hash, changed.clone(), Some(base_hash)).unwrap();
		assert!(store.insert(changed_hash, changed.clone(), Some(base_hash)).is_none(), "a held blob writes nothing");

		assert!(matches!(decode_record(&full), Some(Record::Full(bytes)) if bytes == base));
		let Some(Record::Delta { base: decoded_base, delta }) = decode_record(&delta) else {
			panic!("a delta record")
		};
		assert_eq!(decoded_base, base_hash);
		assert_eq!(apply_copy_delta(&base, &delta).unwrap(), changed);
	}

	#[test]
	fn malformed_deltas_are_rejected() {
		assert!(apply_copy_delta(b"short", &[COPY, 0, 10]).is_none());
		assert!(apply_copy_delta(b"", &[INSERT, 5, 1]).is_none());
		assert!(apply_copy_delta(b"", &[9]).is_none());
		assert!(decode_record(&[DELTA, 1, 2]).is_none());
	}
}
