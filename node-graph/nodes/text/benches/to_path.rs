//! Benchmark: `TextContext::to_path` on a long wrapped paragraph (many glyph runs, one font).
//!
//! Fixture: Fira Sans Regular (SIL OFL), copied from the `fontsan` crate test data for stable offline benchmarking.
//!
//! Run (wall-clock only, default build):
//!   cargo bench -p text-nodes --bench to_path
//!
//! Run with per-glyph-run breakdown printed once to stderr:
//!   cargo bench -p text-nodes --bench to_path --features perf-stats

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use text_nodes::{Font, FontCache, TextContext, TypesettingConfig};

// ~190 KiB — OFL-licensed test font from fontsan (same file Mozilla ships for Fira Sans).
static FONT_BYTES: &[u8] = include_bytes!("fixtures/FiraSans-Regular.ttf");

fn bench_setup() -> (TextContext, Font, FontCache, TypesettingConfig, String) {
	let font = Font::new("Fira Sans".into(), "Regular".into());
	let mut cache = FontCache::default();
	cache.insert(font.clone(), FONT_BYTES.to_vec());

	let mut typesetting = TypesettingConfig::default();
	typesetting.max_width = Some(240.);

	let unit = "The quick brown fox jumps over the lazy dog. ";
	let text = unit.repeat(80);

	(TextContext::default(), font, cache, typesetting, text)
}

fn text_to_path_wrapped(c: &mut Criterion) {
	let (mut ctx, font, cache, typesetting, text) = bench_setup();

	#[cfg(feature = "perf-stats")]
	{
		text_nodes::glyph_run_perf::reset();
		let _ = ctx.to_path::<()>(text.as_str(), &font, &cache, typesetting, false);
		let snap = text_nodes::glyph_run_perf::snapshot();
		eprintln!("{}", snap.summary_line());
	}

	let mut group = c.benchmark_group("text_to_path");
	group.bench_function("wrapped_paragraph_same_font", |b| {
		b.iter(|| {
			let table = ctx.to_path::<()>(black_box(text.as_str()), black_box(&font), black_box(&cache), black_box(typesetting), black_box(false));
			black_box(table);
		});
	});
	group.finish();
}

criterion_group!(benches, text_to_path_wrapped);
criterion_main!(benches);
