use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use syn::GenericParam;

mod codegen;
mod derive_choice_type;
mod parsing;
mod validation;

/// Used to create a node definition.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
	// Performs the `node_impl` macro's functionality of attaching an `impl Node for TheGivenStruct` block to the node struct
	parsing::new_node_fn(attr.into(), item.into()).into()
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
	TokenStream::from(derive_choice_type::derive_choice_type_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

#[cfg(test)]
mod tests {
	use super::*;
	use proc_macro2::TokenStream;
	use quote::quote;
	use similar_asserts::assert_eq;

	fn token_stream_to_string(token_stream: TokenStream) -> String {
		prettyplease::unparse(&syn::parse2(token_stream).unwrap())
	}

	#[test]
	fn test_node_end_to_end() {
		let attr = quote!(category("Math: Arithmetic"), path(graphene_core::TestNode), skip_impl);
		let input = quote!(
			/// Multi
			/// Line
			fn add(a: f64, b: f64) -> f64 {
				a + b
			}
		);
		let parsed = parsing::new_node_fn(attr, input).into();

		let expected = quote! {
			/// Underlying implementation for [#struct_name]
			#[inline]
			#[allow(clippy::too_many_arguments)]
			pub(crate) fn add<'n>(a: f64, b: f64) -> f64 {
				a + b
			}
			#[automatically_derived]
			impl<'n, Node0, F0> graphene_core::Node<'n, f64> for _add_mod::AddNode<Node0>
			where
				F0: core::future::Future<Output = f64> + graphene_core::WasmNotSend + 'n,
				for<'all> f64: graphene_core::WasmNotSend,
				Node0: graphene_core::Node<'n, f64, Output = F0> + graphene_core::WasmNotSync,
				f64: 'n,
			{
				type Output = graphene_core::registry::DynFuture<'n, f64>;
				#[inline]
				fn eval(&'n self, __input: f64) -> Self::Output {
					Box::pin(async move {
						use graphene_core::misc::Clampable;
						let b = self.b.eval(__input.clone()).await;
						self::add(__input, b)
					})
				}
			}
			#[doc(inline)]
			pub use _add_mod::AddNode;
			#[doc(hidden)]
			#[doc(hidden)]
			mod _add_mod {
				use super::*;
				use graphene_core as gcore;
				use gcore::{
					Node, NodeIOTypes, concrete, fn_type, fn_type_fut, future, ProtoNodeIdentifier,
					WasmNotSync, NodeIO,
				};
				use gcore::value::ClonedNode;
				use gcore::ops::TypeNode;
				use gcore::registry::{
					NodeMetadata, FieldMetadata, NODE_REGISTRY, NODE_METADATA, DynAnyNode,
					DowncastBothNode, DynFuture, TypeErasedBox, PanicNode, RegistryValueSource,
					RegistryWidgetOverride,
				};
				use gcore::ctor::ctor;
				static _IMPORT_STUB_ADD_MOD: core::marker::PhantomData<()> = core::marker::PhantomData;
				#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
				pub struct AddNode<Node0> {
					pub(super) b: Node0,
				}
				#[automatically_derived]
				impl<'n, Node0> AddNode<Node0> {
					#[allow(clippy::too_many_arguments)]
					pub fn new(b: Node0) -> Self {
						Self { b }
					}
				}
				#[cfg_attr(not(target_arch = "wasm32"), ctor)]
				fn register_metadata() {
					let metadata = NodeMetadata {
						display_name: "Add",
						category: Some("Math: Arithmetic"),
						description: "Multi\nLine\n",
						properties: None,
						fields: vec![
							FieldMetadata {
								name: "B",
								widget_override:RegistryWidgetOverride::None,
								description: "",
								exposed: false,
								value_source: RegistryValueSource::None,
								default_type: None,
								number_min: None,
								number_max: None,
								number_mode_range: None,
								number_display_decimal_places: None,
								number_step: None,
								unit: None,
							},
						],
					};
					NODE_METADATA
						.lock()
						.unwrap()
						.insert(
							format!(
								"{}::{}", stringify!(graphene_core::TestNode) .replace(' ', ""),
								stringify!(AddNode)
							),
							metadata,
						);
				}
			}
		};

		assert_eq!(token_stream_to_string(expected), token_stream_to_string(parsed));
	}
}
