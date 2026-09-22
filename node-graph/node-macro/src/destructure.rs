use crate::crate_ident::CrateIdent;
use crate::parsing::{new_destructure_extractor_fn, peel_item, peel_list};
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{AttrStyle, Attribute, Data, DeriveInput, Error, Expr, Fields, Ident, Lit, LitStr, Meta, Type, Visibility};

/// One field of a `#[derive(Destructure)]` struct, parsed from the struct definition.
struct DestructureField {
	ident: Ident,
	vis: Visibility,
	/// The declared wire type, `Item<T>` or `List<T>`.
	ty: Type,
	/// The element type `T` carried by the wire.
	element: Type,
	/// Whether the wire is a whole `List<T>` rather than a rank-0 `Item<T>` cell.
	is_list: bool,
	/// The connector label shown in the UI: the `#[name("...")]` override, or the field name converted to title case.
	display_name: String,
	/// Tooltip text collected from the field's doc comments.
	description: String,
	/// The field's doc attributes, re-emitted onto the twin's field and the generated extractor node function.
	doc_attrs: Vec<Attribute>,
}

pub fn derive_destructure_impl(item: TokenStream2) -> syn::Result<TokenStream2> {
	let input = syn::parse2::<DeriveInput>(item)?;

	let Data::Struct(data_struct) = &input.data else {
		return Err(Error::new(input.ident.span(), "`Destructure` can only be derived for a struct"));
	};
	if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
		return Err(Error::new_spanned(
			&input.generics,
			"A `Destructure` struct cannot have generic parameters or a where clause, since each field must have a concrete wire type",
		));
	}
	let Fields::Named(named_fields) = &data_struct.fields else {
		return Err(Error::new_spanned(&data_struct.fields, "A `Destructure` struct must have named fields, one per connector"));
	};
	if named_fields.named.is_empty() {
		return Err(Error::new_spanned(named_fields, "A `Destructure` struct must have at least one field"));
	}

	// Collect each field's connector metadata from its wire type, doc comments, and the `#[name(...)]` and `#[primary]` helper attributes
	let mut fields = Vec::new();
	let mut primary_field_index = None;
	for (field_index, field) in named_fields.named.iter().enumerate() {
		let ident = field.ident.clone().expect("Named fields always have an identifier");

		let (element, is_list) = match (peel_item(&field.ty), peel_list(&field.ty)) {
			(Some(element), _) => (element, false),
			(None, Some(element)) => (element, true),
			(None, None) => {
				return Err(Error::new_spanned(
					&field.ty,
					format!("The field `{ident}` must be a wire type: `Item<T>` for one cell or `List<T>` for a whole list"),
				));
			}
		};

		if let Some(primary_attr) = field.attrs.iter().find(|field_attr| field_attr.path().is_ident("primary")) {
			if !matches!(primary_attr.meta, Meta::Path(_)) {
				return Err(Error::new_spanned(primary_attr, "Expected a bare `#[primary]` with no arguments"));
			}
			if primary_field_index.is_some() {
				return Err(Error::new_spanned(primary_attr, "At most one field may be marked `#[primary]`"));
			}
			primary_field_index = Some(field_index);
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
			is_list,
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
	let struct_ident = &input.ident;
	let struct_vis = &input.vis;
	let struct_snake_name = struct_ident.to_string().to_case(Case::Snake);
	let mapped_ident = format_ident!("{struct_ident}List");

	// The rank-lifted twin returned by the node's mapped variant, lifting every field to a whole-list wire
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

	// Generate a hidden extractor node per field, written against `DestructureField` so its single identifier registers one
	// row taking the struct and one taking the twin, and give both forms the field accessor it relies on
	let mut field_accessors = Vec::new();
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

		field_accessors.push(quote! {
			#[automatically_derived]
			impl #gcore::registry::DestructureField<#index> for #struct_ident {
				type Wire = #field_ty;

				fn field(self) -> Self::Wire {
					self.#field_ident
				}
			}

			#[automatically_derived]
			impl #gcore::registry::DestructureField<#index> for #mapped_ident {
				type Wire = #gcore::list::List<#element>;

				fn field(self) -> Self::Wire {
					self.#field_ident
				}
			}
		});

		let extractor_fn_name = format_ident!("{struct_snake_name}_{field_ident}");
		extractor_modules.push(extractor_fn_name.clone());

		// An empty category keeps the extractor out of the editor's node catalog
		let extractor_display_name = format!("{struct_ident} {}", field.display_name);
		let node_attr = quote!(category(""), name(#extractor_display_name));
		let node_fn = quote! {
			#(#doc_attrs)*
			fn #extractor_fn_name<S: #gcore::registry::DestructureField<#index>>(_: impl #gcore::Ctx, #[implementations(#struct_ident, #mapped_ident)] source: S) -> S::Wire {
				source.field()
			}
		};
		extractor_nodes.push(new_destructure_extractor_fn(node_attr, node_fn)?);
	}

	// The metadata the node macro records on each `destructure_output` node returning this struct, and the twin collection
	// its mapped variant performs per frame slot: an `Item<T>` field pushes into its list and a `List<T>` field extends it
	let field_names = fields.iter().map(|field| field.display_name.as_str()).collect::<Vec<_>>();
	let field_descriptions = fields.iter().map(|field| field.description.as_str()).collect::<Vec<_>>();
	let field_types = fields.iter().map(|field| {
		let element = &field.element;
		match field.is_list {
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
		match field.is_list {
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

	Ok(quote! {
		#mapped_struct

		#destructure_impl

		#(#field_accessors)*

		#(#extractor_nodes)*
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
			"must be a wire type",
		);
	}

	#[test]
	fn rejects_multiple_primary_fields() {
		expect_error(
			quote!(
				struct Test {
					#[primary]
					x: Item<f64>,
					#[primary]
					y: Item<f64>,
				}
			),
			"At most one field",
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
