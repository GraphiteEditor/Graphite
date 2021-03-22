# Graphite codebase docs

This is a great place to start learning about the Graphite codebase and its architecture and code structure.

## Core libraries

Graphite's core rust codebase is split into three reusable libraries:

- Graphite Editor Core Library
- Graphite Document Core Library
- Graphite Renderer Core Library

Each depends on its successor in the list. These are used internally but also intended for usage by third parties through Rust or linked by a project in C, C++, or another language.

## Code structure

The main modules of the project architecture are outlined below. Some parts describe future features and the directories don't exist yet. **Bold** modules are required for Graphite 0.1 which is purely an SVG editor.

- **Web frontend**: `/client/web/`  
  Initial GUI for Graphite that will eventually be replaced by a native GUI implementation
	- **Vue web app**: `src/`  
	  Imports the WASM code and uses Vue props to customize and reuse most GUI components
	- **Rust WebAssembly wrapper**: `wasm/`  
	  Wraps the Editor Core Library and provides an API for the web app to use unburdened by Rust's complex data types that are not supported by WASM
- Native frontend: `/client/native/`  
  The future official desktop client. Blocked on Rust's GUI ecosystem improving or dedicating the time to build a custom system that can nicely support editor extensions. The whole GUI should use WGPU for rendering and compile to WASM to make those calls to the WebGPU API.
- CLI: `/client/cli/`  
  A headless, command line GDD document editor (using the Document Core Library) and GRD render graph renderer (using the Renderer Core Library). Not the initial focus of development, but perhaps useful in testing for certain features throughout the development process. A future version of the CLI will probably redesign the command structure.
- **Graphite Editor Core Library**: `/core/editor/`  
  Used by a frontend editor client to maintain GUI state and dispatch user events. The official Graphite editor is the primary user, but others software like game engines could embed their own customized editor implementations. Depends on the Document Core Library.
- Graphite Document Core Library: `/core/document/`  
  A stateless library for updating Graphite design document (GDD) files. The official Graphite CLI and Editor Core Library are the primary users, but this library is intended to be useful to any application that wants to link the library for the purpose of updating GDD files by sending edit operations. Optionally depends on the Renderer Core Library if rendering is required.
- Graphite Renderer Core Library: `/core/renderer/`  
  A stateless library (with the help of in-memory and/or on-disk caches for performance) for rendering Graphite's render graph (GRD) files. The official Graphite CLI and Document Core Library are the primary users, but this library is intended to be useful to any application that wants to link the library for the purpose of rendering Graphite's render graphs. For example, games can link the library and render procedural textures with customizable parametric input values.

## Architecture diagram

Take this [Architecture Overview](architecture-overview.nomnoml) diagram and paste it into the [Nomnoml](https://nomnoml.com/) diagram editor. We'll set up a better way to render Nomnoml diagrams when we have a proper home for this documentation.