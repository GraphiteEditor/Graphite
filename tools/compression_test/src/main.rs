use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use document_container::{AnyContainer, backends::memory::MemoryBackend};
use document_format::{Codec, Gdd, GddV1Layout};
use document_graph_storage::{Delta, NodeInput, RegistryDelta};
use lz4_flex::frame::{BlockMode, FrameEncoder, FrameInfo};

fn main() {
	let document_path = Path::new("value-diff-test.gdd");
	let archive = std::fs::read(document_path).unwrap();
	let memory = MemoryBackend::new();
	let container = AnyContainer::Memory(memory);
	let gdd = futures::executor::block_on(Gdd::open_from_archive(archive.as_ref(), container, GddV1Layout)).unwrap();

	let deltas: Vec<&Delta> = gdd.session().history().collect();

	// What `history.frames` holds today: every delta as a length-prefixed MessagePack frame.
	let mut current = Vec::new();
	for delta in &deltas {
		Codec::MessagePackFrames.append(&mut current, delta).unwrap();
	}
	let frames: Vec<Vec<u8>> = deltas.iter().map(|delta| rmp_serde::to_vec(delta).unwrap()).collect();
	let frames_total: usize = frames.iter().map(Vec::len).sum();

	println!("history: {} deltas", deltas.len());
	println!("archive (.gdd, xz):                       {:>9} bytes", archive.len());
	println!("history.frames as stored today:           {:>9} bytes", current.len());
	println!();

	let report = |label: &str, bytes: usize| {
		println!("{label:<42}{bytes:>9} bytes   {:>6.2}x smaller than today", current.len() as f64 / bytes as f64);
	};

	report("each delta lz4 independently", independent(&frames));
	report("one linked lz4 chain over the whole history", linked_chain(&frames).iter().sum());
	report("linked lz4 chain per node input", per_input_chains(&deltas, &frames));
	report("whole history as one lz4 frame (reference)", lz4_flex::frame::FrameEncoder::new(Vec::new()).write_and_finish(&current));
	report("zstd chain level 3 (16 MiB window)", zstd_chain(&frames, 3).iter().sum());
	report("zstd chain level 9 (16 MiB window)", zstd_chain(&frames, 9).iter().sum());
	report("zstd chain level 19 (16 MiB window)", zstd_chain(&frames, 19).iter().sum());

	// Only the new value: what a store that doesn't persist `reverse` would need.
	let kinds: Vec<Vec<u8>> = deltas.iter().map(|delta| rmp_serde::to_vec(&delta.kind).unwrap()).collect();
	let kinds_total: usize = kinds.iter().map(Vec::len).sum();
	println!();
	println!("`kind` only (dropping `reverse`):         {kinds_total:>9} bytes   ({frames_total} with reverse)");
	report("linked lz4 chain over `kind` only", linked_chain(&kinds).iter().sum());

	// Detail for the most-edited large input.
	let key = |delta: &Delta| match &delta.kind {
		RegistryDelta::ChangeNodeInput {
			id,
			index,
			new_input: NodeInput::Value { .. },
		} => Some((*id, *index)),
		_ => None,
	};
	let mut per_key: HashMap<_, Vec<usize>> = HashMap::new();
	for (i, delta) in deltas.iter().enumerate() {
		if let Some(k) = key(delta) {
			per_key.entry(k).or_default().push(i);
		}
	}
	let (k, indices) = per_key.iter().max_by_key(|(_, v)| v.iter().map(|&i| frames[i].len()).sum::<usize>()).unwrap();
	let payloads: Vec<Vec<u8>> = indices.iter().map(|&i| frames[i].clone()).collect();
	let ind = payloads.iter().map(|p| lz4_flex::block::compress(p).len()).collect::<Vec<_>>();
	let lnk = linked_chain(&payloads);
	let zst = zstd_chain(&payloads, 3);
	let pred = predecessor_only(&payloads, 3);
	println!();
	println!("largest input {k:?}: {} versions", payloads.len());
	println!("  {:>4} {:>8} {:>7} {:>10} {:>10} {:>11} {:>9} {:>10}", "#", "raw", "growth", "lz4-indep", "lz4-suffix", "zstd-suffix", "zstd/new", "pred-only");
	let mut prev_raw = 0;
	for (i, ((((p, ind), lnk), zst), pred)) in payloads.iter().zip(&ind).zip(&lnk).zip(&zst).zip(&pred).enumerate().take(40) {
		let growth = p.len() - prev_raw;
		println!("  {i:>4} {:>8} {growth:>7} {ind:>10} {lnk:>10} {zst:>11} {:>8.1}% {pred:>10}", p.len(), 100.0 * *zst as f64 / growth as f64);
		prev_raw = p.len();
	}
	let growth_total = payloads.last().unwrap().len();
	let zst_total: usize = zst.iter().sum();
	let pred_total: usize = pred.iter().sum();
	println!("  new bytes total: {growth_total}   lz4 suffix total: {}   zstd suffix total: {zst_total}   predecessor-only total: {pred_total}", lnk.iter().sum::<usize>());
	println!("  zstd suffix / new bytes: {:.1}%  (compression overhead relative to storing only the raw new bytes)", 100.0 * zst_total as f64 / growth_total as f64);
	println!("  predecessor-only / new bytes: {:.1}%", 100.0 * pred_total as f64 / growth_total as f64);

	// Predecessor-only over the whole history: group by input, each version sees only the previous
	// version of the same input; non-value deltas are compressed standalone.
	let mut by_input: HashMap<_, Vec<Vec<u8>>> = HashMap::new();
	let mut standalone = Vec::new();
	for (delta, frame) in deltas.iter().zip(&frames) {
		match key(delta) {
			Some(k) => by_input.entry(k).or_default().push(frame.clone()),
			None => standalone.push(frame.clone()),
		}
	}
	let pred_history: usize = by_input.values().map(|p| predecessor_only(p, 3).iter().sum::<usize>()).sum::<usize>() + standalone.iter().map(|f| predecessor_only(std::slice::from_ref(f), 3)[0]).sum::<usize>();
	report("predecessor-only zstd (per input, level 3)", pred_history);

	// Same estimate over the whole history: "new bytes" = growth vs. the previous version of the same
	// input, or the full frame for the first version / non-value deltas.
	let mut last_len: HashMap<_, usize> = HashMap::new();
	let mut new_bytes_total = 0;
	for (delta, frame) in deltas.iter().zip(&frames) {
		new_bytes_total += match key(delta) {
			Some(k) => {
				let prev = last_len.insert(k, frame.len()).unwrap_or(0);
				frame.len().saturating_sub(prev)
			}
			None => frame.len(),
		};
	}
	let zstd_total: usize = zstd_chain(&frames, 3).iter().sum();
	println!();
	println!("whole history: new bytes {new_bytes_total}   zstd chain {zstd_total}   ratio {:.1}%", 100.0 * zstd_total as f64 / new_bytes_total as f64);

	// Micro-bench: time to get at the *last* payload. For the streaming chain that means decoding the
	// entire stream (block n references everything before it); for predecessor-only it means walking the
	// per-input chain from version 0.
	let iterations = 100;
	println!();
	println!("decode-to-last-payload, {iterations} iterations (debug build):");

	let (_, chain_largest) = zstd_chain_with_output(&payloads, 3);
	let expected_last = payloads.last().unwrap().len();
	bench(&format!("zstd chain, largest input ({} versions, {} B)", payloads.len(), chain_largest.len()), iterations, || {
		let decoded = zstd::decode_all(chain_largest.as_slice()).unwrap();
		assert_eq!(decoded.len(), payloads.iter().map(Vec::len).sum::<usize>());
		expected_last
	});

	let pred_frames = predecessor_only_frames(&payloads, 3);
	let pred_bytes: usize = pred_frames.iter().map(Vec::len).sum();
	bench(&format!("predecessor-only, largest input ({} frames, {} B)", pred_frames.len(), pred_bytes), iterations, || {
		let last = decode_predecessor_chain(&pred_frames);
		assert_eq!(&last, payloads.last().unwrap());
		last.len()
	});

	let (_, chain_all) = zstd_chain_with_output(&frames, 3);
	bench(&format!("zstd chain, whole history ({} deltas, {} B)", frames.len(), chain_all.len()), iterations, || {
		let decoded = zstd::decode_all(chain_all.as_slice()).unwrap();
		assert_eq!(decoded.len(), frames_total);
		frames.last().unwrap().len()
	});

	let lz4_all = lz4_flex::frame::FrameEncoder::new(Vec::new()).write_and_finish_bytes(&current);
	bench(&format!("lz4 whole history, for reference ({} B)", lz4_all.len()), iterations, || {
		let mut out = Vec::with_capacity(current.len());
		std::io::copy(&mut lz4_flex::frame::FrameDecoder::new(lz4_all.as_slice()), &mut out).unwrap();
		out.len()
	});
}

trait WriteAndFinish {
	fn write_and_finish_bytes(self, bytes: &[u8]) -> Vec<u8>;
	fn write_and_finish(self, bytes: &[u8]) -> usize;
}
impl WriteAndFinish for FrameEncoder<Vec<u8>> {
	fn write_and_finish_bytes(mut self, bytes: &[u8]) -> Vec<u8> {
		self.write_all(bytes).unwrap();
		self.finish().unwrap()
	}
	fn write_and_finish(self, bytes: &[u8]) -> usize {
		self.write_and_finish_bytes(bytes).len()
	}
}

/// Baseline: every payload compressed on its own, no shared context.
fn independent(payloads: &[Vec<u8>]) -> usize {
	payloads.iter().map(|p| lz4_flex::block::compress(p).len()).sum()
}

/// Streaming approach: one linked frame, `flush()` after each payload.
/// The bytes emitted between two flushes are the suffix that would be stored for that block.
fn linked_chain(payloads: &[Vec<u8>]) -> Vec<usize> {
	let mut frame_info = FrameInfo::new();
	frame_info.block_mode = BlockMode::Linked;
	let mut encoder = FrameEncoder::with_frame_info(frame_info, Vec::new());

	let mut sizes = Vec::with_capacity(payloads.len());
	let mut previous = 0;
	for p in payloads {
		encoder.write_all(p).unwrap();
		encoder.flush().unwrap();
		let len = encoder.get_ref().len();
		sizes.push(len - previous);
		previous = len;
	}
	// The end mark is stored once for the whole chain, not attributed to any block.
	encoder.finish().unwrap();
	sizes
}

/// Same as [`linked_chain`] but with zstd streaming and a window large enough to hold many
/// previous versions (`window_log` 24 = 16 MiB). `flush()` emits a zstd block boundary without
/// resetting the window, so the bytes between flushes are the suffix for that payload.
fn zstd_chain(payloads: &[Vec<u8>], level: i32) -> Vec<usize> {
	zstd_chain_with_output(payloads, level).0
}

fn zstd_chain_with_output(payloads: &[Vec<u8>], level: i32) -> (Vec<usize>, Vec<u8>) {
	let mut encoder = zstd::Encoder::new(Vec::new(), level).unwrap();
	encoder.window_log(24).unwrap();
	// Long-distance matching finds the "previous payload as prefix" match even far back.
	encoder.long_distance_matching(true).unwrap();

	let mut sizes = Vec::with_capacity(payloads.len());
	let mut previous = 0;
	for p in payloads {
		encoder.write_all(p).unwrap();
		encoder.flush().unwrap();
		let len = encoder.get_ref().len();
		sizes.push(len - previous);
		previous = len;
	}
	let out = encoder.finish().unwrap();
	(sizes, out)
}

/// Predecessor-only, but also returning the compressed frames so they can be decoded back.
fn predecessor_only_frames(payloads: &[Vec<u8>], level: i32) -> Vec<Vec<u8>> {
	use zstd_safe::{CCtx, CParameter};
	let mut frames = Vec::with_capacity(payloads.len());
	let mut previous: &[u8] = &[];
	for p in payloads {
		let mut cctx = CCtx::create();
		cctx.set_parameter(CParameter::CompressionLevel(level)).unwrap();
		cctx.set_parameter(CParameter::WindowLog(24)).unwrap();
		cctx.set_parameter(CParameter::EnableLongDistanceMatching(true)).unwrap();
		cctx.ref_prefix(previous).unwrap();
		let mut out = Vec::with_capacity(zstd_safe::compress_bound(p.len()));
		cctx.compress2(&mut out, p).unwrap();
		frames.push(out);
		previous = p;
	}
	frames
}

/// Walk a predecessor-only chain from the first frame to the last, returning the last payload.
fn decode_predecessor_chain(frames: &[Vec<u8>]) -> Vec<u8> {
	use zstd_safe::DCtx;
	let mut previous = Vec::new();
	for frame in frames {
		let size = zstd_safe::get_frame_content_size(frame).unwrap().unwrap() as usize;
		let mut out = Vec::with_capacity(size);
		{
			// `DCtx<'a>` borrows the prefix until dropped, so it must go before `previous` is reassigned.
			let mut dctx = DCtx::create();
			dctx.set_parameter(zstd_safe::DParameter::WindowLogMax(24)).unwrap();
			dctx.ref_prefix(&previous).unwrap();
			dctx.decompress(&mut out, frame).unwrap();
		}
		previous = out;
	}
	previous
}

fn bench(label: &str, iterations: u32, mut f: impl FnMut() -> usize) {
	let mut checksum = 0;
	let start = std::time::Instant::now();
	for _ in 0..iterations {
		checksum = checksum.max(f());
	}
	let elapsed = start.elapsed();
	println!("  {label:<52} {:>9.3} ms / iter   (last payload {checksum} bytes)", elapsed.as_secs_f64() * 1000.0 / iterations as f64);
}

/// Each payload compressed as its own zstd frame, with only its immediate predecessor supplied as a
/// prefix dictionary (`ZSTD_CCtx_refPrefix`). No stream state is shared: decoding version `n` needs
/// just the raw bytes of version `n-1`. `window_log` must cover prefix + payload for the match to be
/// representable.
fn predecessor_only(payloads: &[Vec<u8>], level: i32) -> Vec<usize> {
	use zstd_safe::{CCtx, CParameter};
	let mut sizes = Vec::with_capacity(payloads.len());
	let mut previous: &[u8] = &[];
	let mut out = Vec::new();
	for p in payloads {
		let mut cctx = CCtx::create();
		cctx.set_parameter(CParameter::CompressionLevel(level)).unwrap();
		cctx.set_parameter(CParameter::WindowLog(24)).unwrap();
		cctx.set_parameter(CParameter::EnableLongDistanceMatching(true)).unwrap();
		cctx.ref_prefix(previous).unwrap();
		out.clear();
		out.reserve(zstd_safe::compress_bound(p.len()));
		let n = cctx.compress2(&mut out, p).unwrap();
		sizes.push(n);
		previous = p;
	}
	sizes
}

/// One linked chain per `(node, input)` for value changes; everything else goes into a shared chain.
/// Keeps consecutive versions of the same value adjacent in the window regardless of interleaving.
fn per_input_chains(deltas: &[&Delta], frames: &[Vec<u8>]) -> usize {
	let mut groups: HashMap<Option<(document_graph_storage::NodeId, u32)>, Vec<Vec<u8>>> = HashMap::new();
	for (delta, frame) in deltas.iter().zip(frames) {
		let key = match &delta.kind {
			RegistryDelta::ChangeNodeInput {
				id,
				index,
				new_input: NodeInput::Value { .. },
			} => Some((*id, *index)),
			_ => None,
		};
		groups.entry(key).or_default().push(frame.clone());
	}
	groups.values().map(|payloads| linked_chain(payloads).iter().sum::<usize>()).sum()
}
