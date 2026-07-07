use crate::crate_ident::CrateIdent;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{AttrStyle, Attribute, Error, Expr, Fields, Ident, ItemStruct, Lit, LitStr, Meta, Type};

/// One field of a `#[node_macro::destructure]` struct, parsed from the struct definition.
struct DestructureField {
	ident: Ident,
	ty: Type,
	/// The connector label shown in the UI: the `#[name("...")]` override, or the field name converted to title case.
	display_name: String,
	/// Tooltip text collected from the field's doc comments.
	description: String,
	/// The field's doc attributes, re-emitted onto the generated extractor node function.
	doc_attrs: Vec<Attribute>,
}

pub fn destructure_impl(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
	if !attr.is_empty() {
		return Err(Error::new(attr.span(), "The `destructure` attribute takes no arguments"));
	}

	let mut item_struct = syn::parse2::<ItemStruct>(item).map_err(|e| Error::new(e.span(), format!("`destructure` must be applied to a struct: {e}")))?;

	if !item_struct.generics.params.is_empty() || item_struct.generics.where_clause.is_some() {
		return Err(Error::new_spanned(
			&item_struct.generics,
			"A `destructure` struct cannot have generic parameters or a where clause, since each field must have a concrete type",
		));
	}
	let Fields::Named(named_fields) = &mut item_struct.fields else {
		return Err(Error::new_spanned(&item_struct.fields, "A `destructure` struct must have named fields, one per connector"));
	};
	if named_fields.named.is_empty() {
		return Err(Error::new_spanned(named_fields, "A `destructure` struct must have at least one field"));
	}

	// Collect each field's connector metadata, stripping the `#[name(...)]` and `#[primary]` helper attributes from the emitted struct
	let mut fields = Vec::new();
	let mut primary_field_index = None;
	for (field_index, field) in named_fields.named.iter_mut().enumerate() {
		let ident = field.ident.clone().expect("Named fields always have an identifier");

		if let Some(position) = field.attrs.iter().position(|field_attr| field_attr.path().is_ident("primary")) {
			let primary_attr = field.attrs.remove(position);
			if !matches!(primary_attr.meta, Meta::Path(_)) {
				return Err(Error::new_spanned(&primary_attr, "Expected a bare `#[primary]` with no arguments"));
			}
			if primary_field_index.is_some() {
				return Err(Error::new_spanned(&primary_attr, "At most one field may be marked `#[primary]`"));
			}
			primary_field_index = Some(field_index);
		}

		let mut display_name = None;
		if let Some(position) = field.attrs.iter().position(|field_attr| field_attr.path().is_ident("name")) {
			let name_attr = field.attrs.remove(position);
			let name_literal: LitStr = name_attr
				.parse_args()
				.map_err(|e| Error::new_spanned(&name_attr, format!("Expected `#[name(\"...\")]` with a string literal: {e}")))?;
			display_name = Some(name_literal.value());
		}
		let display_name = display_name.unwrap_or_else(|| ident.to_string().to_case(Case::Title));

		let doc_attrs: Vec<Attribute> = field.attrs.iter().filter(|field_attr| field_attr.path().is_ident("doc")).cloned().collect();
		let description = doc_attrs
			.iter()
			.filter_map(|doc_attr| {
				if doc_attr.style != AttrStyle::Outer {
					return None;
				}
				let Meta::NameValue(name_value) = &doc_attr.meta else { return None };
				let Expr::Lit(expr_lit) = &name_value.value else { return None };
				let Lit::Str(text) = &expr_lit.lit else { return None };
				Some(text.value().trim().to_string())
			})
			.collect::<Vec<_>>()
			.join("\n");

		fields.push(DestructureField {
			ident,
			ty: field.ty.clone(),
			display_name,
			description,
			doc_attrs,
		});
	}

	// Registration lists the fields in output-connector order, so a `#[primary]` field moves to the front where it
	// becomes the node's primary output in place of the hidden output that otherwise carries the whole struct
	let has_primary = primary_field_index.is_some();
	if let Some(primary_field_index) = primary_field_index {
		let primary_field = fields.remove(primary_field_index);
		fields.insert(0, primary_field);
	}

	let crate_ident = CrateIdent::default();
	let gcore = crate_ident.gcore()?;
	let struct_ident = item_struct.ident.clone();
	let struct_snake_name = struct_ident.to_string().to_case(Case::Snake);

	// Generate a hidden extractor node per field by running the regular node codegen pipeline on a synthesized function.
	// Each extractor takes the struct by value and returns one field, so the preprocessor can wire them up as a multi-output node's secondary outputs.
	let mut extractor_nodes = Vec::new();
	let mut extractor_input_modules = Vec::new();
	for field in &fields {
		let field_ident = &field.ident;
		let field_ty = &field.ty;
		let doc_attrs = &field.doc_attrs;

		let extractor_fn_name = format_ident!("{struct_snake_name}_{field_ident}");
		extractor_input_modules.push(extractor_fn_name.clone());

		// An empty category keeps the extractor out of the editor's node catalog
		let extractor_display_name = format!("{struct_ident} {}", field.display_name);
		let node_attr = quote!(category(""), name(#extractor_display_name));
		// Each field output inherits the struct item's attributes, passing them through like any other kernel
		let node_fn = quote! {
			#(#doc_attrs)*
			fn #extractor_fn_name(_: impl #gcore::Ctx, source: #gcore::list::Item<#struct_ident>) -> #gcore::list::Item<#field_ty> {
				let (source, attributes) = source.into_parts();
				#gcore::list::Item::from_parts(source.#field_ident, attributes)
			}
		};

		extractor_nodes.push(crate::parsing::new_node_fn(node_attr, node_fn)?);
	}

	// Register the struct's destructure metadata, keyed by the TypeIds of the struct and its ranked wire forms, so the
	// preprocessor and editor can recognize nodes returning this struct and expand them into the generated extractor nodes
	let field_names = fields.iter().map(|field| field.display_name.as_str()).collect::<Vec<_>>();
	let field_descriptions = fields.iter().map(|field| field.description.as_str()).collect::<Vec<_>>();
	let field_types = fields.iter().map(|field| &field.ty).collect::<Vec<_>>();

	let registration_module = format_ident!("_{struct_snake_name}_destructure");
	let registry_name = format_ident!(
		"__node_registry_{}_{}Destructure",
		crate::codegen::NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
		struct_ident
	);
	let wasm_shim = if cfg!(feature = "disable-registration") {
		quote!()
	} else {
		quote! {
			#[cfg(target_family = "wasm")]
			#[unsafe(no_mangle)]
			extern "C" fn #registry_name() {
				register_destructure();
			}
		}
	};

	let registration = quote! {
		#[doc(hidden)]
		mod #registration_module {
			use super::*;
			use #gcore::ctor::ctor;
			use #gcore::registry::{DESTRUCTURE_METADATA, DestructureFieldMetadata, DestructureMetadata};

			#[cfg_attr(not(target_family = "wasm"), ctor)]
			fn register_destructure() {
				let metadata = DestructureMetadata {
					fields: vec![
						#(
							DestructureFieldMetadata {
								name: #field_names,
								description: #field_descriptions,
								extractor: super::#extractor_input_modules::IDENTIFIER,
								ty: #gcore::concrete!(#field_types),
							},
						)*
					],
					has_primary: #has_primary,
					struct_name: ::std::any::type_name::<#struct_ident>(),
				};

				// Registered under the bare struct and both ranked wire forms, since registry rows record whichever the node's return type resolved as
				let mut registry = DESTRUCTURE_METADATA.lock().unwrap();
				registry.insert(::std::any::TypeId::of::<#gcore::list::Item<#struct_ident>>(), metadata.clone());
				registry.insert(::std::any::TypeId::of::<#gcore::list::List<#struct_ident>>(), metadata.clone());
				registry.insert(::std::any::TypeId::of::<#struct_ident>(), metadata);
			}

			#wasm_shim
		}
	};

	Ok(quote! {
		#item_struct

		#(#extractor_nodes)*

		#registration
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn expect_error(attr: TokenStream2, item: TokenStream2, message_fragment: &str) {
		let error = destructure_impl(attr, item).expect_err("Expected the destructure macro to reject this input");
		let message = error.to_string();
		assert!(message.contains(message_fragment), "Expected error containing `{message_fragment}`, got `{message}`");
	}

	#[test]
	fn rejects_arguments() {
		expect_error(
			quote!(some_argument),
			quote!(
				struct Test {
					x: f64,
				}
			),
			"takes no arguments",
		);
	}

	#[test]
	fn rejects_non_structs() {
		expect_error(
			quote!(),
			quote!(
				enum Test {
					Variant,
				}
			),
			"must be applied to a struct",
		);
	}

	#[test]
	fn rejects_tuple_structs() {
		expect_error(
			quote!(),
			quote!(
				struct Test(f64, f64);
			),
			"must have named fields",
		);
	}

	#[test]
	fn rejects_generic_structs() {
		expect_error(
			quote!(),
			quote!(
				struct Test<T> {
					x: T,
				}
			),
			"cannot have generic parameters",
		);
	}

	#[test]
	fn rejects_empty_structs() {
		expect_error(
			quote!(),
			quote!(
				struct Test {}
			),
			"at least one field",
		);
	}

	#[test]
	fn rejects_multiple_primary_fields() {
		expect_error(
			quote!(),
			quote!(
				struct Test {
					#[primary]
					x: f64,
					#[primary]
					y: f64,
				}
			),
			"At most one field",
		);
	}

	#[test]
	fn rejects_primary_attribute_with_arguments() {
		expect_error(
			quote!(),
			quote!(
				struct Test {
					#[primary(true)]
					x: f64,
				}
			),
			"bare `#[primary]`",
		);
	}

	#[test]
	fn rejects_malformed_name_attribute() {
		expect_error(
			quote!(),
			quote!(
				struct Test {
					#[name(42)]
					x: f64,
				}
			),
			"string literal",
		);
	}
}
