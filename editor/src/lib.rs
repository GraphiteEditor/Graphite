extern crate graphite_proc_macros;

// `macro_use` puts these macros into scope for all descendant code files
#[macro_use]
mod macros;
#[macro_use]
extern crate log;

pub mod application;
pub mod consts;
pub mod dispatcher;
pub mod messages;
pub mod test_utils;
pub mod utility_traits;

/// Running this test will generate a `types.ts` file at the root of the repo,
/// containing every type annotated with `specta::Type`
#[cfg(test)]
#[test]
fn export_types() {
	use specta::ts::{BigIntExportBehavior, ExportConfiguration};

	specta::export::ts(&ExportConfiguration { bigint: BigIntExportBehavior::Number }, "../types.ts").unwrap();
}
