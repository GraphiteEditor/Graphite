//! Cumulative timings for work inside each [`crate::path_builder::PathBuilder::render_glyph_run`] call.
//!
//! Build with `--features perf-stats`, then call [`reset`] before a workload and [`snapshot`] after
//! to read totals (or use `cargo bench -p text-nodes --bench to_path --features perf-stats`).

#[cfg(feature = "perf-stats")]
mod imp {
	use core::sync::atomic::{AtomicU64, Ordering};

	static GLYPH_RUN_CALLS: AtomicU64 = AtomicU64::new(0);
	static NORMALIZED_COORDS_NS: AtomicU64 = AtomicU64::new(0);
	static FONT_REF_OUTLINE_NS: AtomicU64 = AtomicU64::new(0);
	static OUTLINE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
	static OUTLINE_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub struct GlyphRunPerfSnapshot {
		pub glyph_run_calls: u64,
		pub normalized_coords_total_ns: u64,
		pub font_ref_outline_total_ns: u64,
		pub outline_cache_hits: u64,
		pub outline_cache_misses: u64,
	}

	pub fn reset() {
		GLYPH_RUN_CALLS.store(0, Ordering::Relaxed);
		NORMALIZED_COORDS_NS.store(0, Ordering::Relaxed);
		FONT_REF_OUTLINE_NS.store(0, Ordering::Relaxed);
		OUTLINE_CACHE_HITS.store(0, Ordering::Relaxed);
		OUTLINE_CACHE_MISSES.store(0, Ordering::Relaxed);
	}

	#[inline]
	pub(crate) fn record_glyph_run(normalized_coords_ns: u64, font_ref_outline_ns: u64) {
		GLYPH_RUN_CALLS.fetch_add(1, Ordering::Relaxed);
		NORMALIZED_COORDS_NS.fetch_add(normalized_coords_ns, Ordering::Relaxed);
		FONT_REF_OUTLINE_NS.fetch_add(font_ref_outline_ns, Ordering::Relaxed);
	}

	#[inline]
	pub(crate) fn record_outline_cache_hit() {
		OUTLINE_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
	}

	#[inline]
	pub(crate) fn record_outline_cache_miss() {
		OUTLINE_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
	}

	pub fn snapshot() -> GlyphRunPerfSnapshot {
		GlyphRunPerfSnapshot {
			glyph_run_calls: GLYPH_RUN_CALLS.load(Ordering::Relaxed),
			normalized_coords_total_ns: NORMALIZED_COORDS_NS.load(Ordering::Relaxed),
			font_ref_outline_total_ns: FONT_REF_OUTLINE_NS.load(Ordering::Relaxed),
			outline_cache_hits: OUTLINE_CACHE_HITS.load(Ordering::Relaxed),
			outline_cache_misses: OUTLINE_CACHE_MISSES.load(Ordering::Relaxed),
		}
	}

	impl GlyphRunPerfSnapshot {
		pub fn summary_line(&self) -> String {
			let n = self.glyph_run_calls.max(1);
			format!(
				"glyph_run_perf: {} runs | normalized_coords avg {:.2} µs | outline_lookup avg {:.2} µs (cached from_index+outline_glyphs) | outline_cache hits {} misses {}",
				self.glyph_run_calls,
				self.normalized_coords_total_ns as f64 / n as f64 / 1000.0,
				self.font_ref_outline_total_ns as f64 / n as f64 / 1000.0,
				self.outline_cache_hits,
				self.outline_cache_misses,
			)
		}
	}
}

#[cfg(not(feature = "perf-stats"))]
mod imp {
	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub struct GlyphRunPerfSnapshot {
		pub glyph_run_calls: u64,
		pub normalized_coords_total_ns: u64,
		pub font_ref_outline_total_ns: u64,
		pub outline_cache_hits: u64,
		pub outline_cache_misses: u64,
	}

	pub fn reset() {}

	#[inline]
	pub(crate) fn record_glyph_run(_normalized_coords_ns: u64, _font_ref_outline_ns: u64) {}

	#[inline]
	#[allow(dead_code)] // Only called from `outline_cache` when `perf-stats` is enabled.
	pub(crate) fn record_outline_cache_hit() {}

	#[inline]
	#[allow(dead_code)]
	pub(crate) fn record_outline_cache_miss() {}

	pub fn snapshot() -> GlyphRunPerfSnapshot {
		GlyphRunPerfSnapshot {
			glyph_run_calls: 0,
			normalized_coords_total_ns: 0,
			font_ref_outline_total_ns: 0,
			outline_cache_hits: 0,
			outline_cache_misses: 0,
		}
	}

	impl GlyphRunPerfSnapshot {
		pub fn summary_line(&self) -> String {
			String::from("glyph_run_perf: (enable `perf-stats` feature for breakdown)")
		}
	}
}

pub(crate) use imp::record_glyph_run;
pub use imp::{GlyphRunPerfSnapshot, reset, snapshot};
#[cfg(feature = "perf-stats")]
pub(crate) use imp::{record_outline_cache_hit, record_outline_cache_miss};
