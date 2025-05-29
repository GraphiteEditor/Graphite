/// Running this test will generate a `bindings.ts` file containing every type annotated with `specta::Type`.
#[test]
fn generate_ts_types() {
	use crate::messages::prelude::FrontendMessage;
	use specta::TypeCollection;
	use specta_typescript::{BigIntExportBehavior, Typescript};

	Typescript::default()
		.bigint(BigIntExportBehavior::Number)
		.export_to("../frontend/src/bindings.ts", TypeCollection::default().register::<FrontendMessage>())
		.unwrap();
}
