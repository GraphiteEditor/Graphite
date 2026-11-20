//! Migrations read values that are stored in blobs. The large values they write go back into blobs.

use document_graph_storage::blob::{BLOB_THRESHOLD, BlobBytes};
use document_graph_storage::{BlobHash, NodeInput, Value};
use migration_core::{Inputs, Node, decl, literal, literal_of, migration};
use migration_runner::MigrationBlobs;
use migration_runner::testing::Fixture;

/// Appends a suffix to the text in the first input.
#[migration("blob_nodes::text"@1 => @2)]
pub fn append_to_text(node: &Node, inputs: &mut Inputs) {
	let text: String = literal_of(&node.inputs()[0].input).unwrap_or_default();
	inputs.set(0, literal(format!("{text}!")));
}

/// A large literal stored as a blob, together with its blob bytes.
fn blob_literal(text: &str, blobs: &mut BlobBytes) -> (NodeInput, BlobHash) {
	let mut input = literal(text.to_string());
	input.move_into_blob(blobs);
	let hash = input.blob().expect("the literal is large enough to be a blob");
	(input, hash)
}

#[test]
fn migrations_read_blob_values_and_write_large_ones_back_into_blobs() {
	let text = "a".repeat(BLOB_THRESHOLD);
	let mut stored = BlobBytes::new();
	let (input, original) = blob_literal(&text, &mut stored);

	let mut fixture = Fixture::new(&AppendToText);
	let node = fixture.add(decl!("blob_nodes::text"@1), vec![input]);
	let mut blobs = MigrationBlobs::new(&stored);
	let migrated = fixture.migrate_blobs(&AppendToText, &mut blobs).unwrap();

	let (_, appended) = blob_literal(&format!("{text}!"), &mut BlobBytes::new());
	assert_eq!(
		fixture.inputs(&migrated, node),
		[NodeInput::Value {
			value: Value::Blob(appended),
			exposed: false
		}]
	);
	let added = blobs.into_added();
	assert!(added.bytes.contains_key(&appended), "the new value's bytes are handed back");
	assert_eq!(added.bases.get(&appended), Some(&original), "it is based on the blob its slot held");
}
