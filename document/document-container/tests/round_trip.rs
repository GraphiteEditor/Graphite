use document_container::backends::folder::FolderBackend;
use document_container::backends::memory::MemoryBackend;
use document_container::{AnyContainer, Container, ContainerError};

fn run_round_trip<C: Container>(container: C) {
	container.write("manifest.json", br#"{"format":"gdd"}"#).unwrap();
	container.write("resources/abc123", &[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
	container.write("resources/xyz789", b"another resource").unwrap();

	assert!(container.exists("manifest.json"));
	assert!(container.exists("resources/abc123"));
	assert!(!container.exists("does-not-exist"));

	let manifest = container.read("manifest.json").unwrap();
	assert_eq!(manifest.as_slice(), br#"{"format":"gdd"}"#);

	let blob = container.read("resources/abc123").unwrap();
	assert_eq!(blob.as_slice(), &[0xDE, 0xAD, 0xBE, 0xEF]);

	let top_level = container.list("").unwrap();
	assert!(top_level.iter().any(|p| p == "manifest.json"));
	assert!(
		top_level.iter().all(|p| !p.starts_with("resources/")),
		"list(\"\") must not descend into subdirectories, got {top_level:?}"
	);

	let mut resources = container.list("resources").unwrap();
	resources.sort();
	assert_eq!(resources, vec!["resources/abc123".to_string(), "resources/xyz789".to_string()]);

	container.remove("resources/abc123").unwrap();
	assert!(!container.exists("resources/abc123"));
	assert!(matches!(container.read("resources/abc123"), Err(ContainerError::NotFound(_))));
}

#[test]
fn memory_backend_round_trip() {
	run_round_trip(MemoryBackend::new());
}

#[test]
fn folder_backend_round_trip() {
	let dir = tempfile::tempdir().unwrap();
	let backend = FolderBackend::create(dir.path()).unwrap();
	run_round_trip(backend);
}

fn run_list_and_remove_semantics<C: Container>(container: C) {
	container.write("dir/file", b"x").unwrap();

	// Removing a missing path is idempotent.
	container.remove("dir/missing").unwrap();
	container.remove("dir/file").unwrap();
	container.remove("dir/file").unwrap();

	// Listing a missing prefix yields an empty list.
	assert_eq!(container.list("nonexistent").unwrap(), Vec::<String>::new());

	// Listing a prefix that names a file is an error.
	container.write("manifest.json", b"{}").unwrap();
	assert!(matches!(container.list("manifest.json"), Err(ContainerError::NotADirectory(_))));
	assert!(matches!(container.list_dirs("manifest.json"), Err(ContainerError::NotADirectory(_))));
}

#[test]
fn memory_backend_list_and_remove_semantics() {
	run_list_and_remove_semantics(MemoryBackend::new());
}

#[test]
fn folder_backend_list_and_remove_semantics() {
	let dir = tempfile::tempdir().unwrap();
	run_list_and_remove_semantics(FolderBackend::create(dir.path()).unwrap());
}

#[test]
#[cfg(unix)]
fn folder_backend_rejects_symlink_escape() {
	let outside = tempfile::tempdir().unwrap();
	std::fs::write(outside.path().join("secret"), b"sensitive").unwrap();

	let dir = tempfile::tempdir().unwrap();
	let backend = FolderBackend::create(dir.path()).unwrap();
	// A symlink planted inside the root pointing outside it must not be traversable.
	std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();

	let result = backend.read("link/secret");
	assert!(matches!(result, Err(ContainerError::InvalidPath(_))), "symlink escape should be rejected, got {result:?}");

	// Listing must reject the symlinked prefix too, not just reads, so it can't leak entry names from outside the root.
	assert!(matches!(backend.list("link"), Err(ContainerError::InvalidPath(_))), "list through symlink should be rejected");
	assert!(matches!(backend.list_dirs("link"), Err(ContainerError::InvalidPath(_))), "list_dirs through symlink should be rejected");
}

#[test]
fn folder_backend_rejects_path_traversal() {
	let dir = tempfile::tempdir().unwrap();
	let backend = FolderBackend::create(dir.path()).unwrap();
	for bad in ["../escape", "subdir/../escape", "/abs", "back\\slash"] {
		let result = backend.write(bad, b"nope");
		assert!(matches!(result, Err(ContainerError::InvalidPath(_))), "expected InvalidPath for {bad:?}, got {result:?}");
	}
}

fn run_append<C: Container>(container: C) {
	// Appending to a non-existent path creates it — same semantics as `OpenOptions::append().create(true)`.
	container.append("history.jsonl", b"{\"op\":1}\n").unwrap();
	container.append("history.jsonl", b"{\"op\":2}\n").unwrap();
	container.append("history.jsonl", b"{\"op\":3}\n").unwrap();

	let log = container.read("history.jsonl").unwrap();
	assert_eq!(log.as_slice(), b"{\"op\":1}\n{\"op\":2}\n{\"op\":3}\n");
}

#[test]
fn memory_backend_append() {
	run_append(MemoryBackend::new());
}

#[test]
fn folder_backend_append() {
	let dir = tempfile::tempdir().unwrap();
	let backend = FolderBackend::create(dir.path()).unwrap();
	run_append(backend);
}

#[test]
fn any_container_dispatches_to_active_variant() {
	use document_container::AsyncContainer;

	let container = AnyContainer::Memory(MemoryBackend::new());

	futures::executor::block_on(async {
		container.write("manifest.json", br#"{"format":"gdd"}"#).await.unwrap();
		container.append("history.jsonl", b"frame-1\n").await.unwrap();
		container.append("history.jsonl", b"frame-2\n").await.unwrap();

		assert!(container.exists("manifest.json").await);
		let manifest = container.read("manifest.json").await.unwrap();
		assert_eq!(manifest.as_slice(), br#"{"format":"gdd"}"#);

		let history = container.read("history.jsonl").await.unwrap();
		assert_eq!(history.as_slice(), b"frame-1\nframe-2\n");
	});
}

#[test]
fn folder_backend_reads_empty_file() {
	let dir = tempfile::tempdir().unwrap();
	let backend = FolderBackend::create(dir.path()).unwrap();

	backend.write("empty.bin", &[]).unwrap();
	let read_back = backend.read("empty.bin").unwrap();
	assert_eq!(read_back.as_slice(), &[] as &[u8]);
}

#[test]
fn folder_backend_write_sized_fills_via_mmap() {
	let dir = tempfile::tempdir().unwrap();
	let backend = FolderBackend::create(dir.path()).unwrap();

	let payload = b"hello world";
	backend
		.write_sized("resources/sized", payload.len(), &mut |buffer| {
			buffer.copy_from_slice(payload);
			Ok(())
		})
		.unwrap();

	let read_back = backend.read("resources/sized").unwrap();
	assert_eq!(read_back.as_slice(), payload);
}
