extern crate proc_macro;

mod build_camera_data;
mod tag_derive;

use proc_macro::TokenStream;

#[proc_macro_derive(Tag)]
pub fn tag_derive(input: TokenStream) -> TokenStream {
	tag_derive::tag_derive(input)
}

#[proc_macro]
pub fn build_camera_data(_: TokenStream) -> TokenStream {
	build_camera_data::build_camera_data()
}
