use std::fs;
use std::io;
use std::path::PathBuf;
use winit::data_transfer::{DataTransferId, DataTransferSend, DataTransferSendBuilder, SendData, TypeHint, TypedData};
use winit::error::TransferError;
use winit::event_loop::{ActiveEventLoop, AsyncRequestSerial};

use crate::wrapper::messages::{DataTransfer, DataTransferItem, DataType, IngestAction, RawData};

fn type_hint(data_type: DataType) -> Option<TypeHint> {
	match data_type {
		DataType::Text => Some(TypeHint::Plaintext),
		DataType::Svg | DataType::Raster(_) => Some(TypeHint::Image {
			extension_hint: Some(data_type.extensions().first().copied()?),
		}),
		_ => None,
	}
}

fn data_type(hint: TypeHint) -> Option<DataType> {
	match hint {
		TypeHint::Plaintext => Some(DataType::Text),
		TypeHint::Image { extension_hint: Some(extension) } => match DataType::from_extension(extension) {
			data_type @ (DataType::Svg | DataType::Raster(_)) => Some(data_type),
			_ => None,
		},
		_ => None,
	}
}

pub(crate) fn send(content: DataTransfer) -> Box<dyn DataTransferSend> {
	let mut builder = DataTransferSendBuilder::new(());
	for item in content.0 {
		let Some((data_type, data)) = item.into_raw() else { continue };
		let Some(hint) = type_hint(data_type) else { continue };
		let data = match (hint, data) {
			(TypeHint::Plaintext, RawData::Text(text)) => SendData::String(text),
			(TypeHint::Plaintext, RawData::Bytes(_)) => continue,
			(_, data) => SendData::Bytes(data.into_bytes()),
		};
		builder.add_type(hint, move |_, _| Some(data.clone()));
	}
	builder.build()
}

pub(crate) fn read_files(paths: impl IntoIterator<Item = PathBuf>) -> Vec<DataTransferItem> {
	paths
		.into_iter()
		.filter_map(|path| match fs::read(&path) {
			Ok(data) => Some(DataTransferItem::from_file(path, None, data)),
			Err(e) => {
				tracing::error!("Failed to read file {}: {e}", path.display());
				None
			}
		})
		.collect()
}

#[derive(Default)]
pub(crate) struct TransferReads(Vec<TransferRead>);

struct TransferRead {
	action: IngestAction,
	fetches: Vec<(AsyncRequestSerial, TypeHint)>,
	items: Vec<DataTransferItem>,
}

impl TransferReads {
	pub(crate) fn start(&mut self, event_loop: &dyn ActiveEventLoop, id: DataTransferId, action: IngestAction) -> Result<(), TransferError> {
		let mut hints = Vec::new();
		for hint in event_loop.data_transfer(id)?.available_types().iter().filter_map(|transfer_type| transfer_type.hint()) {
			if (hint == TypeHint::UriList || data_type(hint).is_some()) && !hints.contains(&hint) {
				hints.push(hint);
			}
		}

		let mut fetches = Vec::new();
		for hint in hints {
			fetches.push((event_loop.fetch_data_transfer(id, &hint)?, hint));
		}

		if !fetches.is_empty() {
			self.0.push(TransferRead { action, fetches, items: Vec::new() });
		}
		Ok(())
	}

	pub(crate) fn receive(&mut self, serial: AsyncRequestSerial, value: &dyn TypedData) -> Option<(DataTransfer, IngestAction)> {
		let (index, fetch) = self
			.0
			.iter()
			.enumerate()
			.find_map(|(index, read)| Some((index, read.fetches.iter().position(|(fetch, _)| *fetch == serial)?)))?;
		let read = &mut self.0[index];
		let hint = read.fetches[fetch].1;

		let received = match hint {
			TypeHint::UriList => value.try_as_uris().map(|uris| {
				let paths = uris.iter().filter_map(|uri| url::Url::parse(uri).ok()?.to_file_path().ok());
				read.items.extend(read_files(paths));
			}),
			TypeHint::Plaintext => value.try_as_string().map(|text| read.items.extend(DataTransferItem::from_raw(DataType::Text, RawData::Text(text)))),
			_ => value
				.try_as_bytes()
				.map(|bytes| read.items.extend(data_type(hint).and_then(|data_type| DataTransferItem::from_raw(data_type, RawData::Bytes(bytes))))),
		};
		match received {
			Err(e) if e.kind() == io::ErrorKind::WouldBlock => return None,
			Err(e) => tracing::error!("Failed to read transferred data: {e}"),
			Ok(()) => {}
		}
		read.fetches.remove(fetch);
		if !read.fetches.is_empty() {
			return None;
		}

		let read = self.0.remove(index);
		Some((DataTransfer(read.items), read.action))
	}
}
