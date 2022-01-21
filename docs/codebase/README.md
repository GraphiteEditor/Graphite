# Graphite codebase docs

This is a great place to start learning about the Graphite codebase and its architecture and code structure.

## Core libraries

Graphite's core rust codebase is split into two reusable libraries:

- Editor
- Graphene

The Editor depends on Graphene, but Graphene can also be used alone. These are used internally but also intended for usage by third parties through Rust or linked by a project in C, C++, or another language.

## Code structure

The main modules of the project architecture are outlined below. Some parts describe future features and the directories don't exist yet. **Bold** modules are required for Graphite 0.1 which is purely an SVG editor.

- **Graphite Editor (Frontend)**: `/frontend/`  
  Prototype web-based GUI for Graphite that will eventually be replaced by a native GUI frontend implementation
	- **Vue web app**: `src/`  
	  Imports the WASM code and uses Vue props to customize and reuse most GUI components
	- **Rust WebAssembly translation layer**: `wasm/`  
	  Wraps the editor client backend and provides an API for the web app to use unburdened by Rust's complex data types that are not supported by JS
- **Graphite Editor (Backend)**: `/editor/`  
  Used by a frontend editor client to maintain GUI state and dispatch user events. The official Graphite editor is the primary user, but others software like game engines could embed their own customized editor implementations. Depends on Graphene.
- **Graphene (Document Graph Engine)**: `/graphene/`  
  A stateless library for updating Graphite design document (GDD) files. The official Graphite CLI and editor client backend are the primary users, but this library is intended to be useful to any application that wants to link the library for the purpose of updating GDD files by sending edit operations. This also serves as the 2D render engine and this is intended to be useful to any application that wants to link the Graphene library for the purpose of rendering Graphite graphs. For example, games can link the library and render procedural textures with customizable parametric input values.
