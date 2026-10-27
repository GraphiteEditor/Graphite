use crate::test_utils::test_prelude::*;

const TEMPLATE_DIRECTORY: &str = "../node-graph/compiler-spike/workloads/templates";

/// Draws three rectangle layers with the Rectangle tool and saves the document as the compiler spike's layer template.
/// Run with `cargo test -p graphite-editor export_compiler_spike_templates -- --ignored`, then rerun the spike's workload export.
#[tokio::test]
#[ignore]
async fn export_compiler_spike_templates() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	for i in 0..3 {
		let offset = i as f64 * 20.;
		editor.draw_rect(offset, offset, offset + 10., offset + 10.).await;
	}

	std::fs::create_dir_all(TEMPLATE_DIRECTORY).unwrap();
	std::fs::write(format!("{TEMPLATE_DIRECTORY}/rectangle-layers.graphite"), editor.active_document().serialize_document()).unwrap();
}
