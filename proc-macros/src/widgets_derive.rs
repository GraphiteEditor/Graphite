use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::parse::Parse;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;
use syn::{Expr, ExprField, Token};

struct CallbackVisitor {
	clones: Vec<TokenStream>,
	has_input: bool,
}

impl VisitMut for CallbackVisitor {
	fn visit_ident_mut(&mut self, i: &mut Ident) {
		if i.to_string().as_str() == "input" {
			self.has_input = true;
		} else {
			self.clones.push(quote! {#i = #i.clone()});
		}
	}
}

#[derive(Debug)]
struct XMLNode {
	name: Ident,
	fields: Vec<(Ident, Expr)>,
	children: Vec<XMLNode>,
}

impl Parse for XMLNode {1
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		input.parse::<Token![<]>()?;
		let name: Ident = input.parse()?;

		let mut fields = Vec::new();
		while input.peek(syn::Ident) {
			let key = input.parse()?;
			input.parse::<Token![:]>()?;

			let val = input.parse()?;
			input.parse::<Token![,]>()?;

			fields.push((key, val));
		}

		let mut children = Vec::new();
		if input.peek(Token![/]) {
			input.parse::<Token![/]>()?;
		} else {
			input.parse::<Token![>]>()?;

			while input.peek(Token![<]) && !input.peek2(Token![/]) {
				children.push(input.parse()?);
			}
			input.parse::<Token![<]>()?;
			input.parse::<Token![/]>()?;
			let close: Ident = input.parse()?;
			if close != name {
				close.span().unwrap().error(format!("Expected </{}> found </{}>", name, close)).emit();
			}
		}
		input.parse::<Token![>]>()?;

		Ok(Self { name, fields, children })
	}
}
impl XMLNode {
	fn compute(&self) -> TokenStream {
		let mut visitor = CallbackVisitor { clones: Vec::new(), has_input: false };

		let name = &self.name;
		let name_span = self.name.span().unwrap();
		let children_iter = self.children.iter().map(|child| child.compute());
		let fields_iter = self
			.fields
			.iter_mut()
			.map(|(key, val)| {
				if key.to_string().as_str() == "on_update" {
					visitor.visit_expr_mut(val);
					for f in visitor.fields {
						println!("Field with name={}", quote! {#f});
					}
					(key, val)
				//WidgetCallback::new(|input: &NumberInput|
				} else {
					(key, val)
				}
			})
			.map(|(key, val)| quote! {#key: #val.into()});
		let mut fields = quote! {#(#fields_iter),*};
		let children = quote! {vec![#(#children_iter),*]};
		//
		match name.to_string().as_str() {
			// Special cases for when there are children
			"Layout" => quote! {WidgetLayout::new(#children)},
			"Section" => {
				// LayoutRow::Section does not have a default so we manually set the default for the name
				if self.fields.is_empty() {
					fields = quote! {name:String::new()};
				}
				quote! {LayoutRow::Section {#fields, layout: #children}}
			}
			"Row" => {
				// LayoutRow::Row does not have a default so we manually set the default for the name
				if self.fields.is_empty() {
					fields = quote! {name:String::new()};
				}
				quote! {LayoutRow::Row {#fields, widgets: #children}}
			}
			_ => {
				//name_span.error(format!("Invalid node name: {}", name)).emit();
				quote! {WidgetHolder::new(Widget::#name(#name {
					#fields,
					..Default::default()
				}))}
			}
		}
	}
}

pub fn widgets_impl(input_item: TokenStream) -> syn::Result<TokenStream> {
	let root = syn::parse2::<XMLNode>(input_item)?;

	let computed = root.compute();
	println!("{}", computed);

	Ok(quote! {
		{
			use crate::layout::widgets::{*, SeparatorDirection::*, SeparatorType::*};
			#computed
		}
	})
}
