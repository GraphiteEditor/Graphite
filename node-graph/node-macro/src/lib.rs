use crate::crate_ident::CrateIdent;
use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;

mod buffer_struct;
mod codegen;
mod crate_ident;
mod derive_choice_type;
mod destructure;
mod parsing;
mod shader_nodes;
mod validation;

/// Used to create a node definition.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
	// Performs the `node_impl` macro's functionality of attaching an `impl Node for TheGivenStruct` block to the node struct
	parsing::new_node_fn(attr.into(), item.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

/// Derives `Destructure` for a struct of wires, making each of its fields an output connector of a multi-output node.
///
/// A `#[node_macro::node(..., destructure_output)]` function returns such a struct directly (not wrapped in `Item` or `List`)
/// and becomes a multi-output node: each field is exposed as a named output connector in the graph UI. Every field must be
/// an `Item<T>` or `List<T>` wire, so each output carries its own rank and attributes. The destructuring itself is performed
/// by hidden extractor nodes which this derive generates, one per field. Those extractor nodes exist only in the transient
/// runtime network produced by the Graphene preprocessor, so they don't appear in the graph UI, saved documents, or copied nodes.
///
/// Output names default to the field name converted to title case. Use `#[name("...")]` on a field to override that
/// when the automatic conversion doesn't format correctly. Doc comments on fields are recorded as connector descriptions.
///
/// By default the primary output is hidden and carries the whole struct, and the fields appear as secondary outputs.
/// Marking at most one field with `#[primary]` makes that field the node's primary output instead.
///
/// The derive also generates the struct's rank-lifted twin, named with a `Mapped` suffix, in which every field is a `List<T>`.
/// The node's mapped variant, used when it is framed over a list, collects one twin from the per-slot structs: an `Item<T>`
/// field pushes into its list and a `List<T>` field extends it, flat-mapping per the rank-2 rule. The extractors accept
/// both forms.
///
/// Each struct type the node returns needs hand-written Memoize and Monitor rows in `interpreted-executor`'s node registry,
/// enforced by a test there, so the struct is computed once and shared across all outputs.
///
/// The struct must have named fields with concrete (non-generic) wire types, and derive `dyn_any::DynAny`, `Clone`, and
/// `Debug`, which the twin derives too.
///
/// ```ignore
/// #[derive(Debug, Clone, dyn_any::DynAny, node_macro::Destructure)]
/// pub struct Vec2Components {
/// 	/// The X component of the vec2.
/// 	pub x: Item<f64>,
/// 	/// The Y component of the vec2.
/// 	pub y: Item<f64>,
/// }
///
/// #[node_macro::node(category("Math: Vec2"), name("Split Vec2"), destructure_output)]
/// fn split_vec2(_: impl Ctx, vec2: Item<DVec2>) -> Vec2Components {
/// 	let (vec2, attributes) = vec2.into_parts();
///
/// 	Vec2Components {
/// 		x: Item::from_parts(vec2.x, attributes.clone()),
/// 		y: Item::from_parts(vec2.y, attributes),
/// 	}
/// }
/// ```
#[proc_macro_error]
#[proc_macro_derive(Destructure, attributes(name, primary))]
pub fn derive_destructure(input_item: TokenStream) -> TokenStream {
	destructure::derive_destructure_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

/// Generate meta-information for an enum.
///
/// `#[widget(F)]` on a type indicates the type of widget to use to display/edit the type, currently `Radio` and `Dropdown` are supported.
///
/// `#[label("Foo")]` on a variant overrides the default UI label (which is otherwise the name converted to title case). All labels are collected into a [`core::fmt::Display`] impl.
///
/// `#[icon("tag"))]` sets the icon to use when a variant is shown in a menu or radio button.
///
/// Doc comments on a variant become tooltip description text.
#[proc_macro_derive(ChoiceType, attributes(widget, menu_separator, label, icon))]
pub fn derive_choice_type(input_item: TokenStream) -> TokenStream {
	derive_choice_type::derive_choice_type_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

/// Derive a struct to implement `ShaderStruct`, see that for docs.
#[proc_macro_derive(BufferStruct)]
pub fn derive_buffer_struct(input_item: TokenStream) -> TokenStream {
	let crate_ident = CrateIdent::default();
	TokenStream::from(buffer_struct::derive_buffer_struct(&crate_ident, input_item).unwrap_or_else(|err| err.to_compile_error()))
}
