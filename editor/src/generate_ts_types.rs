/// Running this test will generate a `types.ts` file at the root of the repo,
/// containing every type annotated with `specta::Type`
// #[cfg(all(test, feature = "specta-export"))]
#[ignore]
#[test]
fn generate_ts_types() {
	use crate::messages::prelude::FrontendMessage;
	use specta::ts::{export_named_datatype, BigIntExportBehavior, ExportConfig};
	use specta::{NamedType, TypeMap};
	use std::fs::File;
	use std::io::Write;

	let config = ExportConfig::new().bigint(BigIntExportBehavior::Number);

	let mut type_map = TypeMap::default();

	let datatype = FrontendMessage::definition_named_data_type(&mut type_map);

	let mut export = String::new();

	export += &export_named_datatype(&config, &datatype, &type_map).unwrap();

	type_map
		.iter()
		.map(|(_, v)| v)
		.flat_map(|v| export_named_datatype(&config, v, &type_map))
		.for_each(|e| export += &format!("\n\n{e}"));

	let mut file = File::create("../types.ts").unwrap();

	write!(file, "{export}").ok();
}
