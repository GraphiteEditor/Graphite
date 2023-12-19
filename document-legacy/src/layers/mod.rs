//! # Layers
//! A document consists of a set of [Layers](layer_info::Layer).
//! Layers allow the user to mutate part of the document while leaving the rest unchanged.
//! There are currently these different types of layers:
//! * [Folder layers](folder_layer::FolderLegacyLayer), which encapsulate sub-layers
//! * [Layer layers](layer_layer::LayerLegacyLayer), which contain a node graph layer
//!
//! Refer to the module-level documentation for detailed information on each layer.
//!
//! ## Overlapping layers
//! Layers are rendered on top of each other.
//! When different layers overlap, they are blended together according to the [BlendMode](blend_mode::BlendMode)
//! using the CSS [`mix-blend-mode`](https://developer.mozilla.org/en-US/docs/Web/CSS/mix-blend-mode) property and the layer opacity.

pub mod base64_serde;
/// Contains the [FolderLegacyLayer](folder_layer::FolderLegacyLayer) type that encapsulates other layers, including more folders.
pub mod folder_layer;
/// Contains the base [Layer](layer_info::Layer) type, an abstraction over the different types of layers.
pub mod layer_info;
/// Contains the [LayerLegacyLayer](nodegraph_layer::LayerLegacyLayer) type that contains a node graph.
pub mod layer_layer;
