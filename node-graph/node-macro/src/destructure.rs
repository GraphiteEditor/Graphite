use crate::codegen::NODE_ID;
use crate::crate_ident::CrateIdent;
use crate::parsing::{new_destructure_extractor_fn, peel_item, peel_list};
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::sync::atomic::Ordering;
use syn::{AttrStyle, Attribute, Data, DeriveInput, Error, Expr, Fields, Ident, Lit, LitStr, Meta, Type, Visibility};

/// One field of a `#[derive(Destructure)]` struct, parsed from the struct definition.
struct DestructureField {
	ident: Ident,
	vis: Visibility,
	/// The declared type, `Item<T>` or `List<T>`.
	ty: Type,
	/// The `T` of `Item<T>` or `List<T>`.
	element: Type,
	/// The connector label shown in the UI: the `#[name("...")]` override, or the field name converted to title case.
	display_name: String,
	/// Tooltip text collected from the field's doc comments.
	description: String,
	/// The field's doc attributes, re-emitted onto the mapped struct's field and the generated extractor node function.
	doc_attrs: Vec<Attribute>,
}

impl DestructureField {
	fn is_list(&self) -> bool {
		peel_list(&self.ty).is_some()
	}
}

pub fn derive_destructure_impl(item: TokenStream2) -> syn::Result<TokenStream2> {
	let input = syn::parse2::<DeriveInput>(item)?;

	let Data::Struct(data_struct) = &input.data else {
		return Err(Error::new(input.ident.span(), "`Destructure` can only be derived for a struct"));
	};
	if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
		return Err(Error::new_spanned(
			&input.generics,
			"A `Destructure` struct cannot have generic parameters or a where clause, since each field must have a concrete type",
		));
	}
	let Fields::Named(named_fields) = &data_struct.fields else {
		return Err(Error::new_spanned(&data_struct.fields, "A `Destructure` struct must have named fields, one per connector"));
	};
	if named_fields.named.is_empty() {
		return Err(Error::new_spanned(named_fields, "A `Destructure` struct must have at least one field"));
	}

	// Collect each field's connector metadata from its type, doc comments, and `#[name(...)]` and `#[primary]` attributes
	let mut fields = Vec::new();
	let mut has_primary = false;
	for (field_index, field) in named_fields.named.iter().enumerate() {
		let ident = field.ident.clone().expect("Named fields always have an identifier");

		let Some(element) = peel_item(&field.ty).or_else(|| peel_list(&field.ty)) else {
			return Err(Error::new_spanned(&field.ty, format!("The field `{ident}` must be an `Item<T>` or `List<T>`")));
		};

		if let Some(primary_attr) = field.attrs.iter().find(|field_attr| field_attr.path().is_ident("primary")) {
			if !matches!(primary_attr.meta, Meta::Path(_)) {
				return Err(Error::new_spanned(primary_attr, "Expected a bare `#[primary]` with no arguments"));
			}
			if field_index != 0 {
				return Err(Error::new_spanned(primary_attr, "Only the first field can be marked `#[primary]`"));
			}
			has_primary = true;
		}

		let display_name = match field.attrs.iter().find(|field_attr| field_attr.path().is_ident("name")) {
			Some(name_attr) => {
				let name_literal: LitStr = name_attr
					.parse_args()
					.map_err(|e| Error::new_spanned(name_attr, format!("Expected `#[name(\"...\")]` with a string literal: {e}")))?;
				name_literal.value()
			}
			None => ident.to_string().to_case(Case::Title),
		};

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
			vis: field.vis.clone(),
			ty: field.ty.clone(),
			element,
			display_name,
			description,
			doc_attrs,
		});
	}

	let crate_ident = CrateIdent::default();
	let gcore = crate_ident.gcore()?;
	let graphene_core = crate_ident.graphene_core()?;
	let struct_ident = &input.ident;
	let struct_vis = &input.vis;
	let struct_snake_name = struct_ident.to_string().to_case(Case::Snake);
	let mapped_ident = format_ident!("{struct_ident}Mapped");

	// The struct returned by the node's mapped variant, with every field as a `List<T>`
	let mapped_field_defs = fields.iter().map(|field| {
		let DestructureField { ident, vis, element, doc_attrs, .. } = field;
		quote! {
			#(#doc_attrs)*
			#vis #ident: #gcore::list::List<#element>
		}
	});
	let mapped_struct = quote! {
		#[doc(hidden)]
		#[derive(Debug, Clone, dyn_any::DynAny)]
		#struct_vis struct #mapped_ident {
			#(#mapped_field_defs,)*
		}
	};

	// A hidden extractor node per field, generic over `ExtractField` so one node identifier accepts both the struct and its mapped form
	let mut field_extractions = Vec::new();
	let mut extractor_nodes = Vec::new();
	let mut extractor_modules = Vec::new();
	for (index, field) in fields.iter().enumerate() {
		let DestructureField {
			ident: field_ident,
			ty: field_ty,
			element,
			doc_attrs,
			..
		} = field;

		field_extractions.push(quote! {
			#[automatically_derived]
			impl #gcore::registry::ExtractField<#index> for #struct_ident {
				type Field = #field_ty;

				fn extract_field(self) -> Self::Field {
					self.#field_ident
				}
			}

			#[automatically_derived]
			impl #gcore::registry::ExtractField<#index> for #mapped_ident {
				type Field = #gcore::list::List<#element>;

				fn extract_field(self) -> Self::Field {
					self.#field_ident
				}
			}
		});

		let extractor_fn_name = format_ident!("{struct_snake_name}_{field_ident}");
		extractor_modules.push(extractor_fn_name.clone());

		let extractor_display_name = format!("{struct_ident} {}", field.display_name);
		let node_attr = quote!(category("_Field Extractors"), name(#extractor_display_name));
		let node_fn = quote! {
			#(#doc_attrs)*
			fn #extractor_fn_name<S: #gcore::registry::ExtractField<#index>>(_: impl #gcore::Ctx, #[implementations(#struct_ident, #mapped_ident)] source: S) -> S::Field {
				source.extract_field()
			}
		};
		extractor_nodes.push(new_destructure_extractor_fn(node_attr, node_fn)?);
	}

	// The metadata recorded on each node returning this struct, and how its mapped variant appends each frame slot's struct
	let field_names = fields.iter().map(|field| field.display_name.as_str()).collect::<Vec<_>>();
	let field_descriptions = fields.iter().map(|field| field.description.as_str()).collect::<Vec<_>>();
	let field_types = fields.iter().map(|field| {
		let element = &field.element;
		match field.is_list() {
			true => quote!(#gcore::list!(#element)),
			false => quote!(#gcore::item!(#element)),
		}
	});
	let mapped_field_types = fields.iter().map(|field| {
		let element = &field.element;
		quote!(#gcore::list!(#element))
	});
	let mapped_field_inits = fields.iter().map(|field| {
		let ident = &field.ident;
		quote!(#ident: #gcore::list::List::with_capacity(capacity))
	});
	let push_fields = fields.iter().map(|field| {
		let ident = &field.ident;
		match field.is_list() {
			true => quote!(mapped.#ident.extend(self.#ident);),
			false => quote!(mapped.#ident.push(self.#ident);),
		}
	});

	let destructure_impl = quote! {
		#[automatically_derived]
		impl #gcore::registry::Destructure for #struct_ident {
			type Mapped = #mapped_ident;

			fn metadata() -> #gcore::registry::DestructureMetadata {
				#gcore::registry::DestructureMetadata {
					fields: vec![
						#(
							#gcore::registry::DestructureFieldMetadata {
								name: #field_names,
								description: #field_descriptions,
								extractor: #extractor_modules::IDENTIFIER,
								ty: #field_types,
								mapped_ty: #mapped_field_types,
							},
						)*
					],
					has_primary: #has_primary,
					mapped_type: #gcore::concrete!(#mapped_ident),
				}
			}

			fn mapped_with_capacity(capacity: usize) -> Self::Mapped {
				#mapped_ident {
					#(#mapped_field_inits,)*
				}
			}

			fn push_into(self, mapped: &mut Self::Mapped) {
				#(#push_fields)*
			}
		}
	};

	// Memoize lets the node's outputs share one evaluation, and Monitor lets the editor inspect the struct
	let registration_module = format_ident!("_{struct_snake_name}_registration");
	let wasm_registration = format_ident!("__node_registry_{}_{}", NODE_ID.fetch_add(1, Ordering::SeqCst), struct_ident);
	let registration = quote! {
		#[doc(hidden)]
		mod #registration_module {
			use super::*;
			use #gcore::ctor::ctor;

			#[cfg_attr(not(target_family = "wasm"), ctor)]
			fn register_memoize_and_monitor() {
				#graphene_core::memo::register_memoize_and_monitor::<#struct_ident>();
				#graphene_core::memo::register_memoize_and_monitor::<#mapped_ident>();
			}

			#[cfg(target_family = "wasm")]
			#[unsafe(no_mangle)]
			extern "C" fn #wasm_registration() {
				register_memoize_and_monitor();
			}
		}
	};

	Ok(quote! {
		#mapped_struct

		#destructure_impl

		#(#field_extractions)*

		#(#extractor_nodes)*

		#registration
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn expect_error(item: TokenStream2, message_fragment: &str) {
		let error = derive_destructure_impl(item).expect_err("Expected the derive to reject this input");
		let message = error.to_string();
		assert!(message.contains(message_fragment), "Expected error containing `{message_fragment}`, got `{message}`");
	}

	#[test]
	fn rejects_non_structs() {
		expect_error(
			quote!(
				enum Test {
					Variant,
				}
			),
			"can only be derived for a struct",
		);
	}

	#[test]
	fn rejects_tuple_structs() {
		expect_error(
			quote!(
				struct Test(Item<f64>, Item<f64>);
			),
			"must have named fields",
		);
	}

	#[test]
	fn rejects_generic_structs() {
		expect_error(
			quote!(
				struct Test<T> {
					x: Item<T>,
				}
			),
			"cannot have generic parameters",
		);
	}

	#[test]
	fn rejects_empty_structs() {
		expect_error(
			quote!(
				struct Test {}
			),
			"at least one field",
		);
	}

	#[test]
	fn rejects_unranked_fields() {
		expect_error(
			quote!(
				struct Test {
					x: f64,
				}
			),
			"must be an `Item<T>` or `List<T>`",
		);
	}

	#[test]
	fn rejects_primary_on_a_later_field() {
		expect_error(
			quote!(
				struct Test {
					x: Item<f64>,
					#[primary]
					y: Item<f64>,
				}
			),
			"Only the first field",
		);
	}

	#[test]
	fn rejects_primary_attribute_with_arguments() {
		expect_error(
			quote!(
				struct Test {
					#[primary(true)]
					x: Item<f64>,
				}
			),
			"bare `#[primary]`",
		);
	}

	#[test]
	fn rejects_malformed_name_attribute() {
		expect_error(
			quote!(
				struct Test {
					#[name(42)]
					x: Item<f64>,
				}
			),
			"string literal",
		);
	}
}
