//! Thread-local cache for `ReadFontsRef::from_index` + `outline_glyphs()` keyed by blob id + face index.
//!
//! `OutlineGlyphCollection` borrows font bytes; we keep a clone of [`FontData`] and transmute the
//! collection to `'static` so the borrow remains valid until the slot is dropped (`outlines` before `_font`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use parley::FontData;
use skrifa::raw::FontRef as ReadFontsRef;
use skrifa::{MetadataProvider, OutlineGlyphCollection};

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct OutlineFontKey {
	blob_id: u64,
	index: u32,
}

thread_local! {
	static OUTLINES: RefCell<HashMap<OutlineFontKey, Rc<FontOutlineSlot>>> = RefCell::new(HashMap::new());
}

pub(crate) struct FontOutlineSlot {
	_font: FontData,
	// Declared after `_font` so this is dropped first (before font bytes go away).
	outlines: OutlineGlyphCollection<'static>,
}

impl FontOutlineSlot {
	fn new(font: FontData) -> Self {
		let font_ref = ReadFontsRef::from_index(font.data.as_ref(), font.index).unwrap();
		let outlines = font_ref.outline_glyphs();
		let outlines = unsafe {
			// SAFETY: `_font` outlives `outlines`; `outlines` only reads bytes in `_font.data`.
			std::mem::transmute::<OutlineGlyphCollection<'_>, OutlineGlyphCollection<'static>>(outlines)
		};
		Self { _font: font, outlines }
	}

	pub(crate) fn outlines(&self) -> &OutlineGlyphCollection<'static> {
		&self.outlines
	}
}

/// Shared outline collection for this font run; parses only on first use per thread + key.
pub(crate) fn outlines_for_font(font: &FontData) -> Rc<FontOutlineSlot> {
	let key = OutlineFontKey {
		blob_id: font.data.id(),
		index: font.index,
	};
	OUTLINES.with(|map| {
		let mut map = map.borrow_mut();
		if let Some(slot) = map.get(&key) {
			#[cfg(feature = "perf-stats")]
			crate::glyph_run_perf::record_outline_cache_hit();
			return slot.clone();
		}
		#[cfg(feature = "perf-stats")]
		crate::glyph_run_perf::record_outline_cache_miss();
		let slot = Rc::new(FontOutlineSlot::new(font.clone()));
		map.insert(key, slot.clone());
		slot
	})
}
