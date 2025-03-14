/// Running this test will generate a `types.ts` file at the root of the repo,
/// containing every type annotated with `specta::Type`

#[test]
fn generate_ts_types() {
	use specta::TypeCollection;
	use specta_typescript::{BigIntExportBehavior, Typescript};

	use crate::messages::prelude::FrontendMessage;

	Typescript::default()
		.bigint(BigIntExportBehavior::Number)
		.export_to("../frontend/src/bindings.ts", TypeCollection::default().register::<FrontendMessage>())
		.unwrap();
}
