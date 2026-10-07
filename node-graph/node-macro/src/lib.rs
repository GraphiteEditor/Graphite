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

/// Derives `Destructure` for a struct returned by a multi-output node, making each of its `Item<T>` or `List<T>` fields an output connector.
///
/// A node function returning such a struct (rather than an `Item` or `List`) becomes a multi-output node. The preprocessor
/// connects each output to a hidden extractor node this derive generates for that field.
///
/// Output names default to the field name converted to title case, which `#[name("...")]` on a field can override. Doc comments
/// on fields become connector descriptions. The node's primary output is hidden unless the first field is marked `#[primary]`.
///
/// The derive also generates a `{Struct}Mapped` struct with each field as a `List<T>`, which the node returns when mapped over a list.
///
/// The struct must have named fields with concrete types, and derive `dyn_any::DynAny`, `Clone`, and `Debug`.
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
/// #[node_macro::node(category("Math: Vec2"), name("Split Vec2"))]
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
