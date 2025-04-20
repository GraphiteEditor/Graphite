use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::parse::Parse;
use syn::{Attribute, DeriveInput, Expr, LitStr, Meta};

pub fn derive_choice_type_impl(input_item: TokenStream) -> syn::Result<TokenStream> {
	let input = syn::parse2::<DeriveInput>(input_item).unwrap();

	match input.data {
		syn::Data::Enum(en) => derive_enum(&input.attrs, input.ident, en),
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
		let tok: Ident = input.parse()?;
		if tok == "Radio" {
			Ok(Self::Radio)
		} else if tok == "Dropdown" {
			Ok(Self::Dropdown)
		} else {
			Err(syn::Error::new_spanned(tok, "Widget must be either Radio or Dropdown"))
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
	fn read_attr(&mut self, attr: &Attribute) -> syn::Result<()> {
		if attr.path().is_ident("label") {
			let tok: LitStr = attr.parse_args()?;
			self.label = tok.value();
		}
		if attr.path().is_ident("icon") {
			let tok: LitStr = attr.parse_args()?;
			self.icon = Some(tok.value());
		}
		if attr.path().is_ident("doc") {
			if let Meta::NameValue(nv) = &attr.meta {
				if let Expr::Lit(el) = &nv.value {
					if let syn::Lit::Str(tok) = &el.lit {
						self.description = Some(tok.value());
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

fn derive_enum(enum_attrs: &[Attribute], name: Ident, input: syn::DataEnum) -> syn::Result<TokenStream> {
	let mut enum_info = Type {
		basic_item: BasicItem::default(),
		widget_hint: WidgetHint::Dropdown,
	};
	for att in enum_attrs {
		enum_info.basic_item.read_attr(att)?;
		if att.path().is_ident("widget") {
			enum_info.widget_hint = att.parse_args()?;
		}
	}

	let mut variants = vec![Vec::new()];
	for va in &input.variants {
		let mut basic_item = BasicItem::default();

		for attr in &va.attrs {
			if attr.path().is_ident("menu_separator") {
				attr.meta.require_path_only()?;
				variants.push(Vec::new());
			}
			basic_item.read_attr(attr)?;
		}

		if basic_item.label.len() == 0 {
			basic_item.label = ident_to_label(&va.ident);
		}

		variants.last_mut().unwrap().push(Variant { name: va.ident.clone(), basic_item })
	}

	let crate_name = proc_macro_crate::crate_name("graphene-core").map_err(|e| {
		syn::Error::new(
			proc_macro2::Span::call_site(),
			format!("Failed to find location of graphene_core. Make sure it is imported as a dependency: {}", e),
		)
	})?;
	let crate_name = match crate_name {
		proc_macro_crate::FoundCrate::Itself => quote!(crate),
		proc_macro_crate::FoundCrate::Name(n) => {
			let i = Ident::new(&n, Span::call_site());
			quote! {#i}
		}
	};

	let group: Vec<_> = variants
		.iter()
		.map(|vg| {
			let items = vg
				.iter()
				.map(|v| {
					let vname = &v.name;
					let vname_str = v.name.to_string();
					let label = &v.basic_item.label;
					let docstring = match &v.basic_item.description {
						Some(s) => {
							let s = s.trim();
							quote! { Some(::alloc::borrow::Cow::Borrowed(#s)) }
						}
						None => quote! { None },
					};
					let icon = match &v.basic_item.icon {
						Some(s) => quote! { Some(::alloc::borrow::Cow::Borrowed(#s)) },
						None => quote! { None },
					};
					quote! { ( #name::#vname, #crate_name::registry::VariantMetadata {
						name: ::alloc::borrow::Cow::Borrowed(#vname_str),
						label: ::alloc::borrow::Cow::Borrowed(#label),
						docstring: #docstring,
						icon: #icon,
					}), }
				})
				.collect::<Vec<_>>();
			quote! { &[ #(#items)* ], }
		})
		.collect();
	let display_arm: Vec<_> = variants
		.iter()
		.map(|vg| vg.iter())
		.flatten()
		.map(|v| {
			let vn = &v.name;
			let vl = &v.basic_item.label;
			quote! { #name::#vn => write!(f, #vl), }
		})
		.collect();
	let widget_hint = match enum_info.widget_hint {
		WidgetHint::Radio => quote! { RadioButtons },
		WidgetHint::Dropdown => quote! { Dropdown },
	};
	Ok(quote! {
		impl #crate_name::vector::misc::AsU32 for #name {
			fn as_u32(&self) -> u32 {
				*self as u32
			}
		}

		impl #crate_name::registry::ChoiceTypeStatic for #name {
			const WIDGET_HINT: #crate_name::registry::ChoiceWidgetHint = #crate_name::registry::ChoiceWidgetHint::#widget_hint;
			fn list() -> &'static [&'static [(Self, #crate_name::registry::VariantMetadata)]] {
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
