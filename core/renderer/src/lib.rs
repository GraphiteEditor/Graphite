//! Graphite Renderer Core Library: `/core/renderer/`  
//! A stateless library (with the help of in-memory and/or on-disk caches for performance) for rendering Graphite's render graph (GRD) files.
//! The official Graphite CLI and Document Core Library are the primary users,
//! but this library is intended to be useful to any application that wants to link the library for the purpose of rendering Graphite's render graphs.
//! For example, games can link the library and render procedural textures with customizable parametric input values.

#[cfg(test)]
mod tests {
	#[test]
	fn it_works() {
		assert_eq!(2 + 2, 4);
	}
}
