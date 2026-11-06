//! Compares storing large node input values inline in history against storing them as resources, and xz against zip
//! for the outer archive.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::time::Instant;

use document_container::archive::{Archive, Zip};
use document_container::backends::memory::MemoryBackend;
use document_container::{AnyContainer, AsyncContainer};
use document_format::{ExportFormat, ExportOptions, GddV1};
use document_graph_storage::{Delta, HotOp, NodeField, NodeId, NodeInput, Prior, RegistryDelta, ResourceHash, Value};
use flate2::{Compress, Compression, FlushCompress, Status};

use crate::predecessor_only;

const HISTORY_PATH: &str = "history.frames";
const HOT_LOG_PATH: &str = "hot-log.frames";
const THRESHOLDS: [usize; 4] = [256, 4096, 16_384, 65_536];

pub type InputKey = (NodeId, u32);
pub type Chains = HashMap<InputKey, Vec<Vec<u8>>>;

pub fn compare(gdd: &GddV1, deltas: &[&Delta]) {
	let hot_log = gdd.session().hot_log();

	let zip = compare_archives(gdd);
	let files = futures::executor::block_on(archive_files(&zip));
	let rest_deflated: usize = files
		.iter()
		.filter(|(path, _)| path != HISTORY_PATH && path != HOT_LOG_PATH)
		.map(|(_, bytes)| deflate(bytes).len())
		.sum();
	for (path, bytes) in files.iter().filter(|(_, bytes)| bytes.len() > 10_000) {
		println!("  {path:<20} {:>10} raw {:>10} deflated", bytes.len(), deflate(bytes).len());
	}
	let inline_history = history_frames(deltas.iter().copied());
	let inline_hot_log = hot_log_frames(hot_log.iter());

	println!();
	println!("resource model over history and hot log, every other file deflated as in a zip: {rest_deflated} bytes");
	println!(
		"  {:>9} {:>6} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
		"threshold", "chains", "logs", "logs-defl", "raw-blobs", "loose-defl", "dict-defl", "copy-defl", "pred-zstd"
	);
	println!(
		"  {:>9} {:>6} {:>10} {:>10}",
		"inline",
		"-",
		inline_history.len() + inline_hot_log.len(),
		deflate(&inline_history).len() + deflate(&inline_hot_log).len()
	);

	let mut timings = Vec::new();
	let mut smallest_threshold_chains = None;
	for threshold in THRESHOLDS {
		let (stripped, stripped_hot_log, chains) = split_large_values(deltas, hot_log, threshold);
		let history = history_frames(stripped.iter());
		let hot_log = hot_log_frames(stripped_hot_log.iter());
		let blobs = chains.values().flatten();

		let raw: usize = blobs.clone().map(Vec::len).sum();
		let loose: usize = blobs.map(|blob| deflate(blob).len()).sum();
		let dictionary: usize = chains.values().map(|chain| dictionary_chain(chain)).sum();

		let start = Instant::now();
		let copy: usize = chains.values().map(|chain| copy_chain(chain)).sum();
		let copy_time = start.elapsed();

		let start = Instant::now();
		let zstd: usize = chains.values().map(|chain| predecessor_only(chain, 3).iter().sum::<usize>()).sum();
		let zstd_time = start.elapsed();
		timings.push((threshold, raw, copy_time, zstd_time));
		if threshold == THRESHOLDS[0] {
			smallest_threshold_chains = Some(chains.clone());
		}

		println!(
			"  {threshold:>9} {:>6} {:>10} {:>10} {raw:>10} {loose:>10} {dictionary:>10} {copy:>10} {zstd:>10}",
			chains.len(),
			history.len() + hot_log.len(),
			deflate(&history).len() + deflate(&hot_log).len()
		);
	}
	println!("  logs-defl + any store column + {rest_deflated} estimates the whole zip archive.");

	println!();
	println!("store encode time:");
	for (threshold, raw, copy_time, zstd_time) in timings {
		let megabytes = raw as f64 / 1_000_000.;
		println!(
			"  threshold {threshold:>6}: copy-defl {:>8.1} ms ({:>6.1} MB/s)   pred-zstd {:>8.1} ms ({:>6.1} MB/s)",
			copy_time.as_secs_f64() * 1000.,
			megabytes / copy_time.as_secs_f64(),
			zstd_time.as_secs_f64() * 1000.,
			megabytes / zstd_time.as_secs_f64()
		);
	}

	if let Some(chains) = smallest_threshold_chains {
		change_locality(&chains);
		sample_value_diff(&chains);
		crate::cdc::compare(&chains);
	}
}

/// Export the document as xz and zip, returning the zip so its entries can be inspected.
fn compare_archives(gdd: &GddV1) -> Vec<u8> {
	let legacy = futures::executor::block_on(gdd.read_legacy_document());
	let legacy = legacy.as_ref().map(|bytes| bytes.as_slice());

	println!();
	println!("archive export, whole document:");
	let mut zip = Vec::new();
	for (label, format) in [("xz", ExportFormat::Xz), ("zip (deflate)", ExportFormat::Zip)] {
		let start = Instant::now();
		let bytes = futures::executor::block_on(gdd.export_to_bytes(format, ExportOptions::default(), gdd, legacy)).unwrap();
		println!("  {label:<16} {:>9} bytes {:>9.1} ms", bytes.len(), start.elapsed().as_secs_f64() * 1000.);
		if matches!(format, ExportFormat::Zip) {
			zip = bytes;
		}
	}
	zip
}

/// Every file in a zip archive with its uncompressed bytes.
async fn archive_files(zip: &[u8]) -> Vec<(String, Vec<u8>)> {
	let mut memory = MemoryBackend::new();
	Zip::open(std::io::Cursor::new(zip), &mut memory).unwrap();
	let container = AnyContainer::Memory(memory);

	let mut files = Vec::new();
	for path in container.list("").await.unwrap() {
		let bytes = container.read(&path).await.unwrap().as_slice().to_vec();
		files.push((path, bytes));
	}
	files
}

/// Replace every node input value of at least `threshold` encoded bytes with a hash-sized stand-in, collecting each
/// distinct value into the chain of the input it belongs to in history order, then hot log order.
fn split_large_values(deltas: &[&Delta], hot_log: &[HotOp], threshold: usize) -> (Vec<Delta>, Vec<HotOp>, Chains) {
	let mut chains: Chains = HashMap::new();
	let mut seen = HashSet::new();

	let mut strip = |key: InputKey, input: &mut NodeInput| {
		let NodeInput::Value { value, .. } = input else { return };
		let bytes = postcard::to_allocvec(value).unwrap();
		if bytes.len() < threshold {
			return;
		}

		*value = Value::Str("0".repeat(32));
		if seen.insert(ResourceHash::from(bytes.as_slice())) {
			chains.entry(key).or_default().push(bytes);
		}
	};

	let stripped = deltas
		.iter()
		.map(|&delta| {
			let mut delta = delta.clone();

			// The prior value comes first, since it is the predecessor of the new one in the chain
			for prior in &mut delta.reverse {
				if let Prior::NodeField {
					id,
					field: NodeField::Input { index, previous },
					..
				} = prior
				{
					strip((*id, *index), &mut previous.input);
				}
			}
			if let RegistryDelta::ChangeNodeInput { id, index, new_input } = &mut delta.kind {
				strip((*id, *index), new_input);
			}

			delta
		})
		.collect();

	let stripped_hot_log = hot_log
		.iter()
		.map(|hot_op| {
			let mut hot_op = hot_op.clone();
			if let RegistryDelta::ChangeNodeInput { id, index, new_input } = &mut hot_op.op {
				strip((*id, *index), new_input);
			}
			hot_op
		})
		.collect();

	(stripped, stripped_hot_log, chains)
}

fn hot_log_frames<'a>(hot_ops: impl Iterator<Item = &'a HotOp>) -> Vec<u8> {
	let mut buffer = Vec::new();
	for hot_op in hot_ops {
		document_format::Codec::PostcardFrames.append(&mut buffer, hot_op).unwrap();
	}
	buffer
}

fn history_frames<'a>(deltas: impl Iterator<Item = &'a Delta>) -> Vec<u8> {
	let mut buffer = Vec::new();
	for delta in deltas {
		document_format::Codec::PostcardFrames.append(&mut buffer, delta).unwrap();
	}
	buffer
}

pub fn deflate(bytes: &[u8]) -> Vec<u8> {
	let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), Compression::default());
	encoder.write_all(bytes).unwrap();
	encoder.finish().unwrap()
}

/// Block size the predecessor is indexed at, as in git's delta encoder.
const COPY_BLOCK: usize = 16;
const COPY: u8 = 0;
const INSERT: u8 = 1;

/// Each version as git-style copy and insert instructions against its predecessor, deflated. Pure Rust, O(n) to
/// encode with a block index of the predecessor, and decoding is a plain copy loop.
fn copy_chain(chain: &[Vec<u8>]) -> usize {
	let mut previous: &[u8] = &[];
	let mut total = 0;
	for current in chain {
		let delta = copy_delta(previous, current);
		assert_eq!(&apply_copy_delta(previous, &delta), current, "copy delta does not round trip");
		total += deflate(&delta).len();
		previous = current;
	}
	total
}

fn copy_delta(base: &[u8], target: &[u8]) -> Vec<u8> {
	let block_key = |bytes: &[u8]| u128::from_le_bytes(bytes[..COPY_BLOCK].try_into().unwrap());

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

fn read_varint(delta: &mut &[u8]) -> usize {
	let mut value = 0;
	let mut shift = 0;
	loop {
		let byte = delta[0];
		*delta = &delta[1..];
		value |= ((byte & 0x7f) as usize) << shift;
		if byte < 0x80 {
			return value;
		}
		shift += 7;
	}
}

fn apply_copy_delta(base: &[u8], mut delta: &[u8]) -> Vec<u8> {
	let mut output = Vec::new();
	while let Some((&tag, rest)) = delta.split_first() {
		delta = rest;
		if tag == COPY {
			let offset = read_varint(&mut delta);
			let length = read_varint(&mut delta);
			output.extend_from_slice(&base[offset..offset + length]);
		} else {
			let length = read_varint(&mut delta);
			output.extend_from_slice(&delta[..length]);
			delta = &delta[length..];
		}
	}
	output
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

/// Each version deflated with the last 32 KiB of its predecessor as a preset dictionary, the most of it deflate's window can see.
fn dictionary_chain(chain: &[Vec<u8>]) -> usize {
	let mut previous: &[u8] = &[];
	let mut total = 0;
	for current in chain {
		let mut compress = Compress::new(Compression::default(), false);
		let dictionary = &previous[previous.len().saturating_sub(32 * 1024)..];
		if !dictionary.is_empty() {
			compress.set_dictionary(dictionary).unwrap();
		}

		let mut output = Vec::with_capacity(current.len() / 2 + 1024);
		loop {
			let consumed = compress.total_in() as usize;
			if compress.compress_vec(&current[consumed..], &mut output, FlushCompress::Finish).unwrap() == Status::StreamEnd {
				break;
			}
			output.reserve(output.capacity());
		}

		total += output.len();
		previous = current;
	}
	total
}

/// How scattered the changes between consecutive versions are: inserted runs and bytes per version, and where in the
/// version the runs start.
pub fn change_locality(chains: &Chains) {
	let mut versions = 0;
	let mut runs = 0;
	let mut inserted = 0;
	let mut length = 0;
	let mut histogram = [0_usize; 6];
	// Runs starting in the first 1%, the middle, and the last 10% of the version
	let mut regions = [0_usize; 3];

	for chain in chains.values() {
		for pair in chain.windows(2) {
			let delta = copy_delta(&pair[0], &pair[1]);
			let mut cursor = delta.as_slice();
			let mut position = 0;
			let mut version_runs = 0;
			while let Some((&tag, rest)) = cursor.split_first() {
				cursor = rest;
				if tag == COPY {
					read_varint(&mut cursor);
					position += read_varint(&mut cursor);
					continue;
				}

				let run_length = read_varint(&mut cursor);
				cursor = &cursor[run_length..];
				let fraction = position as f64 / pair[1].len() as f64;
				regions[if fraction < 0.01 {
					0
				} else if fraction < 0.9 {
					1
				} else {
					2
				}] += 1;
				version_runs += 1;
				inserted += run_length;
				position += run_length;
			}

			versions += 1;
			runs += version_runs;
			length += pair[1].len();
			histogram[version_runs.min(5)] += 1;
		}
	}

	println!();
	println!(
		"change locality over {versions} version pairs: {:.1} inserted runs and {:.0} inserted bytes per version, average version {:.0} bytes",
		runs as f64 / versions as f64,
		inserted as f64 / versions as f64,
		length as f64 / versions as f64
	);
	println!("  versions by inserted run count (0, 1, 2, 3, 4, 5+): {histogram:?}");
	println!("  runs starting in the first 1%, middle, last 10%: {regions:?}");
}

/// Print the paths that differ between two consecutive versions in the middle of the longest chain.
pub fn sample_value_diff(chains: &Chains) {
	let Some(chain) = chains.values().max_by_key(|chain| chain.len()) else { return };
	let middle = chain.len() / 2;
	let before: Value = postcard::from_bytes(&chain[middle]).unwrap();
	let after: Value = postcard::from_bytes(&chain[middle + 1]).unwrap();

	println!();
	println!("changed paths between versions {middle} and {} of the longest chain ({} versions):", middle + 1, chain.len());
	let mut printed = 0;
	diff_values(&before, &after, &mut String::new(), &mut printed);
}

fn diff_values(before: &Value, after: &Value, path: &mut String, printed: &mut usize) {
	if before == after || *printed >= 30 {
		return;
	}

	match (before, after) {
		(Value::Array(before), Value::Array(after)) => {
			if before.len() != after.len() {
				println!("  {path}: array length {} -> {}", before.len(), after.len());
				*printed += 1;
			}
			for (index, (before, after)) in before.iter().zip(after).enumerate() {
				let length = path.len();
				path.push_str(&format!("[{index}]"));
				diff_values(before, after, path, printed);
				path.truncate(length);
			}
		}
		(Value::Object(before), Value::Object(after)) => {
			for ((key, before), (_, after)) in before.iter().zip(after) {
				let length = path.len();
				path.push_str(&format!(".{key}"));
				diff_values(before, after, path, printed);
				path.truncate(length);
			}
		}
		_ => {
			let shorten = |value: &Value| format!("{value:?}").chars().take(60).collect::<String>();
			println!("  {path}: {} -> {}", shorten(before), shorten(after));
			*printed += 1;
		}
	}
}
