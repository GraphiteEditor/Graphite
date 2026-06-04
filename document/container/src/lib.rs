//! Container abstraction for the on-disk side of the `.gdd` document format.
//!
//! A [`Container`] is a virtual filesystem of named byte payloads.
//! Backends include a loose folder, an in-memory map, and an OPFS-backed wasm store.
//! Archive codecs ([`archive::Zip`], [`archive::Xz`]) round-trip a container's contents
//! through a compressed byte stream.
//!
//! Reads return a [`ByteHolder`], an ownership-carrying handle whose variant depends on
//! how the backend produced the bytes (mmap region, owned vector, external file mmap).
//! [`AsyncContainer`] mirrors [`Container`] for inherently async backends; every sync
//! [`Container`] is reachable from async code via a blanket impl.

pub mod archive;
pub mod backends;

pub enum ByteHolder {
	/// Bytes synthesized in memory (decompressed from an archive, produced by serialization).
	/// The only variant available on `target_family = "wasm"`.
	Owned(Vec<u8>),
	/// Bytes mmap'd from a file inside the container.
	#[cfg(not(target_family = "wasm"))]
	Mmapped(MmappedBytes),
	/// Bytes mmap'd from a file outside the container (e.g. a linked resource).
	#[cfg(not(target_family = "wasm"))]
	External { path: std::path::PathBuf, bytes: MmappedBytes },
}

impl ByteHolder {
	pub fn as_slice(&self) -> &[u8] {
		match self {
			ByteHolder::Owned(bytes) => bytes,
			#[cfg(not(target_family = "wasm"))]
			ByteHolder::Mmapped(bytes) => bytes.as_ref(),
			#[cfg(not(target_family = "wasm"))]
			ByteHolder::External { bytes, .. } => bytes.as_ref(),
		}
	}

	/// If the bytes are backed by a real filesystem path, return it. Enables consumers to
	/// short-circuit byte copies with `fs::copy` (CoW on supported filesystems).
	#[cfg(not(target_family = "wasm"))]
	pub fn source_path(&self) -> Option<&std::path::Path> {
		match self {
			ByteHolder::Owned(_) => None,
			ByteHolder::Mmapped(bytes) => Some(bytes.path()),
			ByteHolder::External { path, .. } => Some(path),
		}
	}

	#[cfg(target_family = "wasm")]
	pub fn source_path(&self) -> Option<&std::path::Path> {
		None
	}

	/// Open an external file and produce a [`ByteHolder::External`] backed by mmap.
	#[cfg(not(target_family = "wasm"))]
	pub fn open_external(path: impl Into<std::path::PathBuf>) -> Result<Self> {
		let path = path.into();
		let file = mmap_io::mmap::MemoryMappedFile::open_ro(&path).map_err(|error| ContainerError::Backend(format!("mmap of {path:?} failed: {error}")))?;
		let bytes = MmappedBytes::new(file)?;
		Ok(ByteHolder::External { path, bytes })
	}
}

impl AsRef<[u8]> for ByteHolder {
	fn as_ref(&self) -> &[u8] {
		self.as_slice()
	}
}

impl std::fmt::Debug for ByteHolder {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ByteHolder::Owned(bytes) => f.debug_tuple("Owned").field(&format_args!("{} bytes", bytes.len())).finish(),
			#[cfg(not(target_family = "wasm"))]
			ByteHolder::Mmapped(bytes) => f.debug_tuple("Mmapped").field(&format_args!("{} bytes", bytes.as_ref().len())).finish(),
			#[cfg(not(target_family = "wasm"))]
			ByteHolder::External { path, bytes } => f.debug_struct("External").field("path", path).field("len", &bytes.as_ref().len()).finish(),
		}
	}
}

/// Owning wrapper around a memory-mapped file that exposes the mapped region as `&[u8]`.
#[cfg(not(target_family = "wasm"))]
pub struct MmappedBytes(mmap_io::mmap::MemoryMappedFile);

#[cfg(not(target_family = "wasm"))]
impl MmappedBytes {
	/// Wrap a mapped file, probing that its region is sliceable so the failure surfaces here rather than
	/// later degrading to the `&[]` fallback in [`AsRef::as_ref`], which cannot return an error.
	pub fn new(file: mmap_io::mmap::MemoryMappedFile) -> Result<Self> {
		let len = file.len();
		file.as_slice(0, len)
			.map_err(|error| ContainerError::Backend(format!("mmap slice of {:?} failed: {error}", file.path())))?;
		Ok(Self(file))
	}

	pub fn path(&self) -> &std::path::Path {
		self.0.path()
	}
}

#[cfg(not(target_family = "wasm"))]
impl AsRef<[u8]> for MmappedBytes {
	fn as_ref(&self) -> &[u8] {
		let len = self.0.len();
		match self.0.as_slice(0, len) {
			Ok(slice) => slice,
			Err(error) => {
				log::error!("Failed to obtain mmap slice: {error}");
				&[]
			}
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
	#[error("path not found: {0}")]
	NotFound(String),

	#[error("invalid path: {0}")]
	InvalidPath(String),

	#[error("not a directory: {0}")]
	NotADirectory(String),

	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),

	/// An archive declared more decompressed data (`declared` bytes) than the codec is willing to
	/// materialize (`limit` bytes).
	#[error("declared decompressed size {declared} exceeds the {limit}-byte limit")]
	SizeLimitExceeded { declared: u64, limit: u64 },

	/// Failure inside an archive codec (zip, lzma, tar). Wraps a foreign error as text since those
	/// error types don't share a common Rust trait we can chain through.
	#[error("codec error: {0}")]
	Codec(String),

	/// Failure inside a storage backend (mmap, OPFS/JS). Wraps a foreign error as text for the same reason.
	#[error("backend error: {0}")]
	Backend(String),
}

pub type Result<T> = std::result::Result<T, ContainerError>;

/// Normalize a listing prefix to end with a trailing slash (unless it names the container root).
/// The root (empty string or `.`) normalizes to the empty string, so backends concatenate child paths
/// as `prefix/child` without a double, missing, or `./`-rooted slash.
pub(crate) fn with_trailing_slash(prefix: &str) -> String {
	if prefix.is_empty() || prefix == "." {
		String::new()
	} else if prefix.ends_with('/') {
		prefix.to_string()
	} else {
		format!("{prefix}/")
	}
}

/// Validate that `path` names a container-safe file: relative, no `.`/`..` segments, no backslashes,
/// no redundant separators (`a//b`, `a/b/`). Dotfile names like `.gitignore` are fine. Requiring a canonical
/// form gives a file one identity across backends, some of which key on the raw string rather than path
/// components. Used by backends to block container escapes and by archive codecs on untrusted entry names.
/// For listing prefixes, which may name the container root, use [`validate_prefix`] instead.
pub fn validate_path(path: &str) -> Result<()> {
	let invalid = || ContainerError::InvalidPath(path.to_string());

	if path.is_empty() || path.contains('\\') || path.starts_with('/') {
		return Err(invalid());
	}

	// `Path::components` silently folds away `//`, trailing `/`, and interior `.` segments, but backends key
	// on the raw string, so a non-canonical path would resolve to one file on a path-joining backend yet a
	// different identity on a string-keyed backend. Reject the redundant segments the component loop below
	// can't see (it never observes a folded-away `CurDir`/empty segment).
	if path.split('/').any(|segment| segment.is_empty() || segment == ".") {
		return Err(invalid());
	}

	// Reject Windows drive-letter prefixes (`C:foo`, `C:/foo`) that platform-agnostic Path doesn't recognize as absolute on Linux.
	let bytes = path.as_bytes();
	if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
		return Err(invalid());
	}

	for component in std::path::Path::new(path).components() {
		use std::path::Component;
		match component {
			Component::Normal(_) => {}
			Component::CurDir | Component::ParentDir | Component::Prefix(_) | Component::RootDir => return Err(invalid()),
		}
	}
	Ok(())
}

/// Validate a listing prefix. Same rules as [`validate_path`], except the container root is also a valid
/// prefix, named by either the empty string or `.`. Backends pass this to `list`/`list_dirs`.
pub fn validate_prefix(prefix: &str) -> Result<()> {
	if prefix.is_empty() || prefix == "." {
		return Ok(());
	}
	validate_path(prefix)
}

/// Synchronous virtual filesystem of named byte payloads.
pub trait Container {
	/// Read the contents of `path` into a [`ByteHolder`].
	fn read(&self, path: &str) -> Result<ByteHolder>;

	/// Write `bytes` at `path`, creating intermediate directories as needed.
	fn write(&self, path: &str, bytes: &[u8]) -> Result<()>;

	/// Append `bytes` to the file at `path`, creating it (and any intermediate directories)
	/// if it does not yet exist. Equivalent to `write` on a fresh path.
	fn append(&self, path: &str, bytes: &[u8]) -> Result<()>;

	/// Write `size` bytes whose contents are produced by `fill`.
	/// The default implementation allocates and forwards to [`Container::write`];
	/// backends that can mmap a writable region may override to fill in place.
	fn write_sized(&self, path: &str, size: usize, fill: &mut dyn FnMut(&mut [u8]) -> Result<()>) -> Result<()> {
		let mut buffer = vec![0; size];
		fill(&mut buffer)?;
		self.write(path, &buffer)
	}

	/// List file entries directly under `prefix` (non-recursive).
	/// Returned paths include the `prefix` (e.g. `list("resources")` returns
	/// `["resources/abc123", ...]`). A missing prefix yields an empty list; a prefix that names a
	/// file (not a directory) is an error.
	fn list(&self, prefix: &str) -> Result<Vec<String>>;

	/// List subdirectory entries directly under `prefix` (non-recursive).
	/// Returned names include the `prefix`, without a trailing slash
	/// (e.g. `list_dirs("")` returns `["resources"]`). Same missing/file-prefix semantics as [`Container::list`].
	fn list_dirs(&self, prefix: &str) -> Result<Vec<String>>;

	/// Whether a file exists at `path`. Directories return `false`.
	fn exists(&self, path: &str) -> bool;

	/// Remove the file at `path`. Idempotent: removing a missing path succeeds. Directories are never
	/// removed (they exist only implicitly as parents of files).
	fn remove(&self, path: &str) -> Result<()>;
}

/// Asynchronous virtual filesystem of named byte payloads. Mirrors [`Container`].
///
/// The returned futures are intentionally not `Send`: native uses `block_on` at the save seam
/// and wasm is single-threaded, so neither needs cross-thread futures. Revisit if we ever want
/// to run container I/O on a thread pool.
#[expect(async_fn_in_trait, reason = "see trait docs — Send is not required")]
pub trait AsyncContainer {
	async fn read(&self, path: &str) -> Result<ByteHolder>;

	async fn write(&self, path: &str, bytes: &[u8]) -> Result<()>;

	async fn append(&self, path: &str, bytes: &[u8]) -> Result<()>;

	async fn write_sized(&self, path: &str, size: usize, fill: &mut dyn FnMut(&mut [u8]) -> Result<()>) -> Result<()>;

	async fn list(&self, prefix: &str) -> Result<Vec<String>>;

	async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>>;

	async fn exists(&self, path: &str) -> bool;

	async fn remove(&self, path: &str) -> Result<()>;

	/// Synchronous write. On backends with sync I/O (folder, memory) the write completes durably
	/// before return and reports real errors. On OPFS the write is enqueued onto a background task
	/// and `Ok` is returned eagerly; a later failure is logged.
	fn write_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()>;

	/// Synchronous append. Same eager-enqueue semantics on OPFS as [`write_non_blocking`](Self::write_non_blocking);
	/// queued appends preserve order relative to earlier queued writes/appends.
	fn append_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()>;

	/// Synchronous remove. Same semantics as [`write_non_blocking`](Self::write_non_blocking).
	fn remove_non_blocking(&self, path: &str) -> Result<()>;

	/// Non-blocking existence check. On OPFS this reads from an in-memory tracking set populated by
	/// the sync write/remove paths, since the underlying OPFS existence API is async only.
	fn exists_non_blocking(&self, path: &str) -> bool;
}

impl<C: Container + ?Sized> AsyncContainer for C {
	async fn read(&self, path: &str) -> Result<ByteHolder> {
		Container::read(self, path)
	}

	async fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
		Container::write(self, path, bytes)
	}

	async fn append(&self, path: &str, bytes: &[u8]) -> Result<()> {
		Container::append(self, path, bytes)
	}

	async fn write_sized(&self, path: &str, size: usize, fill: &mut dyn FnMut(&mut [u8]) -> Result<()>) -> Result<()> {
		Container::write_sized(self, path, size, fill)
	}

	async fn list(&self, prefix: &str) -> Result<Vec<String>> {
		Container::list(self, prefix)
	}

	async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>> {
		Container::list_dirs(self, prefix)
	}

	async fn exists(&self, path: &str) -> bool {
		Container::exists(self, path)
	}

	async fn remove(&self, path: &str) -> Result<()> {
		Container::remove(self, path)
	}

	fn write_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()> {
		Container::write(self, path, bytes)
	}

	fn append_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()> {
		Container::append(self, path, bytes)
	}

	fn remove_non_blocking(&self, path: &str) -> Result<()> {
		Container::remove(self, path)
	}

	fn exists_non_blocking(&self, path: &str) -> bool {
		Container::exists(self, path)
	}
}

/// Type-erased container that dispatches to one of the in-tree backends.
///
/// `AsyncContainer::read` returns `impl Future`, so `dyn AsyncContainer` is not object-safe.
/// `AnyContainer` is the workaround: `Gdd` holds one of these by value, and the `AsyncContainer`
/// impl forwards to the active variant.
pub enum AnyContainer {
	Memory(backends::memory::MemoryBackend),
	#[cfg(not(target_family = "wasm"))]
	Folder(backends::folder::FolderBackend),
	#[cfg(target_family = "wasm")]
	Opfs(backends::opfs::OpfsBackend),
}

impl AsyncContainer for AnyContainer {
	async fn read(&self, path: &str) -> Result<ByteHolder> {
		match self {
			Self::Memory(backend) => AsyncContainer::read(backend, path).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::read(backend, path).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::read(backend, path).await,
		}
	}

	async fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
		match self {
			Self::Memory(backend) => AsyncContainer::write(backend, path, bytes).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::write(backend, path, bytes).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::write(backend, path, bytes).await,
		}
	}

	async fn append(&self, path: &str, bytes: &[u8]) -> Result<()> {
		match self {
			Self::Memory(backend) => AsyncContainer::append(backend, path, bytes).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::append(backend, path, bytes).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::append(backend, path, bytes).await,
		}
	}

	async fn write_sized(&self, path: &str, size: usize, fill: &mut dyn FnMut(&mut [u8]) -> Result<()>) -> Result<()> {
		match self {
			Self::Memory(backend) => AsyncContainer::write_sized(backend, path, size, fill).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::write_sized(backend, path, size, fill).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::write_sized(backend, path, size, fill).await,
		}
	}

	async fn list(&self, prefix: &str) -> Result<Vec<String>> {
		match self {
			Self::Memory(backend) => AsyncContainer::list(backend, prefix).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::list(backend, prefix).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::list(backend, prefix).await,
		}
	}

	async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>> {
		match self {
			Self::Memory(backend) => AsyncContainer::list_dirs(backend, prefix).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::list_dirs(backend, prefix).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::list_dirs(backend, prefix).await,
		}
	}

	async fn exists(&self, path: &str) -> bool {
		match self {
			Self::Memory(backend) => AsyncContainer::exists(backend, path).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::exists(backend, path).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::exists(backend, path).await,
		}
	}

	async fn remove(&self, path: &str) -> Result<()> {
		match self {
			Self::Memory(backend) => AsyncContainer::remove(backend, path).await,
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::remove(backend, path).await,
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::remove(backend, path).await,
		}
	}

	fn write_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()> {
		match self {
			Self::Memory(backend) => AsyncContainer::write_non_blocking(backend, path, bytes),
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::write_non_blocking(backend, path, bytes),
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::write_non_blocking(backend, path, bytes),
		}
	}

	fn append_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()> {
		match self {
			Self::Memory(backend) => AsyncContainer::append_non_blocking(backend, path, bytes),
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::append_non_blocking(backend, path, bytes),
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::append_non_blocking(backend, path, bytes),
		}
	}

	fn remove_non_blocking(&self, path: &str) -> Result<()> {
		match self {
			Self::Memory(backend) => AsyncContainer::remove_non_blocking(backend, path),
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::remove_non_blocking(backend, path),
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::remove_non_blocking(backend, path),
		}
	}

	fn exists_non_blocking(&self, path: &str) -> bool {
		match self {
			Self::Memory(backend) => AsyncContainer::exists_non_blocking(backend, path),
			#[cfg(not(target_family = "wasm"))]
			Self::Folder(backend) => AsyncContainer::exists_non_blocking(backend, path),
			#[cfg(target_family = "wasm")]
			Self::Opfs(backend) => AsyncContainer::exists_non_blocking(backend, path),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{ContainerError, validate_path, validate_prefix};

	#[test]
	fn validate_path_accepts_well_formed() {
		// `..` and `.` are only rejected as whole path components; as substrings of a name they are fine.
		for ok in ["manifest.json", "resources/abc", "a/b/c.bin", "my..file.txt", "..hidden", ".gitignore"] {
			assert!(validate_path(ok).is_ok(), "{ok:?} should be accepted");
		}
	}

	#[test]
	fn validate_path_rejects_unsafe() {
		for bad in ["", "../escape", "a/../b", ".", "./leading", "/abs", "back\\slash", "C:/win", "\\\\?\\unc"] {
			let result = validate_path(bad);
			assert!(matches!(result, Err(ContainerError::InvalidPath(_))), "{bad:?} should be rejected, got {result:?}");
		}
	}

	#[test]
	fn validate_prefix_accepts_root_tokens() {
		// The container root is a valid listing prefix, named by either the empty string or `.`.
		for root in ["", ".", "resources", "a/b"] {
			assert!(validate_prefix(root).is_ok(), "{root:?} should be accepted as a prefix");
		}
		// Unsafe prefixes are still rejected, same as paths.
		assert!(validate_prefix("../escape").is_err());
	}
}
