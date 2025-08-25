use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::parse::Parse;
use syn::{Attribute, DeriveInput, Expr, LitStr, Meta};

pub fn derive_choice_type_impl(input_item: TokenStream) -> syn::Result<TokenStream> {
	let input = syn::parse2::<DeriveInput>(input_item).unwrap();

	match input.data {
		syn::Data::Enum(data_enum) => derive_enum(&input.attrs, input.ident, data_enum),
		_ => Err(syn::Error::new(input.ident.span(), "Only enums are supported at the moment")),
	}
}

struct Type {
	basic_item: BasicItem,
	widget_hint: WidgetHint,
}

enum WidgetHint {
	Radio,
	Dropdown,
}
impl Parse for WidgetHint {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		let tokens: Ident = input.parse()?;
		if tokens == "Radio" {
			Ok(Self::Radio)
		} else if tokens == "Dropdown" {
			Ok(Self::Dropdown)
		} else {
			Err(syn::Error::new_spanned(tokens, "Widget must be either Radio or Dropdown"))
		}
	}
}

#[derive(Default)]
struct BasicItem {
	label: String,
	description: Option<String>,
	icon: Option<String>,
}
impl BasicItem {
	fn read_attribute(&mut self, attribute: &Attribute) -> syn::Result<()> {
		if attribute.path().is_ident("label") {
			let token: LitStr = attribute.parse_args()?;
			self.label = token.value();
		}
		if attribute.path().is_ident("icon") {
			let token: LitStr = attribute.parse_args()?;
			self.icon = Some(token.value());
		}
		if attribute.path().is_ident("doc") {
			if let Meta::NameValue(meta_name_value) = &attribute.meta {
				if let Expr::Lit(el) = &meta_name_value.value {
					if let syn::Lit::Str(token) = &el.lit {
						self.description = Some(token.value());
					}
				}
			}
		}
		Ok(())
	}
}

struct Variant {
	name: Ident,
	basic_item: BasicItem,
}

fn derive_enum(enum_attributes: &[Attribute], name: Ident, input: syn::DataEnum) -> syn::Result<TokenStream> {
	let mut enum_info = Type {
		basic_item: BasicItem::default(),
		widget_hint: WidgetHint::Dropdown,
	};
	for attribute in enum_attributes {
		enum_info.basic_item.read_attribute(attribute)?;
		if attribute.path().is_ident("widget") {
			enum_info.widget_hint = attribute.parse_args()?;
		}
	}

	let mut variants = vec![Vec::new()];
	for variant in &input.variants {
		let mut basic_item = BasicItem::default();

		for attribute in &variant.attrs {
			if attribute.path().is_ident("menu_separator") {
				attribute.meta.require_path_only()?;
				variants.push(Vec::new());
			}
			basic_item.read_attribute(attribute)?;
		}

		if basic_item.label.is_empty() {
			basic_item.label = ident_to_label(&variant.ident);
		}

		variants.last_mut().unwrap().push(Variant {
			name: variant.ident.clone(),
			basic_item,
		})
	}
	let display_arm: Vec<_> = variants
		.iter()
		.flat_map(|variants| variants.iter())
		.map(|variant| {
			let variant_name = &variant.name;
			let variant_label = &variant.basic_item.label;
			quote! { #name::#variant_name => write!(f, #variant_label), }
		})
		.collect();

	let crate_name = {
		let crate_name = proc_macro_crate::crate_name("graphene-core-shaders")
			.or_else(|_e| proc_macro_crate::crate_name("graphene-core"))
			.map_err(|e| {
				syn::Error::new(
					Span::call_site(),
					format!("Failed to find location of 'graphene_core' or 'graphene-core-shaders'. Make sure it is imported as a dependency: {e}"),
				)
			})?;
		match crate_name {
			proc_macro_crate::FoundCrate::Itself => quote!(crate),
			proc_macro_crate::FoundCrate::Name(name) => {
				let identifier = Ident::new(&name, Span::call_site());
				quote! { #identifier }
			}
		}
	};

	let enum_description = match &enum_info.basic_item.description {
		Some(s) => {
			let s = s.trim();
			quote! { Some(#s) }
		}
		None => quote! { None },
	};
	let group: Vec<_> = variants
		.iter()
		.map(|variants| {
			let items = variants
				.iter()
				.map(|variant| {
					let vname = &variant.name;
					let vname_str = variant.name.to_string();
					let label = &variant.basic_item.label;
					let docstring = match &variant.basic_item.description {
						Some(s) => {
							let s = s.trim();
							quote! { Some(#s) }
						}
						None => quote! { None },
					};
					let icon = match &variant.basic_item.icon {
						Some(s) => quote! { Some(#s) },
						None => quote! { None },
					};
					quote! {
						(
							#name::#vname, #crate_name::choice_type::VariantMetadata {
								name: #vname_str,
								label: #label,
								docstring: #docstring,
								icon: #icon,
							}
						),
					}
				})
				.collect::<Vec<_>>();
			quote! { &[ #(#items)* ], }
		})
		.collect();
	let widget_hint = match enum_info.widget_hint {
		WidgetHint::Radio => quote! { RadioButtons },
		WidgetHint::Dropdown => quote! { Dropdown },
	};
	Ok(quote! {
		impl #crate_name::AsU32 for #name {
			fn as_u32(&self) -> u32 {
				*self as u32
			}
		}

		impl #crate_name::choice_type::ChoiceTypeStatic for #name {
			const WIDGET_HINT: #crate_name::choice_type::ChoiceWidgetHint = #crate_name::choice_type::ChoiceWidgetHint::#widget_hint;
			const DESCRIPTION: Option<&'static str> = #enum_description;
			fn list() -> &'static [&'static [(Self, #crate_name::choice_type::VariantMetadata)]] {
				&[ #(#group)* ]
			}
		}

		impl core::fmt::Display for #name {
			fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
				match self {
					#( #display_arm )*
				}
			}
		}
	})
}

fn ident_to_label(id: &Ident) -> String {
	use convert_case::{Case, Casing};
	id.to_string().from_case(Case::Pascal).to_case(Case::Title)
}
