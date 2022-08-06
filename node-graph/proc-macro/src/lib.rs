use proc_macro::TokenStream;
use proc_macro_roids::*;
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, FnArg, ItemFn, Pat, Type};

fn extract_type(a: FnArg) -> Type {
	match a {
		FnArg::Typed(p) => *p.ty, // notice `ty` instead of `pat`
		_ => panic!("Not supported on types with `self`!"),
	}
}

fn extract_arg_types(fn_args: Punctuated<FnArg, syn::token::Comma>) -> Vec<Type> {
	fn_args.into_iter().map(extract_type).collect::<Vec<_>>()
}

fn extract_arg_idents(fn_args: Punctuated<FnArg, syn::token::Comma>) -> Vec<Pat> {
	fn_args.into_iter().map(extract_arg_pat).collect::<Vec<_>>()
}

fn extract_arg_pat(a: FnArg) -> Pat {
	match a {
		FnArg::Typed(p) => *p.pat,
		_ => panic!("Not supported on types with `self`!"),
	}
}

#[proc_macro_attribute] // 2
pub fn to_node(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let string = item.to_string();
	let item2 = item;
	let parsed = parse_macro_input!(item2 as ItemFn); // 3

	//item.extend(generate_to_string(parsed, string)); // 4
	//item
	generate_to_string(parsed, string)
}

fn generate_to_string(parsed: ItemFn, string: String) -> TokenStream {
	let whole_function = parsed.clone();
	//let fn_body = parsed.block; // function body
	let sig = parsed.sig; // function signature

	//let vis = parsed.vis; // visibility, pub or not
	let generics = sig.generics;
	let fn_args = sig.inputs; // comma separated args
	let fn_return_type = sig.output; // return type
	let fn_name = sig.ident; // function name/identifier
	let idents = extract_arg_idents(fn_args.clone());
	let types = extract_arg_types(fn_args);
	let types = types.iter().map(|t| t.to_token_stream()).collect::<Vec<_>>();
	let idents = idents.iter().map(|t| t.to_token_stream()).collect::<Vec<_>>();
	let _const_idents = idents
		.iter()
		.map(|t| {
			let name = t.to_string().to_uppercase();
			quote! {#name}
		})
		.collect::<Vec<_>>();

	let node_fn_name = fn_name.append("_node");
	let struct_name = fn_name.append("_input");
	let return_type_string = fn_return_type.to_token_stream().to_string().replace("->", "");
	let arg_type_string = types.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
	let error = format!("called {} with the wrong type", fn_name);

	let x = quote! {
		//#whole_function
		mod #fn_name {
			#[derive(Copy, Clone)]
			type F32Node<'n> = &'n (dyn Node<'n, (), Output = &'n (dyn Any + 'static)> + 'n);
			struct #struct_name {
				#(#idents: #types,)*
			}
			impl Node for #struct_name {

			}

		}
		fn #node_fn_name #generics() -> Node<'static> {
			Node { func: Box::new(move |x| {
				let args = x.downcast::<(#(#types,)*)>().expect(#error);
				let (#(#idents,)*) = *args;
				#whole_function

				Box::new(#fn_name(#(#idents,)*))
				}),
				code: #string.to_string(),
				return_type: #return_type_string.trim().to_string(),
				args: format!("({})",#arg_type_string.trim()),
				position: (0., 0.),
			}
		}
	};
	//panic!("{}\n{:?}", x.to_string(), x);
	x.into()
}
