//! # Layers
//! A document consists of a set of [Layers](layer_info::Layer).
//! Layers allow the user to mutate part of the document while leaving the rest unchanged.
//! Graphene currently includes these different types of layers:
//! * [Folder layers](folder_layer::FolderLayer), which encapsulate sub-layers
//! * [Shape layers](shape_layer::ShapeLayer), which contain generic SVG [`<path>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path)s
//! * [Text layers](text_layer::TextLayer), which contain a description of laid out text
//! * [Image layers](image_layer::ImageLayer), which contain a bitmap image
//!
//! Refer to the module-level documentation for detailed information on each layer.
//!
//! ## Overlapping layers
//! Layers are rendered on top of each other.
//! When different layers overlap, they are blended together according to the [BlendMode](blend_mode::BlendMode)
//! using the CSS [`mix-blend-mode`](https://developer.mozilla.org/en-US/docs/Web/CSS/mix-blend-mode) property and the layer opacity.

/// Different ways of combining overlapping SVG elements.
pub mod blend_mode;
/// Contains the [FolderLayer](folder_layer::FolderLayer) type that encapsulates other layers, including more folders.
pub mod folder_layer;
/// Contains the [ImageLayer](image_layer::ImageLayer) type that contains a bitmap image.
pub mod image_layer;
/// Contains the base [Layer](layer_info::Layer) type, an abstraction over the different types of layers.
pub mod layer_info;
/// Contains the [ShapeLayer](shape_layer::ShapeLayer) type, a generic SVG element defined using Bezier paths.
pub mod shape_layer;
pub mod style;
/// Contains the [TextLayer](text_layer::TextLayer) type.
pub mod text_layer;
