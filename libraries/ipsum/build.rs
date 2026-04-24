use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, Write};

// The canonical Lorem Ipsum placeholder text. This is the single source of truth: build.rs reads
// it to compute CLASSIC_WORD_INDICES, and emits it verbatim into chain_data.rs so lib.rs can use
// it without duplication. Sentence boundaries fall at words 19, 36, 52, and 69 (the full text).
const CLASSIC_LOREM_IPSUM_TEXT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

const SENTENCE_ENDERS: [char; 3] = ['.', '!', '?'];

fn main() {
	println!("cargo::rerun-if-changed=corpus.txt");

	let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
	let out_dir = std::env::var("OUT_DIR").unwrap();

	let corpus = std::fs::read_to_string(format!("{manifest_dir}/corpus.txt")).expect("Failed to read corpus.txt");

	// Preprocess the corpus into a flat stream of lowercased word tokens. Paragraph structure
	// is deliberately discarded: the Markov chain can't faithfully reproduce the corpus's
	// paragraph-length distribution (its '\n' emission rate diverges from training statistics),
	// so paragraph breaks are instead inserted at runtime using a geometric distribution that
	// matches the target modern English body text profile.
	let without_quotes = strip_quotation_marks(&corpus);
	let lowercased = lowercase_all(&without_quotes);
	let tokens_all = lowercased.split_whitespace().collect::<Vec<_>>();

	// Count word frequencies to determine which vocabulary to keep.
	// Because WORD_DATA (all unique words concatenated) must fit in a u16 offset table,
	// we select as many high-frequency words as possible within the 65,535-byte cap
	let mut word_freq: HashMap<&str, u32> = HashMap::new();
	for &token in &tokens_all {
		*word_freq.entry(token).or_insert(0) += 1;
	}

	// Sort by descending frequency, then alphabetically for determinism at equal frequencies
	let mut words_by_freq: Vec<(&str, u32)> = word_freq.into_iter().collect();
	words_by_freq.sort_unstable_by(|(word_a, freq_a), (word_b, freq_b)| freq_b.cmp(freq_a).then(word_a.cmp(word_b)));

	// Greedily fill the vocabulary budget with the most frequent words
	let mut vocab: HashSet<&str> = HashSet::new();
	let mut vocab_byte_total: usize = 0;
	for &(word, _) in &words_by_freq {
		if vocab_byte_total + word.len() > u16::MAX as usize {
			break;
		}
		vocab.insert(word);
		vocab_byte_total += word.len();
	}

	// Assign each in-vocabulary token an optional index across the whole stream
	let mut words: Vec<&str> = Vec::new();
	let mut word_to_index: HashMap<&str, u16> = HashMap::new();
	let token_indices: Vec<Option<u16>> = tokens_all.iter().map(|&token| vocab.contains(token).then(|| intern(token, &mut words, &mut word_to_index))).collect();

	assert!(words.len() <= u16::MAX as usize, "Corpus vocabulary exceeds u16 capacity ({} unique words)", words.len());

	// Build the bigram transition table from windows of three consecutive in-vocabulary tokens.
	// Triples containing an out-of-vocabulary word are silently skipped.
	// Duplicate entries are intentional: they implement weighted random selection
	let mut transition_map: HashMap<(u16, u16), Vec<u16>> = HashMap::new();
	for window in token_indices.windows(3) {
		if let [Some(a), Some(b), Some(c)] = window[..] {
			transition_map.entry((a, b)).or_default().push(c);
		}
	}

	// Collect restart bigrams used at dead-ends and as initial seeds: the first bigram of the
	// corpus plus any in-vocab bigram immediately following a sentence-ender ('.' '!' '?')
	let mut starters: Vec<(u16, u16)> = Vec::new();
	if let [Some(a), Some(b), ..] = token_indices[..] {
		starters.push((a, b));
	}
	for i in 0..token_indices.len().saturating_sub(2) {
		if tokens_all[i].ends_with(SENTENCE_ENDERS)
			&& let (Some(a), Some(b)) = (token_indices[i + 1], token_indices[i + 2])
		{
			starters.push((a, b));
		}
	}
	// Keep only starters whose bigram has at least one successor in the transition table.
	// Otherwise a runtime dead-end reseeding into a productive-looking starter could loop
	// indefinitely if that starter itself has no transitions
	starters.retain(|&(a, b)| transition_map.contains_key(&(a, b)));
	starters.sort_unstable();
	starters.dedup();

	// Detect and remove bigrams that belong to unescapable cycles — Markov states from which
	// the chain can never reach a dead end (empty successors), meaning randomness has no chance
	// of escaping. We compute the "escapable" set via fixed-point iteration (all states that
	// can eventually reach a dead end), then prune the inescapable remainder so the runtime
	// chain always eventually terminates
	let all_bigrams: HashSet<(u16, u16)> = transition_map.keys().copied().collect();

	// Base case: bigrams with at least one successor that leads to a dead-end (not in transition_map)
	let mut escapable: HashSet<(u16, u16)> = HashSet::new();
	for (&(a, b), followers) in &transition_map {
		if followers.iter().any(|&c| !all_bigrams.contains(&(b, c))) {
			escapable.insert((a, b));
		}
	}

	// Fixed-point: (a, b) is escapable if any successor c makes (b, c) escapable
	loop {
		let newly_escapable: Vec<(u16, u16)> = transition_map
			.iter()
			.filter(|&(&(a, b), followers)| !escapable.contains(&(a, b)) && followers.iter().any(|&c| escapable.contains(&(b, c))))
			.map(|(&bigram, _)| bigram)
			.collect();
		if newly_escapable.is_empty() {
			break;
		}
		escapable.extend(newly_escapable);
	}

	// Prune inescapable bigrams: keep only successors that lead to escapable states or dead ends.
	// Any bigram whose entire successor list was cycle-internal becomes a dead end and is removed
	// entirely (runtime will restart from STARTERS, so a missing key is equivalent to dead end)
	let inescapable_bigrams: Vec<(u16, u16)> = all_bigrams.iter().filter(|b| !escapable.contains(b)).copied().collect();
	if !inescapable_bigrams.is_empty() {
		for bigram @ (_, b) in inescapable_bigrams {
			if let Some(followers) = transition_map.get_mut(&bigram) {
				followers.retain(|&c| escapable.contains(&(b, c)) || !all_bigrams.contains(&(b, c)));
			}
			if transition_map.get(&bigram).is_some_and(|followers| followers.is_empty()) {
				transition_map.remove(&bigram);
			}
		}
		// Re-filter starters in case any were pruned into dead ends
		starters.retain(|&(a, b)| transition_map.contains_key(&(a, b)));
	}

	// Sort transitions by packed bigram key (word_a << 16 | word_b) for binary search at runtime.
	// Split into two tables: single-successor bigrams (89% of all bigrams) store just key + value,
	// while multi-successor bigrams use the traditional key + range-into-pool layout. This saves
	// ~49 KB because single-successor entries avoid the 2-byte TRANSITION_STARTS overhead
	let mut sorted_transitions: Vec<((u16, u16), Vec<u16>)> = transition_map.into_iter().collect();
	sorted_transitions.sort_unstable_by_key(|&((a, b), _)| (a as u32) << 16 | b as u32);

	let mut single_keys: Vec<u32> = Vec::new();
	let mut single_values: Vec<u16> = Vec::new();
	let mut multi_keys: Vec<u32> = Vec::new();
	let mut multi_starts: Vec<u16> = Vec::new();
	let mut successors: Vec<u16> = Vec::new();

	for ((a, b), followers) in &sorted_transitions {
		let key = (*a as u32) << 16 | *b as u32;
		if followers.len() == 1 {
			single_keys.push(key);
			single_values.push(followers[0]);
		} else {
			multi_keys.push(key);
			multi_starts.push(successors.len() as u16);
			successors.extend_from_slice(followers);
		}
	}
	multi_starts.push(successors.len() as u16); // Sentinel for the final entry's range end

	assert!(successors.len() <= u16::MAX as usize, "SUCCESSORS pool ({}) exceeds u16 range", successors.len());

	// Build the concatenated word string and u16 byte-offset table.
	// Storing an (offset, length) struct would waste 2 bytes per word to alignment padding,
	// so instead we store n+1 offsets and derive each word's length as offsets[i+1] - offsets[i]
	let word_data: String = words.iter().copied().collect();
	assert!(word_data.len() <= u16::MAX as usize, "WORD_DATA ({} bytes) exceeds u16 range", word_data.len());

	let mut word_offsets: Vec<u16> = Vec::with_capacity(words.len() + 1);
	let mut byte_offset: u16 = 0;
	for word in &words {
		word_offsets.push(byte_offset);
		byte_offset += word.len() as u16;
	}
	word_offsets.push(byte_offset); // Sentinel: WORD_DATA.len()

	// Write chain_data.rs into OUT_DIR for inclusion by lib.rs
	let out_path = format!("{out_dir}/chain_data.rs");
	let file = std::fs::File::create(&out_path).expect("Failed to create chain_data.rs");
	let mut out = BufWriter::new(file);

	writeln!(out, "// Auto-generated by build.rs from corpus files. Do not edit.").unwrap();
	writeln!(out).unwrap();

	writeln!(out, "const SENTENCE_ENDERS: [char; 3] = {:?};", SENTENCE_ENDERS).unwrap();
	writeln!(out).unwrap();

	// All words concatenated into one string; WORD_OFFSETS[i..i+1] gives word i's byte range
	writeln!(out, "static WORD_DATA: &str = {:?};", word_data).unwrap();
	writeln!(out).unwrap();

	writeln!(out, "static WORD_OFFSETS: &[u16] = &[").unwrap();
	for chunk in word_offsets.chunks(16) {
		write!(out, "   ").unwrap();
		for &offset in chunk {
			write!(out, " {offset},").unwrap();
		}
		writeln!(out).unwrap();
	}
	writeln!(out, "];").unwrap();
	writeln!(out).unwrap();

	// STARTERS as u16 indices into the combined transition tables (single + multi).
	// Every starter bigram is guaranteed to exist in exactly one of the two tables,
	// so we store a single u16 index that the runtime resolves back to a (u16, u16) pair.
	// The index space is: [0..single_keys.len()) for single-successor bigrams, then
	// [single_keys.len()..single_keys.len()+multi_keys.len()) for multi-successor bigrams
	writeln!(out, "static STARTERS: &[u16] = &[").unwrap();
	for &(a, b) in &starters {
		let key = (a as u32) << 16 | b as u32;
		let index = if let Ok(i) = single_keys.binary_search(&key) {
			i
		} else if let Ok(i) = multi_keys.binary_search(&key) {
			single_keys.len() + i
		} else {
			panic!("Starter bigram ({a}, {b}) not found in either transition table");
		};
		writeln!(out, "    {index},").unwrap();
	}
	writeln!(out, "];").unwrap();
	writeln!(out).unwrap();

	// Single-successor bigrams: packed key + single value. 89% of all bigrams fall here
	writeln!(out, "static SINGLE_KEYS: &[u32] = &[").unwrap();
	for chunk in single_keys.chunks(8) {
		write!(out, "   ").unwrap();
		for &key in chunk {
			write!(out, " {key},").unwrap();
		}
		writeln!(out).unwrap();
	}
	writeln!(out, "];").unwrap();
	writeln!(out).unwrap();

	writeln!(out, "static SINGLE_VALUES: &[u16] = &[").unwrap();
	for chunk in single_values.chunks(16) {
		write!(out, "   ").unwrap();
		for &val in chunk {
			write!(out, " {val},").unwrap();
		}
		writeln!(out).unwrap();
	}
	writeln!(out, "];").unwrap();
	writeln!(out).unwrap();

	// Multi-successor bigrams: packed key + range into SUCCESSORS pool
	writeln!(out, "static MULTI_KEYS: &[u32] = &[").unwrap();
	for chunk in multi_keys.chunks(8) {
		write!(out, "   ").unwrap();
		for &key in chunk {
			write!(out, " {key},").unwrap();
		}
		writeln!(out).unwrap();
	}
	writeln!(out, "];").unwrap();
	writeln!(out).unwrap();

	writeln!(out, "static MULTI_STARTS: &[u16] = &[").unwrap();
	for chunk in multi_starts.chunks(16) {
		write!(out, "   ").unwrap();
		for &start in chunk {
			write!(out, " {start},").unwrap();
		}
		writeln!(out).unwrap();
	}
	writeln!(out, "];").unwrap();
	writeln!(out).unwrap();

	writeln!(out, "static SUCCESSORS: &[u16] = &[").unwrap();
	for chunk in successors.chunks(16) {
		write!(out, "   ").unwrap();
		for &index in chunk {
			write!(out, " {index},").unwrap();
		}
		writeln!(out).unwrap();
	}
	writeln!(out, "];").unwrap();
	writeln!(out).unwrap();

	// Emit the classic text itself so lib.rs can use it without duplication
	writeln!(out, "static CLASSIC_LOREM_IPSUM_TEXT: &str = {:?};", CLASSIC_LOREM_IPSUM_TEXT).unwrap();
	writeln!(out).unwrap();

	// Vocab index for each classic Lorem Ipsum word, used at runtime to seed the Markov chain
	// after the classic prefix ends. Words are lowercased before lookup since the vocabulary
	// was lowercased during preprocessing, while the classic text retains original capitalization.
	// u16::MAX (65535) is used as a sentinel for words not found in the vocabulary, saving 2 bytes
	// per entry compared to Option<u16> (which is 4 bytes due to alignment)
	writeln!(out, "static CLASSIC_WORD_INDICES: &[u16] = &[").unwrap();
	for classic_word in CLASSIC_LOREM_IPSUM_TEXT.split_whitespace() {
		let lowered = classic_word.to_lowercase();
		match word_to_index.get(lowered.as_str()) {
			Some(&index) => writeln!(out, "    {index},").unwrap(),
			None => writeln!(out, "    {},", u16::MAX).unwrap(),
		}
	}
	writeln!(out, "];").unwrap();
}

/// Strips all single-quote characters from the corpus. The original Latin text uses them
/// exclusively as quotation marks for dialogue and citations (e.g. `'Si`, `malum.'`), never
/// as apostrophes for contractions (Latin doesn't use those). They would produce artifacts
/// in the generated output (e.g. `dolor.'`) since the Markov chain treats them as part of
/// the word token.
fn strip_quotation_marks(corpus: &str) -> String {
	corpus.replace('\'', "")
}

/// Lowercases the entire corpus so the vocabulary contains only lowercase word forms.
/// Capitalization is handled at runtime by `push_token`, which capitalizes the first letter
/// after sentence-enders and paragraph breaks. Training on all-lowercase input avoids
/// duplicate vocabulary entries for the same word in different positions (e.g. sentence-
/// initial "Addo" vs. mid-sentence "addo").
fn lowercase_all(corpus: &str) -> String {
	corpus.to_lowercase()
}

/// Looks up or inserts a word into the vocabulary, returning its index.
fn intern<'a>(word: &'a str, words: &mut Vec<&'a str>, word_to_index: &mut HashMap<&'a str, u16>) -> u16 {
	if let Some(&index) = word_to_index.get(word) {
		return index;
	}
	let index = words.len() as u16;
	words.push(word);
	word_to_index.insert(word, index);
	index
}
