use proc_macro2::{Ident, Literal, TokenStream as TokenStream2};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Field, PathArguments, Type};

/// Check if a specified `#[widget_builder target]` attribute can be found in the list
fn has_attribute(attrs: &[Attribute], target: &str) -> bool {
	attrs
		.iter()
		.filter(|attr| attr.path().to_token_stream().to_string() == "widget_builder")
		.any(|attr| attr.meta.require_list().is_ok_and(|list| list.tokens.to_string() == target))
}

/// Make setting strings easier by allowing all types that `impl Into<String>`
///
/// Returns the new input type and a conversion to the original.
fn easier_string_assignment(field_ty: &Type, field_ident: &Ident) -> (TokenStream2, TokenStream2) {
	if let Type::Path(type_path) = field_ty {
		if let Some(last_segment) = type_path.path.segments.last() {
			// Check if this type is a `String`
			// Based on https://stackoverflow.com/questions/66906261/rust-proc-macro-derive-how-do-i-check-if-a-field-is-of-a-primitive-type-like-b
			if last_segment.ident == Ident::new("String", last_segment.ident.span()) {
				return (
					quote::quote_spanned!(type_path.span() => impl Into<String>),
					quote::quote_spanned!(field_ident.span() => #field_ident.into()),
				);
			}
		}
	}
	(quote::quote_spanned!(field_ty.span() => #field_ty), quote::quote_spanned!(field_ident.span() => #field_ident))
}

/// Extract the identifier of the field (which should always be present)
fn extract_ident(field: &Field) -> syn::Result<&Ident> {
	field
		.ident
		.as_ref()
		.ok_or_else(|| syn::Error::new_spanned(field, "Constructing a builder not supported for unnamed fields"))
}

/// Find the type passed into the builder and the right hand side of the assignment.
///
/// Applies special behavior for easier String and WidgetCallback assignment.
fn find_type_and_assignment(field: &Field) -> syn::Result<(TokenStream2, TokenStream2)> {
	let field_ty = &field.ty;
	let field_ident = extract_ident(field)?;

	let (mut function_input_ty, mut assignment) = easier_string_assignment(field_ty, field_ident);

	// Check if type is `WidgetCallback`
	if let Type::Path(type_path) = field_ty {
		if let Some(last_segment) = type_path.path.segments.last() {
			if let PathArguments::AngleBracketed(generic_args) = &last_segment.arguments {
				if let Some(first_generic) = generic_args.args.first() {
					if last_segment.ident == Ident::new("WidgetCallback", last_segment.ident.span()) {
						// Assign builder pattern to assign the closure directly
						function_input_ty = quote::quote_spanned!(field_ty.span() => impl Fn(&#first_generic) -> crate::messages::message::Message + 'static + Send + Sync);
						assignment = quote::quote_spanned!(field_ident.span() => crate::messages::layout::utility_types::layout_widget::WidgetCallback::new(#field_ident));
					}
				}
			}
		}
	}
	Ok((function_input_ty, assignment))
}

// Construct a builder function for a specific field in the struct
fn construct_builder(field: &Field) -> syn::Result<TokenStream2> {
	// Check if this field should be skipped with `#[widget_builder(skip)]`
	if has_attribute(&field.attrs, "skip") {
		return Ok(Default::default());
	}
	let field_ident = extract_ident(field)?;

	// Create a doc comment literal describing the behaviour of the function
	let doc_comment = Literal::string(&format!("Set the `{field_ident}` field using a builder pattern."));

	let (function_input_ty, assignment) = find_type_and_assignment(field)?;

	// Create builder function
	Ok(quote::quote_spanned!(field.span() =>
		#[doc = #doc_comment]
		pub fn #field_ident(mut self, #field_ident: #function_input_ty) -> Self{
			self.#field_ident = #assignment;
			self
		}
	))
}

pub fn derive_widget_builder_impl(input_item: TokenStream2) -> syn::Result<TokenStream2> {
	let input = syn::parse2::<DeriveInput>(input_item)?;

	let struct_name_ident = input.ident;

	// Extract the struct fields
	let fields = match &input.data {
		Data::Enum(enum_data) => return Err(syn::Error::new_spanned(enum_data.enum_token, "Derive widget builder is not supported for enums")),
		Data::Union(union_data) => return Err(syn::Error::new_spanned(union_data.union_token, "Derive widget builder is not supported for unions")),
		Data::Struct(struct_data) => &struct_data.fields,
	};

	// Create functions based on each field
	let builder_functions = fields.iter().map(construct_builder).collect::<Result<Vec<_>, _>>()?;

	// Check if this should not have the `widget_holder()` function due to a `#[widget_builder(not_widget_holder)]` attribute
	let widget_holder_fn = if !has_attribute(&input.attrs, "not_widget_holder") {
		// A doc comment for the widget_holder function
		let widget_holder_doc_comment = Literal::string(&format!("Wrap {struct_name_ident} as a WidgetHolder."));

		// Construct the `widget_holder` function
		quote::quote! {
			#[doc = #widget_holder_doc_comment]
			pub fn widget_holder(self) -> crate::messages::layout::utility_types::layout_widget::WidgetHolder{
				crate::messages::layout::utility_types::layout_widget::WidgetHolder::new( crate::messages::layout::utility_types::layout_widget::Widget::#struct_name_ident(self))
			}
		}
	} else {
		quote::quote!()
	};

	// The new function takes any fields tagged with `#[widget_builder(constructor)]` as arguments.
	let new_fn = {
		// A doc comment for the new function
		let new_doc_comment = Literal::string(&format!("Create a new {struct_name_ident}, based on default values."));

		let is_constructor = |field: &Field| has_attribute(&field.attrs, "constructor");

		let idents = fields.iter().filter(|field| is_constructor(field)).map(extract_ident).collect::<Result<Vec<_>, _>>()?;
		let types_and_assignments = fields.iter().filter(|field| is_constructor(field)).map(find_type_and_assignment).collect::<Result<Vec<_>, _>>()?;
		let (types, assignments): (Vec<_>, Vec<_>) = types_and_assignments.into_iter().unzip();

		let construction = if idents.is_empty() {
			quote::quote!(Default::default())
		} else {
			let default = (idents.len() != fields.len()).then_some(quote::quote!(..Default::default())).unwrap_or_default();
			quote::quote! {
				Self {
					#(#idents: #assignments,)*
					#default
				}
			}
		};

		quote::quote! {
			#[doc = #new_doc_comment]
			pub fn new(#(#idents: #types),*) -> Self {
				#construction
			}
		}
	};

	// Construct the code block
	Ok(quote::quote! {
		impl #struct_name_ident {
			#new_fn

			#(#builder_functions)*

			#widget_holder_fn
		}
	})
}
