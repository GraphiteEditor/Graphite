/// Running this test will generate a `types.ts` file at the root of the repo,
/// containing every type annotated with `specta::Type`
// #[cfg(all(test, feature = "specta-export"))]
#[ignore]
#[test]
fn generate_ts_types() {
	use crate::messages::prelude::FrontendMessage;
	use specta::{
		ts::{export_datatype, BigIntExportBehavior, ExportConfiguration},
		DefOpts, NamedType, Type, TypeDefs,
	};
	use std::fs::File;
	use std::io::Write;

	let config = ExportConfiguration::new().bigint(BigIntExportBehavior::Number);

	let mut type_map = TypeDefs::new();

	let datatype = FrontendMessage::named_data_type(
		DefOpts {
			parent_inline: false,
			type_map: &mut type_map,
		},
		&FrontendMessage::definition_generics().into_iter().map(Into::into).collect::<Vec<_>>(),
	)
	.unwrap();

	let mut export = String::new();

	export += &export_datatype(&config, &datatype).unwrap();

	type_map.values().flatten().flat_map(|v| export_datatype(&config, v)).for_each(|e| export += &format!("\n\n{e}"));

	let mut file = File::create("../types.ts").unwrap();

	write!(file, "{export}").ok();
}
