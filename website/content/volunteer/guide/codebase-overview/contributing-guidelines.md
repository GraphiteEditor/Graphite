+++
title = "Contributing guidelines"

[extra]
order = 3 # Page number after chapter intro
+++

## Code style

The Graphite project prizes code quality and accessibility to new contributors. Therefore, we ask you please make all efforts to contribute readable, well-documented code according to these best practices.

### Naming

Please use descriptive variable/function/symbol names and keep abbreviations to a minimum. Prefer to spell out full words most of the time, so `gen_doc_fmt` should be written out as `generate_document_format` instead.

This avoids the mental burden of expanding abbreviations into semantic meaning. Monitors are wide enough to display long variable/function names, so descriptive is better than cryptic. To streamline code review, it's recommended that you set up a spellcheck plugin in your editor. The project uses American English spelling conventions.

### Linting

Please ensure Clippy is enabled. This should be set up automatically in VS Code. Try to avoid committing code with lint warnings.

### Comments

For consistency, please try to write comments in *Sentence case* (starting with a capital letter). End with a period only if multiple sentences are used in the same comment. For doc comments (`///`), always write in full sentences (ending with a period).

Comments should be placed on a separate line, but exceptions are permitted where sensible. They should target the maximum line length of 200 characters (don't go over, and don't target a considerably lower number like 80 for line breaks).

### Imports

At the top of Rust files, please follow the convention of separating imports into three blocks, in this order:
1. Local (`use super::` and `use crate::`)
2. First-party crates (e.g. `use editor::`)
3. Third-party libraries (e.g. `use std::` or `use serde::`)

Combine related imports with common paths at the same depth. For example, the lines `use crate::A::B::C;`, `use crate::A::B::C::Foo;`, and `use crate::A::B::C::Bar;` should be combined into `use crate::A::B::C::{self, Foo, Bar};`. But do not combine imports at mixed path depths. For example, `use crate::A::{B::C::Foo, X::Hello};` should be split into two separate import lines. In simpler terms, avoid putting a `::` inside `{}`.

## Tests

It's great if you can write tests for your code, especially if it's a tricky stand-alone function. However at the moment, we are prioritizing rapid iteration and will usually accept code without associated unit tests. That stance will change in the near future as we begin focusing more on stability than iteration speed.

## Draft pull requests

Once you begin writing code, please open a pull request immediately and mark it as a **Draft**. Please push to this on a frequent basis, even if things don't compile or work fully yet. It's very helpful to have your work-in-progress code up on GitHub so the status of your feature is less of a mystery.

Open a new PR as a draft / convert an existing PR to a draft:

<img src="https://static.graphite.rs/content/volunteer/guide/draft-pr.avif" onerror="this.onerror = null; this.src = this.src.replace('.avif', '.png')" alt="Screenhots showing GitHub's &quot;Create pull request (arrow) > Create draft pull request&quot; and &quot;Still in progress? Convert to draft&quot; buttons" />
