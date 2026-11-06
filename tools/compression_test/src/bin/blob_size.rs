//! Compares a document's stored size with node input values inline against the blob store, uncompressed and packed as zip and xz.

use std::path::Path;

use document_container::archive::{Archive, ArchiveWriter, Xz, Zip};
use document_container::{AnyContainer, AsyncContainer, backends::memory::MemoryBackend};
use document_format::{Gdd, GddV1Layout};

// Format steps register when linked
use migration_formats as _;

/// Every file of a working copy, with its bytes.
async fn files(working: &AnyContainer) -> Vec<(String, Vec<u8>)> {
	let mut paths = working.list("").await.unwrap_or_default();
	for directory in working.list_dirs("").await.unwrap_or_default() {
		paths.extend(working.list(&directory).await.unwrap_or_default());
	}
	paths.sort();

	let mut files = Vec::new();
	for path in paths {
		let bytes = working.read(&path).await.unwrap().as_slice().to_vec();
		files.push((path, bytes));
	}
	files
}

fn pack<A: Archive>(files: &[(String, Vec<u8>)]) -> usize
where
	A::Writer<std::io::Cursor<Vec<u8>>>: ArchiveWriter<Sink = std::io::Cursor<Vec<u8>>>,
{
	let mut writer = A::writer(std::io::Cursor::new(Vec::new())).unwrap();
	for (path, bytes) in files {
		writer.write_entry(path, bytes).unwrap();
	}
	writer.finish_into().unwrap().into_inner().len()
}

fn report(label: &str, files: &[(String, Vec<u8>)]) -> [usize; 3] {
	let sizes = [files.iter().map(|(_, bytes)| bytes.len()).sum(), pack::<Zip>(files), pack::<Xz>(files)];
	println!("{label:<34}{:>12}{:>12}{:>12}", sizes[0], sizes[1], sizes[2]);
	sizes
}

fn main() {
	let document_path = std::env::args().nth(1).unwrap_or_else(|| "Brush Big.gdd".to_string());
	let archive = std::fs::read(Path::new(&document_path)).unwrap();

	futures::executor::block_on(async {
		let mut inline = AnyContainer::Memory(MemoryBackend::new());
		document_container::archive::open_auto(&archive, &mut inline).unwrap();
		let inline_files = files(&inline).await;

		let gdd = Gdd::open_from_archive(archive.as_ref(), AnyContainer::Memory(MemoryBackend::new()), GddV1Layout).await.unwrap();
		let values: Vec<usize> = gdd.blobs().values().map(Vec::len).collect();
		println!(
			"blob values: {}, mean {} bytes, max {} bytes",
			values.len(),
			values.iter().sum::<usize>() / values.len().max(1),
			values.iter().max().unwrap_or(&0)
		);
		let (working, _) = gdd.into_storage();
		let blob_files = files(&working).await;

		let without_legacy = |files: &[(String, Vec<u8>)]| files.iter().filter(|(path, _)| !path.starts_with("legacy")).cloned().collect::<Vec<_>>();

		println!("{:<34}{:>12}{:>12}{:>12}", "bytes", "uncompressed", "zip", "xz");
		let inline_sizes = report("values inline", &inline_files);
		let blob_sizes = report("blob store", &blob_files);
		let inline_core = report("values inline, no legacy.graphite", &without_legacy(&inline_files));
		let blob_core = report("blob store, no legacy.graphite", &without_legacy(&blob_files));

		archive_variants(&blob_files);

		for (label, before, after) in [("ratio", inline_sizes, blob_sizes), ("ratio, no legacy.graphite", inline_core, blob_core)] {
			let ratio = |i: usize| before[i] as f64 / after[i] as f64;
			println!("{label:<34}{:>11.2}x{:>11.2}x{:>11.2}x", ratio(0), ratio(1), ratio(2));
		}
	});
}

/// A blob record's kind byte and its base hash bytes if it has one.
fn split_record(record: &[u8]) -> (u8, &[u8]) {
	let header = if record[0] == 1 { 33 } else { 1 };
	(record[0], &record[1..header])
}

/// Records ordered so each follows its base, chain by chain, which keeps related bytes close for xz's window.
fn chain_order(blobs: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
	use std::collections::HashMap;

	let name = |path: &str| path.rsplit('/').next().unwrap().to_string();
	let mut children: HashMap<String, Vec<usize>> = HashMap::new();
	let mut roots = Vec::new();
	for (index, (_, record)) in blobs.iter().enumerate() {
		let (kind, base) = split_record(record);
		match kind {
			1 => children.entry(base.iter().map(|byte| format!("{byte:02x}")).collect()).or_default().push(index),
			_ => roots.push(index),
		}
	}

	let mut ordered = Vec::new();
	let mut pending: Vec<usize> = roots.into_iter().rev().collect();
	while let Some(index) = pending.pop() {
		ordered.push(blobs[index].clone());
		pending.extend(children.get(&name(&blobs[index].0)).into_iter().flatten().rev());
	}
	ordered
}

pub fn archive_variants(files: &[(String, Vec<u8>)]) {
	let (blobs, rest): (Vec<_>, Vec<_>) = files.iter().filter(|(path, _)| !path.starts_with("legacy")).cloned().partition(|(path, _)| path.starts_with("blobs/"));
	let packed = |records: &[(String, Vec<u8>)]| {
		let mut pack = Vec::new();
		for (path, record) in records {
			pack.extend_from_slice(path.as_bytes());
			pack.extend_from_slice(&(record.len() as u32).to_le_bytes());
			pack.extend_from_slice(record);
		}
		("blobs.pack".to_string(), pack)
	};

	let deltas: Vec<usize> = blobs.iter().filter(|(_, record)| record[0] == 1).map(|(_, record)| record.len()).collect();
	let mut sorted = deltas.clone();
	sorted.sort_unstable();
	println!(
		"\ndelta records: {}, mean {} bytes, median {} bytes, max {} bytes",
		deltas.len(),
		deltas.iter().sum::<usize>() / deltas.len().max(1),
		sorted.get(sorted.len() / 2).unwrap_or(&0),
		sorted.last().unwrap_or(&0)
	);
	let ordered = chain_order(&blobs);
	let with = |blob_files: Vec<(String, Vec<u8>)>| rest.iter().cloned().chain(blob_files).collect::<Vec<_>>();

	println!("\nno legacy.graphite, by blob layout");
	report("files (current)", &with(blobs.clone()));
	report("one pack, hash order", &with(vec![packed(&blobs)]));
	report("one pack, chain order", &with(vec![packed(&ordered)]));
}
