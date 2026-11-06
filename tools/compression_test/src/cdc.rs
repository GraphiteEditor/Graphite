//! Content-defined chunking of the resource model's large values, as an alternative to per-input delta chains.
//! A value becomes a list of chunk hashes and every distinct chunk is stored once, deflated on its own.

use std::collections::HashSet;
use std::time::Instant;

use fastcdc::v2020::FastCDC;

use crate::models::{Chains, deflate};

const AVERAGE_SIZES: [u32; 5] = [1024, 2048, 4096, 8192, 16_384];
const HASH_LENGTH: usize = 32;

#[derive(Default)]
struct ChunkStore {
	known: HashSet<[u8; HASH_LENGTH]>,
	data_count: usize,
	data_raw: usize,
	data_deflated: usize,
	/// Hash lists are random bytes, so they are counted raw.
	list_raw: usize,
}

impl ChunkStore {
	fn insert(&mut self, bytes: &[u8], is_list: bool) -> [u8; HASH_LENGTH] {
		let hash = *blake3::hash(bytes).as_bytes();
		if !self.known.insert(hash) {
			return hash;
		}

		if is_list {
			self.list_raw += bytes.len();
		} else {
			self.data_count += 1;
			self.data_raw += bytes.len();
			self.data_deflated += deflate(bytes).len();
		}
		hash
	}

	/// Store `bytes` as chunks and return the hash of the list naming them. With `recursive`, a list longer than one
	/// average chunk is itself chunked, so versions sharing most of their chunks also share most of their list.
	fn store_value(&mut self, bytes: &[u8], average: u32, recursive: bool, is_list: bool) -> [u8; HASH_LENGTH] {
		let mut list = Vec::new();
		for chunk in FastCDC::new(bytes, average / 4, average, average * 4) {
			let chunk_bytes = &bytes[chunk.offset..chunk.offset + chunk.length];
			list.extend(self.insert(chunk_bytes, is_list));
		}

		if recursive && list.len() > average as usize {
			return self.store_value(&list, average, true, true);
		}
		self.insert(&list, true)
	}
}

pub fn compare(chains: &Chains) {
	let versions: usize = chains.values().map(Vec::len).sum();
	let raw: usize = chains.values().flatten().map(Vec::len).sum();

	println!();
	println!("content-defined chunking over the same {versions} versions ({raw} bytes raw), threshold 256:");
	println!(
		"  {:>7} {:>7} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>9}",
		"average", "chunks", "data-raw", "data-defl", "list-flat", "list-tree", "total-flat", "total-tree", "ms"
	);

	for average in AVERAGE_SIZES {
		let start = Instant::now();
		let mut flat = ChunkStore::default();
		for blob in chains.values().flatten() {
			flat.store_value(blob, average, false, false);
		}
		let elapsed = start.elapsed();

		let mut tree = ChunkStore::default();
		for blob in chains.values().flatten() {
			tree.store_value(blob, average, true, false);
		}

		println!(
			"  {average:>7} {:>7} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>9.1}",
			flat.data_count,
			flat.data_raw,
			flat.data_deflated,
			flat.list_raw,
			tree.list_raw,
			flat.data_deflated + flat.list_raw,
			tree.data_deflated + tree.list_raw,
			elapsed.as_secs_f64() * 1000.
		);
	}
	println!("  total = deflated chunk data + hash lists, comparable to the store columns above.");
}
