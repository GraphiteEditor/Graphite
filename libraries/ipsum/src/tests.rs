use super::*;
use std::collections::HashSet;

fn deterministic_rng(seed: u64) -> impl FnMut(usize) -> usize {
	let mut state = seed;
	move |n| {
		// Simple LCG for deterministic tests
		state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
		(state >> 33) as usize % n
	}
}

fn strip_punctuation(word: &str) -> String {
	word.to_lowercase().trim_end_matches(|c: char| c.is_ascii_punctuation()).to_owned()
}

/// Every vocabulary and classic word without its trailing punctuation, to check that output words are never cut short.
fn word_stems() -> HashSet<String> {
	let corpus = corpus();
	let vocabulary = (0..VOCABULARY_LEN as u16).map(|index| strip_punctuation(corpus.word(index)));
	vocabulary.chain(CLASSIC_TEXT.split_whitespace().map(strip_punctuation)).collect()
}

fn assert_whole_words(text: &str, stems: &HashSet<String>) {
	for token in text.split_whitespace() {
		assert!(stems.contains(&strip_punctuation(token)), "cut-off word {token:?} in: {text:?}");
	}
}

fn count_sentences(text: &str) -> usize {
	text.split_whitespace().filter(|word| word.ends_with(SENTENCE_ENDERS)).count()
}

// ======
// Corpus
// ======

#[test]
fn corpus_is_ascii_and_indexed() {
	let corpus = corpus();
	assert!(corpus.words.is_ascii() && CLASSIC_TEXT.is_ascii());
	assert!(!TOKENS.is_empty() && !corpus.sentence_starts.is_empty());
	assert_eq!(corpus.positions_by_bigram.len(), TOKENS.len() - 2);

	for index in 0..VOCABULARY_LEN as u16 {
		let word = corpus.word(index);
		let letters = word.trim_end_matches(|c: char| c.is_ascii_punctuation());
		assert!(
			letters.chars().all(|c| c.is_ascii_lowercase()) && !letters.is_empty(),
			"vocabulary word {word:?} should be lowercase letters"
		);
		assert!(word.len() - letters.len() <= 1, "vocabulary word {word:?} should have at most one trailing mark");
	}
}

#[test]
fn successors_match_the_corpus() {
	let corpus = corpus();
	let (a, b) = (TOKENS[0], TOKENS[1]);
	let expected: Vec<u16> = (0..TOKENS.len() - 2).filter(|&p| TOKENS[p] == a && TOKENS[p + 1] == b).map(|p| TOKENS[p + 2]).collect();
	let actual: Vec<u16> = corpus.occurrences_of(a, b).iter().map(|&p| successor(p)).collect();
	assert_eq!(actual, expected);
}

// =======
// Lengths
// =======

#[test]
fn words_count_matches_requested() {
	for count in [1, 2, 5, 10, 25, 50, 100, 1000] {
		let text = generate(0, Unit::Words, count, Unit::Words, deterministic_rng(42));
		let actual = text.split_whitespace().count();
		assert_eq!(actual, count, "requested {count} words, got {actual}: {text:?}");
		assert!(text.ends_with(SENTENCE_ENDERS), "should end with a sentence-ender: {text:?}");
	}
}

#[test]
fn sentences_count_matches_requested() {
	for count in [1, 3, 5, 20] {
		let text = generate(0, Unit::Words, count, Unit::Sentences, deterministic_rng(42));
		let actual = count_sentences(&text);
		assert_eq!(actual, count, "requested {count} sentences, got {actual}: {text:?}");
		assert!(text.ends_with(SENTENCE_ENDERS), "should end with a sentence-ender: {text:?}");
	}
}

#[test]
fn paragraphs_are_counted_and_complete() {
	for seed in 0..20 {
		for count in [1, 2, 3, 5] {
			let text = generate(0, Unit::Words, count, Unit::Paragraphs, deterministic_rng(seed));
			let paragraphs: Vec<&str> = text.split('\n').collect();
			assert_eq!(paragraphs.len(), count, "seed {seed}: requested {count} paragraphs: {text:?}");
			for paragraph in paragraphs {
				assert!(paragraph.ends_with(SENTENCE_ENDERS), "seed {seed}: paragraph should end with a sentence: {paragraph:?}");
				let words = paragraph.split_whitespace().count();
				let longest = PARAGRAPH_WORDS.end() + HARD_MAX_SENTENCE_WORDS + 25;
				assert!((*PARAGRAPH_WORDS.start()..=longest).contains(&words), "seed {seed}: paragraph of {words} words: {paragraph:?}");
			}
		}
	}
}

#[test]
fn settings_never_change_the_words() {
	let words = |text: &str| -> Vec<String> { text.split_whitespace().map(strip_punctuation).collect() };
	let shares_a_start = |a: &[String], b: &[String]| a.iter().zip(b).all(|(x, y)| x == y);

	for seed in 0..10 {
		let reference = words(&generate(0, Unit::Words, 400, Unit::Words, deterministic_rng(seed)));

		// The length unit only decides where the text stops and whether paragraph breaks are written
		for (length, unit) in [(2500, Unit::Characters), (20, Unit::Sentences), (5, Unit::Paragraphs)] {
			let other = words(&generate(0, Unit::Words, length, unit, deterministic_rng(seed)));
			assert!(shares_a_start(&reference, &other), "seed {seed}: {unit:?} changed the words");
		}

		// The opener is only put in front of the text, wherever it is cut
		let every_word_count = (1..=69).map(|opener_words| (opener_words, Unit::Words, opener_words));
		for (opener_length, opener_unit, opener_words) in every_word_count.chain([(1, Unit::Sentences, 19), (1, Unit::Paragraphs, 69)]) {
			let with_opener = generate(opener_length, opener_unit, 400, Unit::Words, deterministic_rng(seed));
			assert!(
				shares_a_start(&reference, &words(&with_opener)[opener_words..]),
				"seed {seed}: opener {opener_length} {opener_unit:?} changed the words"
			);
		}
	}
}

#[test]
fn characters_never_exceed_requested_and_use_whole_words() {
	let stems = word_stems();
	for count in 30..=400 {
		let text = generate(0, Unit::Words, count, Unit::Characters, deterministic_rng(count as u64));
		assert!(text.len() <= count, "requested {count} chars, got {}: {text:?}", text.len());
		assert!(text.len() + 24 >= count, "requested {count} chars but fell far short: {text:?}");
		assert!(text.ends_with(SENTENCE_ENDERS), "should end with a sentence-ender: {text:?}");
		assert_whole_words(&text, &stems);
	}
}

#[test]
fn longer_lengths_only_extend_the_text() {
	// Clause punctuation and the last paragraph break only appear once enough words follow them, so both are set aside
	let comparable = |text: &str| text.replace(CLAUSE_ENDERS, "").replace('\n', " ");

	for (unit, max_length) in [(Unit::Characters, 800), (Unit::Words, 300), (Unit::Sentences, 30), (Unit::Paragraphs, 8)] {
		for (seed, opener_length) in [(0, 0), (1, 8), (2, 8), (3, 69)] {
			let mut previous = String::new();
			for length in 1..=max_length {
				let text = generate(opener_length, Unit::Words, length, unit, deterministic_rng(seed));
				let unclosed = comparable(previous.trim_end_matches(|c: char| c.is_ascii_punctuation()));
				assert!(
					text.len() >= previous.len() && comparable(&text).starts_with(&unclosed),
					"{length} {unit:?} should extend {previous:?} but gave {text:?}"
				);

				let last_paragraph_words = text.rsplit('\n').next().unwrap_or_default().split_whitespace().count();
				assert!(
					!text.contains('\n') || last_paragraph_words >= MIN_WORDS_AFTER_PARAGRAPH_BREAK,
					"{length} {unit:?} ends on a stub paragraph: {text:?}"
				);

				let added_words = text.split_whitespace().count() - previous.split_whitespace().count();
				let one_word_at_a_time = matches!(unit, Unit::Characters | Unit::Words);
				assert!(
					!one_word_at_a_time || added_words <= 1,
					"{length} {unit:?} added {added_words} words at once: {previous:?} then {text:?}"
				);
				previous = text;
			}
		}
	}
}

#[test]
fn forced_endings_have_no_stranded_fragments() {
	assert_eq!(closed("Nihil novetur, de"), "Nihil novetur de.");
	assert_eq!(closed("Nihil novetur, de ipsis rebus"), "Nihil novetur, de ipsis rebus.");
	assert_eq!(closed("Odit sordidos, vanos, leves, futtiles,"), "Odit sordidos, vanos leves futtiles.");
	assert_eq!(closed("Explicabo brevi. Nullus, in"), "Explicabo brevi. Nullus in.");
	assert_eq!(closed("Explicabo brevi.\n"), "Explicabo brevi.");
}

// ======
// Opener
// ======

#[test]
fn opener_cut_mid_sentence_is_closed_as_its_own_sentence() {
	let text = generate(8, Unit::Words, 50, Unit::Words, deterministic_rng(42));
	let after_opener = text
		.strip_prefix("Lorem ipsum dolor sit amet, consectetur adipiscing elit. ")
		.unwrap_or_else(|| panic!("opener mismatch: {text:?}"));
	assert!(after_opener.starts_with(|c: char| c.is_uppercase()), "the continuation should open a new sentence: {text:?}");

	// A clause mark too close to the cut is dropped, as it is at the end of the whole text
	let text = generate(6, Unit::Words, 50, Unit::Words, deterministic_rng(42));
	assert!(text.starts_with("Lorem ipsum dolor sit amet consectetur. "), "opener mismatch: {text:?}");
}

#[test]
fn opener_sentences_prefix_matches_classic() {
	let text = generate(2, Unit::Sentences, 60, Unit::Words, deterministic_rng(42));
	let second_sentence_end = CLASSIC_TEXT.find("consequat.").unwrap() + "consequat.".len();
	assert!(text.starts_with(&CLASSIC_TEXT[..second_sentence_end]), "opener mismatch: {text:?}");
}

#[test]
fn opener_paragraph_is_the_whole_classic_text() {
	let text = generate(1, Unit::Paragraphs, 3, Unit::Paragraphs, deterministic_rng(42));
	let paragraphs: Vec<&str> = text.split('\n').collect();
	assert_eq!(paragraphs.len(), 3, "expected 3 paragraphs: {text:?}");
	assert_eq!(paragraphs[0], CLASSIC_TEXT);

	let text = generate(1, Unit::Paragraphs, 1, Unit::Paragraphs, deterministic_rng(42));
	assert_eq!(text, CLASSIC_TEXT);

	let text = generate(1, Unit::Paragraphs, 100, Unit::Words, deterministic_rng(42));
	assert!(text.starts_with(&format!("{CLASSIC_TEXT}\n")), "classic paragraph should be followed by a break: {text:?}");
	assert_eq!(text.split_whitespace().count(), 100);
}

#[test]
fn opener_characters_fits_whole_words_once_closed() {
	let assert_opener = |opener_length: usize, expected: &str| {
		let text = generate(opener_length, Unit::Characters, 100, Unit::Words, deterministic_rng(42));
		assert!(text.starts_with(expected), "opener of {opener_length} chars should give {expected:?}: {text:?}");
	};

	assert_opener(6, "Lorem. ");
	assert_opener(26, "Lorem ipsum dolor sit. ");
	assert_opener(27, "Lorem ipsum dolor sit amet. ");
	assert_opener(1000, CLASSIC_TEXT);

	// Too short for even the first word and its period
	let text = generate(5, Unit::Characters, 100, Unit::Words, deterministic_rng(42));
	assert!(!text.starts_with("Lorem"), "{text:?}");
}

#[test]
fn opener_never_overshoots_total() {
	let text = generate(100, Unit::Words, 10, Unit::Words, deterministic_rng(42));
	assert_eq!(text.split_whitespace().count(), 10, "{text:?}");

	let text = generate(1, Unit::Paragraphs, 2, Unit::Sentences, deterministic_rng(42));
	assert_eq!(count_sentences(&text), 2, "{text:?}");

	let text = generate(1, Unit::Paragraphs, 40, Unit::Characters, deterministic_rng(42));
	assert!(text.len() <= 40 && text.ends_with(SENTENCE_ENDERS), "{text:?}");
}

#[test]
fn opener_counts_toward_the_first_paragraph_without_being_split() {
	let mut breaks_right_after_opener = 0;
	for seed in 0..40 {
		// The whole passage is long enough to fill a paragraph, which it does only when a short one was drawn
		let text = generate(69, Unit::Words, 300, Unit::Words, deterministic_rng(seed));
		assert!(text.starts_with(CLASSIC_TEXT), "seed {seed}: the opener should never be split: {text:?}");
		breaks_right_after_opener += usize::from(text[CLASSIC_TEXT.len()..].starts_with('\n'));

		let first_paragraph_words = text.split('\n').next().unwrap().split_whitespace().count();
		let longest = PARAGRAPH_WORDS.end() + HARD_MAX_SENTENCE_WORDS + 25;
		assert!(first_paragraph_words <= longest, "seed {seed}: first paragraph of {first_paragraph_words} words: {text:?}");

		// Shorter openers neither fill a paragraph nor escape its budget
		let text = generate(1, Unit::Sentences, 300, Unit::Words, deterministic_rng(seed));
		let first_paragraph_words = text.split('\n').next().unwrap().split_whitespace().count();
		assert!(
			(*PARAGRAPH_WORDS.start()..=longest).contains(&first_paragraph_words),
			"seed {seed}: first paragraph of {first_paragraph_words} words"
		);
	}
	assert!((5..35).contains(&breaks_right_after_opener), "{breaks_right_after_opener} of 40 seeds broke right after the opener");
}

// ==========
// Edge cases
// ==========

#[test]
fn zero_total_returns_empty() {
	let text = generate(8, Unit::Words, 0, Unit::Words, deterministic_rng(42));
	assert!(text.is_empty(), "zero total should return empty: {text:?}");
}

#[test]
fn zero_opener_no_classic_text() {
	let text = generate(0, Unit::Words, 20, Unit::Words, deterministic_rng(42));
	assert!(!text.starts_with("Lorem"), "zero opener should not start with Lorem: {text:?}");
}

#[test]
fn no_whitespace_or_punctuation_artifacts() {
	let stems = word_stems();
	let totals = [(300, Unit::Words), (15, Unit::Sentences), (4, Unit::Paragraphs), (137, Unit::Characters), (1500, Unit::Characters)];
	for seed in 0..50 {
		for (total_length, total_unit) in totals {
			for opener_length in [0, 8] {
				let text = generate(opener_length, Unit::Words, total_length, total_unit, deterministic_rng(seed));
				for artifact in ["  ", " \n", "\n ", " .", " ,", ". .", ",.", ";.", ":.", "..", "?.", "!."] {
					assert!(!text.contains(artifact), "seed {seed}, {total_length} {total_unit:?}: {artifact:?} artifact in: {text:?}");
				}
				assert!(text.ends_with(SENTENCE_ENDERS), "seed {seed}, {total_length} {total_unit:?}: no sentence end in: {text:?}");
				assert_whole_words(&text, &stems);
			}
		}
	}
}

#[test]
fn capitalization_follows_sentence_and_paragraph_starts() {
	for seed in 0..20 {
		let text = generate(0, Unit::Words, 4, Unit::Paragraphs, deterministic_rng(seed));
		for paragraph in text.split('\n') {
			let mut expect_capital = true;
			for word in paragraph.split_whitespace() {
				let first = word.chars().next().unwrap();
				assert_eq!(first.is_uppercase(), expect_capital, "seed {seed}: capitalization of {word:?} in: {paragraph:?}");
				expect_capital = word.ends_with(SENTENCE_ENDERS);
			}
		}
	}
}

#[test]
fn overlong_sentences_are_reined_in() {
	let mut far_over_hard_cap = 0;
	let mut sentences = 0;
	for seed in 0..20 {
		let text = generate(0, Unit::Words, 2000, Unit::Words, deterministic_rng(seed));
		let mut length = 0;
		for word in text.split_whitespace() {
			length += 1;
			if word.ends_with(SENTENCE_ENDERS) {
				sentences += 1;
				far_over_hard_cap += usize::from(length > HARD_MAX_SENTENCE_WORDS + 25);
				length = 0;
			}
		}
	}
	assert!(far_over_hard_cap * 100 < sentences, "{far_over_hard_cap} of {sentences} sentences ran far past the hard cap");
}

// =========
// Stability
// =========

#[test]
fn deterministic_output() {
	let text1 = generate(8, Unit::Words, 50, Unit::Words, deterministic_rng(42));
	let text2 = generate(8, Unit::Words, 50, Unit::Words, deterministic_rng(42));
	assert_eq!(text1, text2, "same seed should produce identical output");
}

#[test]
fn different_seeds_differ() {
	let text1 = generate(0, Unit::Words, 50, Unit::Words, deterministic_rng(1));
	let text2 = generate(0, Unit::Words, 50, Unit::Words, deterministic_rng(2));
	assert_ne!(text1, text2, "different seeds should produce different output");
}

// ===================
// Typographic profile
// ===================

/// Word length, punctuation density, and sentence and paragraph lengths in the bands of English body text,
/// so the placeholder text lays out like the copy it stands in for.
#[test]
fn typographic_profile_resembles_body_text() {
	let mut letters = 0;
	let mut words = 0;
	let mut short_words = 0;
	let mut commas = 0;
	let mut sentences = 0;
	let mut questions = 0;
	let mut paragraphs = 0;
	for seed in 0..20 {
		let text = generate(0, Unit::Words, 2000, Unit::Words, deterministic_rng(seed));
		paragraphs += text.split('\n').count();
		for token in text.split_whitespace() {
			let word_length = token.trim_end_matches(|c: char| c.is_ascii_punctuation()).len();
			letters += word_length;
			words += 1;
			short_words += usize::from(word_length <= 3);
			commas += usize::from(token.ends_with(','));
			sentences += usize::from(token.ends_with(SENTENCE_ENDERS));
			questions += usize::from(token.ends_with('?'));
		}
	}

	let letters_per_word = letters as f64 / words as f64;
	assert!((4.5..=6.5).contains(&letters_per_word), "letters per word: {letters_per_word:.2}");
	assert!(short_words * 100 / words >= 20, "short words: {}%", short_words * 100 / words);
	let commas_per_100_words = commas as f64 * 100. / words as f64;
	assert!((3. ..=10.).contains(&commas_per_100_words), "commas per 100 words: {commas_per_100_words:.1}");
	assert!(questions * 100 / sentences <= 6, "questions: {}% of sentences", questions * 100 / sentences);
	let words_per_sentence = words as f64 / sentences as f64;
	assert!((12. ..=25.).contains(&words_per_sentence), "words per sentence: {words_per_sentence:.1}");
	let words_per_paragraph = words as f64 / paragraphs as f64;
	assert!((40. ..=120.).contains(&words_per_paragraph), "words per paragraph: {words_per_paragraph:.1}");
}

/// Greedy line breaking at a body-text measure should fill lines nearly as well as English does,
/// which depends on the supply of short words.
#[test]
fn lines_fill_a_body_measure() {
	const MEASURE: usize = 60;
	let mut filled = 0;
	let mut lines = 0;
	for seed in 0..10 {
		let text = generate(0, Unit::Words, 1000, Unit::Words, deterministic_rng(seed));
		for paragraph in text.split('\n') {
			let mut line_length = 0;
			for word in paragraph.split_whitespace() {
				let cost = word.len() + usize::from(line_length > 0);
				if line_length + cost > MEASURE {
					filled += line_length;
					lines += 1;
					line_length = word.len();
				} else {
					line_length += cost;
				}
			}
		}
	}

	let fill = filled as f64 / (lines * MEASURE) as f64;
	assert!(fill >= 0.9, "lines fill {:.1}% of a {MEASURE}-character measure", fill * 100.);
}

#[test]
fn phrases_do_not_loop() {
	let mut repeats = 0;
	let mut words_total = 0;
	for seed in 0..20 {
		let text = generate(0, Unit::Words, 2000, Unit::Words, deterministic_rng(seed));
		let words: Vec<&str> = text.split_whitespace().collect();
		words_total += words.len();
		for (i, window) in words.windows(3).enumerate() {
			let recent = &words[i.saturating_sub(REPETITION_WINDOW)..i];
			repeats += usize::from(recent.windows(3).any(|earlier| earlier == window));
		}
	}
	assert!(repeats * 2000 < words_total, "{repeats} repeated phrases in {words_total} words");
}

// ==============
// Golden outputs
// ==============

/// The generated text is part of the compatibility contract: any change here changes the text produced for every seed
/// already in use, so update these only for a deliberate change to the generator or corpus.
#[test]
fn golden_outputs_are_stable() {
	assert_eq!(
		generate(8, Unit::Words, 20, Unit::Words, deterministic_rng(42)),
		"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Cur igitur non bene? Quia quod bene, id recte, frugaliter honeste ille."
	);
	assert_eq!(
		generate(0, Unit::Words, 2, Unit::Sentences, deterministic_rng(7)),
		"Quare hoc videndum est, possitne nobis hoc ratio philosophorum dare. Pollicetur certe."
	);
	assert_eq!(
		generate(0, Unit::Words, 80, Unit::Characters, deterministic_rng(3)),
		"Incendi igitur eos qui ratione voluptatem sequi nesciunt neque porro ex eo."
	);
	assert_eq!(
		generate(1, Unit::Paragraphs, 80, Unit::Words, deterministic_rng(1)),
		format!("{CLASSIC_TEXT}\nQuem tiberina descensio festo illo die tanto gaudio affecit quanto paulum.")
	);
}
