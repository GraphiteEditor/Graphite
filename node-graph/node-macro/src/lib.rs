use crate::crate_ident::CrateIdent;
use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use syn::GenericParam;

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

/// Marks a struct as destructurable at node boundaries, splitting its fields into individual node connectors.
///
/// When a `#[node_macro::node]` function returns a struct tagged with this attribute, that node becomes a multi-output node:
/// each struct field is exposed as a named secondary output connector in the graph UI. The destructuring itself is performed
/// by hidden extractor nodes which this macro generates, one per field. Those extractor nodes exist only in the transient
/// runtime network produced by the Graphene preprocessor; they are never shown in the graph UI, saved to documents, or
/// serialized when copying nodes.
///
/// Output names default to the field name converted to title case. Use `#[name("...")]` on a field to override that
/// when the automatic conversion doesn't format correctly. Doc comments on fields are recorded as connector descriptions.
///
/// By default the node has no primary output: a hidden primary output carries the whole struct and the fields appear as
/// secondary outputs. Marking at most one field with `#[primary]` makes that field the node's primary output instead.
///
/// The struct is computed once and shared across all outputs when a Memoize implementation is registered for its type
/// (see the `MemoizeNode` entries in `interpreted-executor`'s node registry); otherwise the node re-evaluates per
/// connected output.
///
/// The struct must have named fields with concrete (non-generic) types, and the value must be able to flow through the
/// graph, which in practice means deriving `dyn_any::DynAny` plus `Clone`, `Debug`, and being `Send + Sync`.
///
/// The same registration is planned to eventually drive destructured *inputs*, where a single struct parameter of a node
/// function expands into one input connector per field, grouped in the Properties panel.
///
/// ```ignore
/// #[node_macro::destructure]
/// #[derive(Debug, Clone, Copy, dyn_any::DynAny)]
/// pub struct Vec2Components {
/// 	/// The X component of the vec2.
/// 	x: f64,
/// 	/// The Y component of the vec2.
/// 	y: f64,
/// }
///
/// #[node_macro::node(name("Split Vec2"), category("Math: Vec2"))]
/// fn split_vec2(_: impl Ctx, vec2: DVec2) -> Vec2Components {
/// 	Vec2Components { x: vec2.x, y: vec2.y }
/// }
/// ```
#[proc_macro_error]
#[proc_macro_attribute]
pub fn destructure(attr: TokenStream, item: TokenStream) -> TokenStream {
	destructure::destructure_impl(attr.into(), item.into()).unwrap_or_else(|err| err.to_compile_error()).into()
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
