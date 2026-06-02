//! Xz-compressed tarball archive codec.

use crate::archive::{Archive, ArchiveWriter, MAX_DECOMPRESSED_SIZE};
use crate::{Container, ContainerError, Result, validate_path};
use lzma_rust2::{XzOptions, XzReader, XzWriter as InnerXzWriter};
use std::io::{Read, Seek, Write};

pub struct Xz;

/// xz-tar writer. Held as an `Option` so `finish` can take ownership and unwind the layered
/// writers in the right order: drop the tar builder first to flush its trailer, then finish xz.
pub struct XzWriter<W: Write + Seek> {
	tar: Option<tar::Builder<InnerXzWriter<W>>>,
}

impl Archive for Xz {
	type Writer<W: Write + Seek> = XzWriter<W>;

	fn writer<W: Write + Seek>(output: W) -> Result<Self::Writer<W>> {
		let xz_writer = InnerXzWriter::new(output, XzOptions::default()).map_err(lzma_err)?;
		Ok(XzWriter {
			tar: Some(tar::Builder::new(xz_writer)),
		})
	}

	fn deserialize<R: Read + Seek, C: Container>(source: R, dest: &mut C) -> Result<()> {
		// `take` bounds how many bytes we decompress from the xz stream, but each tar entry's declared
		// size is fed to `write_sized`, which pre-allocates from it before reading. Cap the cumulative
		// declared size too so a header claiming a huge size can't trigger a giant allocation up front.
		let xz_reader = XzReader::new(source, false);
		let bounded = xz_reader.take(MAX_DECOMPRESSED_SIZE);

		let mut tar_reader = tar::Archive::new(bounded);
		let mut total_size = 0u64;

		for entry in tar_reader.entries()? {
			let mut entry = entry?;
			if entry.header().entry_type() != tar::EntryType::Regular {
				continue;
			}
			let path = entry.path()?.to_string_lossy().into_owned();
			validate_path(&path)?;

			let size = entry.size();
			total_size = total_size.saturating_add(size);
			if total_size >= MAX_DECOMPRESSED_SIZE {
				return Err(ContainerError::SizeLimitExceeded {
					declared: total_size,
					limit: MAX_DECOMPRESSED_SIZE,
				});
			}

			dest.write_sized(&path, size as usize, &mut |buffer| {
				entry.read_exact(buffer).map_err(ContainerError::Io)?;
				Ok(())
			})?;
		}

		Ok(())
	}
}

impl<W: Write + Seek> ArchiveWriter for XzWriter<W> {
	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		let tar = self.tar.as_mut().ok_or_else(|| ContainerError::Codec("XzWriter already finished".into()))?;
		let mut header = tar::Header::new_gnu();
		header.set_path(path).map_err(|error| ContainerError::Codec(format!("tar: invalid path {path}: {error}")))?;
		header.set_size(bytes.len() as u64);
		header.set_mode(0o644);
		header.set_cksum();
		tar.append(&header, bytes)?;
		Ok(())
	}

	fn finish(mut self) -> Result<()> {
		let mut tar = self.tar.take().ok_or_else(|| ContainerError::Codec("XzWriter already finished".into()))?;
		tar.finish()?;
		let xz_writer = tar.into_inner()?;
		xz_writer.finish().map_err(lzma_err)?;
		Ok(())
	}
}

fn lzma_err(error: std::io::Error) -> ContainerError {
	ContainerError::Codec(format!("lzma: {error}"))
}
