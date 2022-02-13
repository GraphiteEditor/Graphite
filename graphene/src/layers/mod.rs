//! # Layers
//! A document consists of a number of [Layers](layer_info::Layer).
//! Layers allow the user to mutate part of the document while leaving the rest unchanged.
//! Graphene currently includes three different types of layers:
//! * [Shape Layers](shape_layer::ShapeLayer), which contain generic SVG [`<path>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path)s,
//! * [Text Layers](text_layer::TextLayer) layers, which contain character sequences,
//! * [Folder Layers](folder_layer::FolderLayer), which encapsulate sub-layers
//!
//! Refer to the module-level documentation for detailed information on each layer.
//!
//! ## Overlapping layers
//! Layers are rendered on top of each other.
//! When different layers overlap, they are blended together according to the [BlendMode](blend_mode::BlendMode)
//! using the css [`mix-blend-mode`](https://developer.mozilla.org/en-US/docs/Web/CSS/mix-blend-mode) property.

/// Different ways of combining overlapping SVG Elements.
pub mod blend_mode;
/// Contains the [FolderLayer](folder_layer::FolderLayer) type that encapsulates other layers, including more folders.
pub mod folder_layer;
/// Contains the base [Layer](layer_info::Layer) type, an abstraction over the different types of layers.
pub mod layer_info;
/// Contains the [ShapeLayer](shape_layer::ShapeLayer) type, a generic SVG Element defined using Bezier paths.
pub mod shape_layer;
pub mod style;
/// Contains the [TextLayer](text_layer::TextLayer) type.
pub mod text_layer;
