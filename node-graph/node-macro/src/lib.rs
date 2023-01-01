use proc_macro::TokenStream;
use quote::{format_ident, ToTokens};
use syn::{parse_macro_input, FnArg, Ident, ItemFn, Pat, PatIdent, ReturnType};

#[proc_macro_attribute]
pub fn node_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
	let node_name = parse_macro_input!(attr as Ident);
	let function = parse_macro_input!(item as ItemFn);

	let function_name = &function.sig.ident;
	let mut function_inputs = function.sig.inputs.iter().filter_map(|arg| if let FnArg::Typed(typed_arg) = arg { Some(typed_arg) } else { None });

	// Extract primary input as first argument
	let primary_input = function_inputs.next().expect("Primary input required - set to `()` if not needed.");
	let Pat::Ident(PatIdent{ident: primary_input_ident,..} ) =&*primary_input.pat else {
		panic!("Expected ident as primary input.");
	};
	let primary_input_ty = &primary_input.ty;

	// Extract secondary inputs as all other arguments
	let secondary_inputs = function_inputs.collect::<Vec<_>>();
	let secondary_idents = secondary_inputs
		.iter()
		.map(|input| {
			let Pat::Ident(PatIdent { ident: primary_input_ident,.. }) = &*input.pat else { panic!("Expected ident for secondary input."); };
			primary_input_ident
		})
		.collect::<Vec<_>>();

	// Extract the output type of the entire node - `()` by default
	let output = if let ReturnType::Type(_, ty) = &function.sig.output {
		ty.to_token_stream()
	} else {
		quote::quote!(())
	};

	// Generics are simply `S0` through to `Sn-1` where n is the number of secondary inputs
	let generics = (0..secondary_inputs.len()).map(|x| format_ident!("S{x}")).collect::<Vec<_>>();
	// Bindings for all of the above generics to a node with an input of `()` and an output of the type in the function
	let where_clause = secondary_inputs
		.iter()
		.zip(&generics)
		.map(|(ty, name)| {
			let ty = &ty.ty;
			quote::quote!(#name: Node<(), Output = #ty>)
		})
		.collect::<Vec<_>>();

	quote::quote! {
		#function

		impl <#(#generics),*> Node<#primary_input_ty> for #node_name<#(#generics),*>
			where
			#(#where_clause),* {

			type Output = #output;
			fn eval(self, #primary_input_ident: #primary_input_ty) -> #output{
				#function_name(#primary_input_ident #(, self.#secondary_idents.eval(()))*)
			}
		}

		impl <#(#generics),*> Node<#primary_input_ty> for &#node_name<#(#generics),*>
			where
			#(#where_clause + Copy),* {

			type Output = #output;
			fn eval(self, #primary_input_ident: #primary_input_ty) -> #output{
				#function_name(#primary_input_ident #(, self.#secondary_idents.eval(()))*)
			}
		}

		impl <#(#generics),*> #node_name<#(#generics),*>
			where
			#(#where_clause + Copy),* {
				pub fn new(#(#secondary_idents: #generics),*) -> Self{
					Self{
						#(#secondary_idents),*
					}
				}
		}
	}
	.into()
}
