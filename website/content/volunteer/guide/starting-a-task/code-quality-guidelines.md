+++
title = "Code quality guidelines"

[extra]
order = 2 # Page number after chapter intro
+++

The Graphite project prizes code quality and accessibility to new contributors. Therefore, we ask you please make all efforts to contribute readable, well-documented code according to these best practices.

## Linting

Please ensure Clippy is enabled. This should be set up automatically in VS Code. Try to avoid committing code with lint warnings. You may execute `cargo clippy` anytime to confirm.

## Naming

Please use descriptive variable/function/symbol names and keep abbreviations to a minimum. Prefer spelling out full words most of the time, so `gen_doc_fmt` should be written out as `generate_document_format` instead.

This avoids the mental burden of expanding abbreviations into semantic meaning. Monitors are wide enough to display long variable/function names, so descriptive is better than cryptic.

Totally unambiguous, common shortened forms are acceptable such as "max" for "maximum", "eval" for "evaluate", and "info" for "information".

To avoid wasted effort in code review, it's recommended that you set up a spellcheck plugin, like [this extension](https://marketplace.visualstudio.com/items?itemName=streetsidesoftware.code-spell-checker) for VS Code. The project uses American English spelling conventions.

## Whole-number floats

Always use the style `42.` instead of `42.0` for whole-number floats to maintain consistency and brevity. For range syntax, either `0.0..42.` or `(0.)..42.` is acceptable.

## Comments

For consistency, please try to write comments in *Sentence case* (starting with a capital letter). End with a period only if multiple sentences are used in the same comment. For doc comments (`///`), always write in full sentences ending with a period. There should always be one space after the `//` or `///` comment markers, and `/* */` style comments shouldn't be used.

Avoid including commented-out code, unless you have a compelling reason to keep it around for future adaption, in your PRs that are open for code review.

Comments should usually be placed on a separate line above the code they are referring to, not at the end of the code line.

## Blank lines

Please make a habit of grouping together related lines of codes in blocks separated by blank lines. If you have dozens of lines comprising a single unbroken block of logic, you are likely not splitting it apart enough to aid readability. Find sensible places to partition the logic and insert blank lines between each. Roughly 10% of the code you write should ideally be blank lines, otherwise you are likely underutilizing them at the expense of readability.

## Imports

Our imports used to be a mess before we tamed the chaos with these rules.

At the top of Rust files, use the convention of separating imports into three blocks, ordered as:
1. Local (`use super::` and `use crate::`)
2. First-party crates (e.g. `use editor::` or `bezier_rs::`)
3. Third-party libraries (e.g. `use std::` or `use glam::`)

Combine related imports with common paths at the same depth. For example:

```rs
use crate::A::B::C;
use crate::A::B::C::Foo;
use crate::A::B::C::Bar;

// Should be combined into:

use crate::A::B::C::{self, Foo, Bar};
```

But do not combine imports at mixed path depths. For example, `use crate::A::{B::C::Foo, X::Hello};` should be split into two separate import lines— avoid having `::` inside `{}`.
