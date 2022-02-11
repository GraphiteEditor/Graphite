//! # Layers
//! A document consists of a number of [Layers](layer_info::Layer).
//! Layers allow the user to mutate part of the document while leaving the rest unchanged.
//! Graphene currently includes three different types of layers:
//! * [Shapes](simple_shape::Shape), which contain generic SVG [`<path>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path)s,
//! * [Text](text::Text) layers, which contain character sequences,
//! * [Folders](folder::Folder), which encapsulate sub-layers
//!
//! Refer to the module-level documentation for detailed information on each layer.
//!
//! ## Overlapping layers
//! Layers are rendered on top of each other.
//! When different layers overlap, they are blended together according to the [BlendMode](blend_mode::BlendMode)
//! by a [`<feBlend>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/feBlend) filter.

/// Different ways of combining overlapping SVG Elements.
pub mod blend_mode;
/// Contains the [Folder](folder::Folder) type that encapsulates other layers, including more folders.
pub mod folder;
/// Contains the base [Layer](layer_info::Layer) type, an abstraction over the different types of layers.
pub mod layer_info;
/// Contains the [Shape](simple_shape::Shape) type, a generic SVG Element defined using Bezier paths.
pub mod simple_shape;
pub mod style;
/// Contains the [Text](text::Text) type.
pub mod text;
