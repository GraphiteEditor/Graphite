+++
title = "Contribute to Graphite"
template = "page.html"
+++

It's great to hear you are interested in contributing to Graphite! We want to make it as easy and frictionless as possible for you to get started. Here are the basics.

## Building and running the codebase.

Graphite is built with Rust and web technologies. Download and install the latest LTS version of [Node.js](https://nodejs.org/) and stable release of [Rust](https://www.rust-lang.org/), as well as [Git](https://git-scm.com/).

On Windows, Rust requires the MSVC toolchain properly configured with the Visual Studio Build Tools installed on your machine including the "Desktop development with C++" workload. **If you are having issues building,** you might have skipped this step or you may have outdated Node.js or Rust versions (compare `node --version` and `rustc --version` against the versions listed in the links above, use `rustup update` to upgrade Rust).

Clone the project:
```
git clone https://github.com/GraphiteEditor/Graphite.git
```

Then install the required Node.js packages:
```
cd frontend
npm install
```

You only need to explicitly install Node.js dependencies. Rust's cargo dependencies will be installed automatically on your first build. One dependency in the build chain, `wasm-pack`, will be installed automatically on your system when the Node.js packages are installing. If you prefer to install this manually, you can get it from the [wasm-pack website](https://rustwasm.github.io/wasm-pack/), then install dependencies with `npm install --no-optional`. **This is necessary on Apple Silicon/M1 Macs** until [this bug](https://github.com/rustwasm/wasm-pack/issues/952) is fixed.

To run the project while developing:
```
npm start
```

This spins up the dev server at <http://localhost:8080> with a file watcher that performs hot reloading of the web page. You should be able to start the server, edit and save web and Rust code, and rarely have to kill the server (by hitting <kbd>Ctrl</kbd><kbd>C</kbd> twice). You sometimes may need to reload the web page if the hot reloading didn't behave perfectly.

While developing Rust code, `cargo check` and `cargo clippy` may be run from the root directory. You can also use `npm run lint`/`npm run lint-no-fix` to solve web code formatting and `cargo fmt` for Rust code formatting. If you don't use VS Code and its format-on-save feature, please remember or format before committing or consider [setting up a `pre-commit` hook](https://githooks.com/) to do that automatically.

We provide default configurations for VS Code users. When you open the project, watch for a prompt to install the project's suggested extensions. They will provide helpful web and Rust tooling. If you use a different IDE, you won't get default configurations for the project out of the box, so please remember to format your code and check CI for errors.

## Task board.

Visit our [**task board**](https://github.com/GraphiteEditor/Graphite/projects/1) board and look through the current sprint's column. You are also welcome to work on tasks prioritized for upcoming sprints. Find any issues with the green "Available" tag.

Pay attention to the tags which provide some useful information like which ones are a [**Good First Issue**](https://github.com/GraphiteEditor/Graphite/issues?q=is%3Aissue+is%3Aopen+label%3AAvailable+label%3A%22Good+First+Issue%22+) and whether they involve [**only Rust**](https://github.com/GraphiteEditor/Graphite/issues?q=is%3Aissue+is%3Aopen+label%3ARust+label%3AAvailable+-label%3AWeb+), [**only Web**](https://github.com/GraphiteEditor/Graphite/issues?q=is%3Aissue+is%3Aopen+label%3AWeb+label%3AAvailable+-label%3ARust+) (HTML/CSS/TypeScript/Vue.js), or [**both**](https://github.com/GraphiteEditor/Graphite/issues?q=is%3Aissue+is%3Aopen+label%3AAvailable+label%3ARust+label%3AWeb+). Feel free to pick whatever task interests you, then comment on the issue that you would like to start. After commenting, you can dig in right away, then we will assign the issue to your GitHub user to keep the status of things organized.

## Mentorship.

Join the [project's Discord server](https://discord.graphite.rs) then hop on the `#development` channel and ping @Keavon and @TrueDoctor. We would be delighted to help you get started with in-depth explanations of the code, one-on-one mentorship and pair programming. This is very valuable and not at all an inconvenience to us because it helps you avoid the intimidating step of getting started, so please do not hesitate to reach out right away.

## Docs.

We have some documentation [here](https://github.com/GraphiteEditor/Graphite/blob/master/docs/README.md). However it could use some improvements and currently isn't a substitute for mentorship described in the section above. If you also want to dig into the code and solidify your understanding by writing documentation, that would be equally valuable to the project!

## Codebase overview.

The Graphite Editor is built as a web app powered by Vue.js in the frontend and Rust in the backend which is compiled to WebAssembly (wasm) and run in the browser.

The Editor's frontend web code lives in `/frontend/src` and the backend Rust code lives in `/editor`. The web-based frontend is intended to be semi-temporary and eventually replaceable with a pure-Rust GUI frontend. Therefore, all backend code should be unaware of JavaScript or web concepts and all Editor application logic should be written in Rust not JS.

### Frontend/backend communication

Frontend (JS) -> backend (Rust/wasm) communication is achieved through a thin Rust translation layer in `/frontend/wasm/api.rs` which wraps the Editor backend's complex Rust data type API and provides the JS with a simpler API of callable functions. These wrapper functions are compiled by wasm-bindgen into autogenerated JS functions that serve as an entry point into the wasm.

Backend (Rust) -> frontend (JS) communication happens by sending a queue of messages to the frontend message dispatcher. After the JS calls any wrapper API function to get into backend (Rust) code execution, the Editor's business logic runs and queues up `FrontendMessage`s (defined in `editor/src/frontend/frontend_message_handler.rs`) which get mapped from Rust to JS-friendly data types in `frontend/src/dispatcher/js-messages.ts`. Various JS code subscribes to these messages by calling `subscribeJsMessage(MessageName, (messageData) => { /* callback code */ });`.

### Editor backend and Graphene modules

The Graphite Editor backend handles all the day-to-day logic and responsibilities of a user-facing interactive application. Some duties include: user input, GUI state management, viewport tool behavior, layer management and selection, and handling of multiple document tabs.

The actual document (the artwork data and layers included in a saved `.graphite` file) is part of another core module located in `/graphene`. Graphene manages a document and will grow into the codebase for the full node graph system in the future. While it's OK for the Editor to read data from, or make immutable function calls upon, the Graphene document, it should never be directly mutated. Instead, messages (called Operations) should be sent to the Graphene document to request changes occur. Graphene is designed to be used by the Editor or by third-party Rust or C/C++ code directly so a careful separation of concerns between the Editor and Graphene should be considered.

### The message bus

Every part of the Graphite stack works based on the concept of message passing. Messages are pushed to the front or back of a queue and each one is processed by the module's dispatcher in the order encountered. Only the dispatcher owns a mutable reference to update its module's state.

<details><summary><b>Addition technical details (click to show)</b></summary>

A message is an enum variant of a certain message sub-type like `FrontendMessage`, `ToolMessage`, `DocumentsMessage` (plural), or `DocumentMessage` (singular). An example is `DocumentMessage::DeleteSelectedLayers` (which carries no data) or `DocumentMessage::RenameLayer(Vec<LayerId>, String)` (which carries a layer path and a string as data).

Message sub-types hierarchically wrap other message sub-types; for example, `DocumentMessage` is wrapped by `DocumentsMessage` via `DocumentsMessage::Document(DocumentMessage)` (this carries the child message as data), and `EllipseMessage` is wrapped by `ToolMessage` via `ToolMessage::Ellipse(EllipseMessage)` (again, this carries the child message as data). Every message sub-type is wrapped by the top-level `Message`, so the previous example is actually `Message::Tool(ToolMessage::Ellipse(EllipseMessage))`.

Because this is cumbersome, we have a proc macro `#[child]` that automatically implements the `From` trait on message sub-types and lets you write `DocumentMessage::DeleteSelectedLayers.into()` instead of `Message(DocumentsMessage::Document(DocumentMessage::DeleteSelectedLayers))`.

</details>

## Contributing guide.

### Code style

The Graphite project highly values code quality and accessibility to new contributors. Therefore, please make an effort to make your code readable and well-documented.

**Naming:** Please use descriptive variable/function/symbol names and keep abbreviations to a minimum. Prefer to spell out full words most of the time, so `gen_doc_fmt` should be written out as `generate_document_format` instead. This avoids the mental burden of expanding abbreviations into semantic meaning. Monitors are wide enough to display long variable/function names, so descriptive is better than cryptic.

**Imports:** At the top of Rust files, please follow the convention of separating imports into three blocks, in this order:
1. Local (`use super::` and `use crate::`)
2. First-party crates (e.g. `use graphene::`)
3. Third-party libraries (e.g. `use std::` or `use serde::`)

Combine related imports with common paths at the same depth. For example, the lines `use crate::A::B::C;`, `use crate::A::B::C::Foo;`, and `use crate::A::B::C::Bar;` should be combined into `use crate::A::B::C::{self, Foo, Bar};`. But do not combine imports at mixed path depths. For example, `use crate::A::{B::C::Foo, X::Hello};` should be split into two separate import lines. In simpler terms, avoid putting a `::` inside `{}`.

**Tests:** It's great if you can write tests for your code, especially if it's a tricky stand-alone function. However at the moment, we are prioritizing rapid iteration and will usually accept code without associated unit tests. That stance will change in the near future as we begin focusing more on stability than iteration speed.

Additional best practices will be added here soon. Please ask @Keavon in the mean time.

### Draft pull requests

Once you begin writing code, please open a pull request immediately and mark it as a **Draft**. Please push to this on a frequent basis, even if things don't compile or work fully yet. It's very helpful to have your work-in-progress code up on GitHub so the status of your feature is less of a mystery.

Open a new PR as a draft / Convert an existing PR to a draft:

![](https://static.graphite.rs/content/contribute/draft-pr.png)
