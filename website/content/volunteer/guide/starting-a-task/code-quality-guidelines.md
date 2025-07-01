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

For consistency, please try to write comments (`//`) in *Sentence case* (with a capital first letter) and don't end with a period unless multiple sentences are used in the same comment. For doc comments (`///`), always end your sentences with a period. There should always be one space after the `//` or `///` comment markers, and `/* */` style comments should be avoided.

Avoid including commented-out code, unless you have a compelling reason to keep it around for future adaption, in your PRs that are open for code review.

Comments should usually be placed on a separate line above the code they are referring to, not at the end of the same code line.

## Blank lines

Please make a habit of grouping together related lines of code in blocks separated by blank lines. These are like your paragraphs if you were writing a novel — they greatly aid readability and your copy editor would have significant concerns with your writing if they were absent.

If you have dozens of lines comprising a single unbroken block of logic, you are likely not splitting it apart enough to aid readability. Find sensible places to partition the logic and insert blank lines between each. Roughly 10% of the code you write should ideally be blank lines, otherwise you are likely underutilizing them at the expense of readability.

## Imports

Our imports used to be a mess before we tamed the chaos with a formatting rule that has to be applied manually, since `rustfmt` doesn't support it.

We always combine imports with common paths, but only at the same depth. For example:

```rs
use crate::A::B::C;
use crate::A::B::C::Foo;
use crate::A::B::C::Bar;

// Should be combined into:

use crate::A::B::C::{self, Foo, Bar};
```

But we do not combine imports at mixed path depths. In other words, never put `::` inside `{}`. For example:

```rs
use crate::A::{B::C::Foo, X::Hello};

// Should be separated into:

use crate::A::B::C::Foo;
use crate::A::X::Hello;
```
