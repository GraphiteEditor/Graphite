//! Resource I/O on the [`Gdd`] handle: the content-addressed byte store that backs raster images,
//! fonts, embedded WASM, and proto-node declarations. Registration goes through the session as an
//! `AddResource` delta; the bytes live in the working copy's `resources/<hash>` directory.

#[cfg(not(target_family = "wasm"))]
use std::path::Path;
use std::sync::Arc;

use document_container::{AnyContainer, AsyncContainer, ByteHolder, ContainerError};
use graphene_resource::ResourceFuture;
use graphene_resource::{LoadResource, Resource, ResourceHash, ResourceStorage};

use crate::Gdd;
use crate::error::Error;
use crate::layout::Layout;

impl<L: Layout> Gdd<L> {
	pub async fn read_resource(&self, hash: &ResourceHash) -> Result<ByteHolder, ContainerError> {
		self.working.read(&self.layout.resource_path(hash)).await
	}

	/// Register a resource under `id` and store its bytes. Commits an `AddResource` delta (a single
	/// `DataSource::Embedded` source resolved to the content hash) through the session so the registry
	/// records the resource and the entry replicates, then writes the bytes into the working copy's
	/// content-addressed store. The caller owns `id` allocation.
	pub fn add_resource(&mut self, id: graph_storage::ResourceId, bytes: &[u8]) -> Result<(), Error> {
		let hash = ResourceHash::from(bytes);

		self.working.write_non_blocking(&self.layout.resource_path(&hash), bytes)?;

		let hot_ops = self.session.stage_embedded_resource(id, hash)?;
		self.append_and_retire(&hot_ops, false)?;
		Ok(())
	}

	/// Like [`add_resource`](Self::add_resource) but copies the bytes from a filesystem `src` rather
	/// than buffering them. Folder backends use `fs::copy` (CoW on supported filesystems); other
	/// backends fall back to read-then-write. Native-only: there is no filesystem source path on wasm.
	#[cfg(not(target_family = "wasm"))]
	pub fn add_resource_from_path(&mut self, id: graph_storage::ResourceId, hash: ResourceHash, src: &Path) -> Result<(), Error> {
		let dest_path = self.layout.resource_path(&hash);
		if let AnyContainer::Folder(folder) = self.working.as_ref() {
			let full = folder.root().join(&dest_path);
			if let Some(parent) = full.parent() {
				std::fs::create_dir_all(parent).map_err(ContainerError::Io)?;
			}
			std::fs::copy(src, &full).map_err(ContainerError::Io)?;
		} else {
			let bytes = std::fs::read(src).map_err(ContainerError::Io)?;
			// The folder fast path trusts the caller's hash to avoid reading the file; here we've read
			// the bytes anyway, so verify the hash matches and flag a content-addressing bug in debug.
			debug_assert_eq!(hash, ResourceHash::from(bytes.as_slice()), "add_resource_from_path hash does not match the file at {src:?}");
			self.working.write_non_blocking(&dest_path, &bytes)?;
		}

		let hot_ops = self.session.stage_embedded_resource(id, hash)?;
		self.append_and_retire(&hot_ops, false)?;
		Ok(())
	}

	pub async fn has_resource(&self, hash: &ResourceHash) -> bool {
		self.working.exists(&self.layout.resource_path(hash)).await
	}

	pub fn remove_resource(&self, hash: &ResourceHash) -> Result<(), ContainerError> {
		self.working.remove_non_blocking(&self.layout.resource_path(hash))
	}

	pub fn resource_proxy(&self) -> ResourceProxy<L>
	where
		L: Clone,
	{
		ResourceProxy(self.working.clone(), self.layout.clone())
	}

	/// Enumerate every resource currently in the working copy. Paths that don't parse as a
	/// `ResourceHash` (foreign files dropped into the resources directory) are silently skipped.
	pub async fn resource_hashes(&self) -> Result<Vec<ResourceHash>, ContainerError> {
		let dir = self.layout.resources_dir();
		if !self.working.list_dirs("").await?.iter().any(|d| d == dir) {
			return Ok(Vec::new());
		}
		let entries = self.working.list(dir).await?;
		let prefix = format!("{dir}/");
		let mut hashes = Vec::with_capacity(entries.len());
		for entry in entries {
			let Some(name) = entry.strip_prefix(&prefix) else { continue };
			if let Ok(hash) = name.parse::<ResourceHash>() {
				hashes.push(hash);
			}
		}
		Ok(hashes)
	}
}

impl<L: Layout + Send + Sync> LoadResource for Gdd<L> {
	fn load(&self, hash: ResourceHash) -> ResourceFuture<'_> {
		Box::pin(async move {
			let bytes = self.working.read(&self.layout.resource_path(&hash)).await.ok()?;
			Some(Resource::new(bytes))
		})
	}
}

pub struct ResourceProxy<T: Layout>(Arc<AnyContainer>, T);

impl<L: Layout + Send + Sync> LoadResource for ResourceProxy<L> {
	fn load(&self, hash: ResourceHash) -> ResourceFuture<'_> {
		Box::pin(async move {
			let bytes = self.0.read(&self.1.resource_path(&hash)).await.ok()?;
			Some(Resource::new(bytes))
		})
	}
}

impl<L: Layout + Send + Sync> ResourceStorage for Gdd<L> {
	fn store(&self, data: &[u8]) -> ResourceHash {
		let hash = ResourceHash::from(data);
		if let Err(error) = self.working.write_non_blocking(&self.layout.resource_path(&hash), data) {
			log::error!("ResourceStorage::store failed for {hash}: {error}");
		}
		hash
	}

	fn contains(&self, hash: &ResourceHash) -> bool {
		self.working.exists_non_blocking(&self.layout.resource_path(hash))
	}

	fn garbage_collect(&self, used: &[ResourceHash]) {
		// `garbage_collect` is synchronous but listing resources is async, so the native path blocks on
		// it. That's unavailable on wasm (single-threaded; `block_on` would deadlock). The editor never
		// uses `Gdd` as the runtime `ResourceStorage` on wasm (it GCs the app-global cache instead), so
		// this is an unreachable configuration there rather than a missing feature.
		#[cfg(target_family = "wasm")]
		{
			let _ = used;
			log::error!("ResourceStorage::garbage_collect is not supported for Gdd on wasm");
		}
		#[cfg(not(target_family = "wasm"))]
		{
			let kept: std::collections::HashSet<&ResourceHash> = used.iter().collect();
			let hashes = match futures::executor::block_on(self.resource_hashes()) {
				Ok(hashes) => hashes,
				Err(error) => {
					log::error!("Failed to list resources during garbage_collect: {error}");
					return;
				}
			};
			for hash in hashes {
				if kept.contains(&hash) {
					continue;
				}
				if let Err(error) = self.working.remove_non_blocking(&self.layout.resource_path(&hash)) {
					log::error!("ResourceStorage::garbage_collect failed to remove {hash}: {error}");
				}
			}
		}
	}
}
