use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use std::fs;
use std::path::Path;
use toml::{Table, Value};

enum CustomValue {
	String(String),
	Integer(i64),
	Float(f64),
	Boolean(bool),
	Array(Vec<CustomValue>),
}

impl ToTokens for CustomValue {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		match self {
			Self::String(x) => x.to_tokens(tokens),
			Self::Integer(x) => {
				let x: proc_macro2::TokenStream = format!("{:?}", x).parse().unwrap();
				x.to_tokens(tokens)
			}
			Self::Float(x) => {
				let x: proc_macro2::TokenStream = format!("{:?}", x).parse().unwrap();
				x.to_tokens(tokens)
			}
			Self::Boolean(x) => x.to_tokens(tokens),
			Self::Array(x) => quote! { [ #( #x ),* ] }.to_tokens(tokens),
		}
	}
}

impl From<Value> for CustomValue {
	fn from(value: Value) -> Self {
		match value {
			Value::String(x) => Self::String(x),
			Value::Integer(x) => Self::Integer(x),
			Value::Float(x) => Self::Float(x),
			Value::Boolean(x) => Self::Boolean(x),
			Value::Array(x) => Self::Array(x.into_iter().map(|x| x.into()).collect()),
			_ => panic!("Unsupported data type"),
		}
	}
}

pub fn build_camera_data() -> TokenStream {
	let mut camera_data: Vec<(String, Table)> = Vec::new();

	let mut path = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).to_path_buf();
	path.push("camera_data");

	fs::read_dir(path).unwrap().for_each(|entry| {
		let company_name_path = entry.unwrap().path();
		if !company_name_path.is_dir() {
			panic!("camera_data should only contain folders of company names")
		}

		let company_name = company_name_path.file_name().unwrap().to_str().unwrap().to_string();

		fs::read_dir(company_name_path).unwrap().for_each(|entry| {
			let model_path = entry.unwrap().path();
			if !model_path.is_file() || model_path.extension().unwrap() != "toml" {
				panic!("The folders within camera_data should only contain toml files")
			}

			let name = company_name.clone() + " " + model_path.file_stem().unwrap().to_str().unwrap();

			let mut values: Table = toml::from_str(&fs::read_to_string(model_path).unwrap()).unwrap();

			if let Some(val) = values.get_mut("xyz_to_camera") {
				*val = Value::Array(val.as_array().unwrap().iter().map(|x| Value::Integer((x.as_float().unwrap() * 10_000.) as i64)).collect());
			}

			camera_data.push((name, values))
		});
	});

	let x: Vec<_> = camera_data
		.iter()
		.map(|(name, camera_data)| {
			let keys: Vec<_> = camera_data.keys().map(|key| syn::Ident::new(key, proc_macro2::Span::call_site())).collect();
			let values: Vec<CustomValue> = camera_data.values().cloned().map(|x| x.into()).collect();

			quote! {
				(
					#name,
					CameraData {
						#( #keys: #values, )*
						..CameraData::DEFAULT
					}
				)
			}
		})
		.collect();

	quote!([ #(#x),* ]).into()
}
