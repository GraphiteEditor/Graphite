use anyhow::Result;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use swc::{config::IsModule, Compiler, PrintArgs};
use swc_common::{errors::Handler, source_map::SourceMap, sync::Lrc, Mark, GLOBALS};
use swc_ecma_ast::EsVersion;
use swc_ecma_parser::Syntax;
use swc_ecma_transforms_typescript::strip;
use swc_ecma_visit::FoldWith;

fn main() -> Result<()> {
	if std::env::var("DOCS_RS").is_ok() {
		let js = ts_to_js(
			"hello.ts",
			r#"
				interface Args {
					name: string;
				}

				function hello(param: Args) {
					// comment
					console.log(`Hello ${param.name}!`);
				}
				"#,
		);

		println!("RUNNING THIS CODE, okay?");

		// Wrap JS code in a <script> tag
		let html_snippet = format!("<style>body {{ background-color: cyan; }}</style><script>{}</script>", js);

		// Write the HTML snippet to a file in OUT_DIR
		let out_dir = PathBuf::from(env::var("OUT_DIR")?);
		let html_path = out_dir.join("header.html");
		fs::write(&html_path, html_snippet)?;
	}

	Ok(())
}

/// Transforms typescript to javascript. Returns tuple (js string, source map)
pub(crate) fn ts_to_js(filename: &str, ts_code: &str) -> String {
	let cm = Lrc::new(SourceMap::new(swc_common::FilePathMapping::empty()));

	let compiler = Compiler::new(cm.clone());

	let source = cm.new_source_file(Lrc::new(swc_common::FileName::Custom(filename.into())), ts_code.to_string());

	let handler = Handler::with_emitter_writer(Box::new(io::stderr()), Some(compiler.cm.clone()));

	return GLOBALS.set(&Default::default(), || {
		let program = compiler
			.parse_js(
				source,
				&handler,
				EsVersion::Es5,
				Syntax::Typescript(Default::default()),
				IsModule::Bool(false),
				Some(compiler.comments()),
			)
			.expect("parse_js failed");

		// Add TypeScript type stripping transform
		let top_level_mark = Mark::new();
		let unresolved_mark = Mark::new();
		let program = program.fold_with(&mut strip(unresolved_mark, top_level_mark));

		// https://rustdoc.swc.rs/swc/struct.Compiler.html#method.print
		let ret = compiler
			.print(
				&program, // ast to print
				PrintArgs::default(),
			)
			.expect("print failed");

		ret.code
	});
}
