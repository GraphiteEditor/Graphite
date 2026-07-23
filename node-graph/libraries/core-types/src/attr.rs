//! Typed attribute keys.
//!
//! Each key is a zero-sized marker implementing [`Attr`], which ties the name (the string
//! stored in the attribute store) to the Rust value type.
//!
//! Keys are declared with the [`node_macro::attrs!`] macro: `Name: Type` entries, where
//! `namespace { ... }` blocks contribute a `namespace:` name prefix. The key name is
//! derived mechanically from the ident (UpperCamel -> snake_case). An optional `= value`
//! after the type declares the key's implicit default (see [`Attr::implicit_default`]).

use crate::Color;
use crate::list::NodeIdPath;
use glam::{DAffine2, DVec2};
use graphene_hash::CacheHash;
use std::fmt::Debug;

pub trait Attr {
	type Value: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static;
	fn name() -> &'static str;

	/// The value an item without this attribute is considered to have: the value type's `Default`,
	/// unless the key's `attrs!` declaration overrides it with `= value`.
	fn implicit_default() -> Self::Value {
		Default::default()
	}
}

node_macro::attrs! {
	/// Item's `DAffine2` transformation, composed multiplicatively through nested groups.
	Transform: DAffine2,
	/// Item's `BlendMode`, controlling how it composites with content beneath it.
	BlendMode: crate::blending::BlendMode,
	/// Item's opacity multiplier, composed multiplicatively through nested groups. Affects content clipped to the item.
	Opacity: f64 = 1.,
	/// Item's fill opacity multiplier. Like opacity but does not affect content clipped to the item.
	OpacityFill: f64 = 1.,
	/// Whether an item inherits the alpha of the content beneath it (clipping mask).
	ClippingMask: bool,
	/// Byte offset where a regex match begins ('Regex Find All', 'Regex Capture' text nodes).
	Start: u64,
	/// Byte offset where a regex match ends ('Regex Find All', 'Regex Capture' text nodes).
	End: u64,
	/// A regex named-capture-group's name, or empty for unnamed groups ('Regex Capture' text node).
	Name: String,
	/// A JSON value's type (`"string"`, `"number"`, `"object"`, etc.) from 'JSON Query All'.
	Type: String,
	/// Artboard's top-left corner in document coordinates.
	Location: DVec2,
	/// Artboard's width and height.
	Dimensions: DVec2,
	/// Artboard's background fill.
	Background: Color,
	/// Whether an artboard clips content to its bounds.
	Clip: bool,
	/// Text item's font size in document-space units.
	FontSize: f64,
	/// Text item's line height as a ratio of the font size.
	LineHeight: f64,
	/// Text item's extra spacing between letters in document-space units.
	LetterSpacing: f64,
	/// Text item's maximum line-wrap width in document-space units.
	MaxWidth: Option<f64>,
	/// Text item's maximum block height in document-space units, past which lines are not drawn.
	MaxHeight: Option<f64>,
	/// Text item's faux-italic letter tilt angle in degrees.
	LetterTilt: f64,
		editor {
		/// Path from the root network to the layer node owning this item.
		/// Used by editor tools to route clicks/selection back to the originating layer.
		LayerPath: NodeIdPath,
		/// Affine mapping the unit square `[(0, 0), (1, 1)]` (top-left convention) onto the 'Text'
		/// node's text frame in this item's local space. Each item carries the frame relative to its own
		/// glyph origin so it survives `Index Elements` filtering. The Text tool reads this to position
		/// its drag cage. Stored as an affine to allow non-axis-aligned frames in the future.
		TextFrame: DAffine2,
	},
}
