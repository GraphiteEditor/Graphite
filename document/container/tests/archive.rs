#![cfg(any(feature = "zip", feature = "xz"))]

use document_container::Container;
use document_container::archive::{Archive, ArchiveWriter};
use document_container::backends::folder::FolderBackend;
use document_container::backends::memory::MemoryBackend;

fn entries() -> Vec<(&'static str, &'static [u8])> {
	vec![
		("manifest.json", br#"{"format":"gdd"}"#),
		("document.json", b"{\"registry\":\"...\"}"),
		("history.jsonl", b"{\"rev\":1}\n{\"rev\":2}\n"),
		("resources/abc123", &[0xDE, 0xAD, 0xBE, 0xEF]),
		("resources/xyz789", b"another resource"),
	]
}

fn assert_round_trip(restored: &MemoryBackend) {
	assert_eq!(restored.read("manifest.json").unwrap().as_slice(), br#"{"format":"gdd"}"#);
	assert_eq!(restored.read("document.json").unwrap().as_slice(), b"{\"registry\":\"...\"}");
	assert_eq!(restored.read("history.jsonl").unwrap().as_slice(), b"{\"rev\":1}\n{\"rev\":2}\n");
	assert_eq!(restored.read("resources/abc123").unwrap().as_slice(), &[0xDE, 0xAD, 0xBE, 0xEF]);
	assert_eq!(restored.read("resources/xyz789").unwrap().as_slice(), b"another resource");
}

#[cfg(feature = "zip")]
#[test]
fn zip_round_trip() {
	use document_container::archive::Zip;
	use std::io::Cursor;

	let mut buffer = Cursor::new(Vec::new());
	let mut writer = Zip::writer(&mut buffer).unwrap();
	for (path, bytes) in entries() {
		writer.write_entry(path, bytes).unwrap();
	}
	writer.finish().unwrap();

	let mut restored = MemoryBackend::new();
	<Zip as Archive>::open(Cursor::new(buffer.get_ref()), &mut restored).unwrap();
	assert_round_trip(&restored);
}

#[cfg(feature = "zip")]
#[test]
fn zip_deserialize_streams_into_folder_backend() {
	use document_container::archive::Zip;
	use std::io::Cursor;

	let mut buffer = Cursor::new(Vec::new());
	let mut writer = Zip::writer(&mut buffer).unwrap();
	for (path, bytes) in entries() {
		writer.write_entry(path, bytes).unwrap();
	}
	writer.finish().unwrap();

	let dir = tempfile::tempdir().unwrap();
	let mut restored = FolderBackend::create(dir.path()).unwrap();
	<Zip as Archive>::open(Cursor::new(buffer.get_ref()), &mut restored).unwrap();

	assert_eq!(restored.read("manifest.json").unwrap().as_slice(), br#"{"format":"gdd"}"#);
	assert_eq!(restored.read("resources/abc123").unwrap().as_slice(), &[0xDE, 0xAD, 0xBE, 0xEF]);
}

#[cfg(feature = "xz")]
#[test]
fn xz_round_trip() {
	use document_container::archive::Xz;
	use std::io::Cursor;

	let mut buffer = Cursor::new(Vec::new());
	let mut writer = Xz::writer(&mut buffer).unwrap();
	for (path, bytes) in entries() {
		writer.write_entry(path, bytes).unwrap();
	}
	writer.finish().unwrap();

	let mut restored = MemoryBackend::new();
	<Xz as Archive>::open(Cursor::new(buffer.get_ref()), &mut restored).unwrap();
	assert_round_trip(&restored);
}
