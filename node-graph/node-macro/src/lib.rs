use crate::crate_ident::CrateIdent;
use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use syn::GenericParam;

mod buffer_struct;
mod codegen;
mod crate_ident;
mod derive_choice_type;
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

/// Generate meta-information for an enum.
///
/// `#[widget(F)]` on a type indicates the type of widget to use to display/edit the type, currently `Radio` and `Dropdown` are supported.
///
/// `#[label("Foo")]` on a variant overrides the default UI label (which is otherwise the name converted to title case). All labels are collected into a [`core::fmt::Display`] impl.
///
/// `#[icon("tag"))]` sets the icon to use when a variant is shown in a menu or radio button.
///
/// Doc comments on a variant become tooltip text.
#[proc_macro_derive(ChoiceType, attributes(widget, menu_separator, label, icon))]
pub fn derive_choice_type(input_item: TokenStream) -> TokenStream {
	derive_choice_type::derive_choice_type_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()).into()
}

/// Derive a struct to implement `ShaderStruct`, see that for docs.
#[proc_macro_derive(BufferStruct)]
pub fn derive_buffer_struct(input_item: TokenStream) -> TokenStream {
	let crate_ident = CrateIdent::default();
	TokenStream::from(buffer_struct::derive_buffer_struct(&crate_ident, input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}
