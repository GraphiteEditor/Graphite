use crate::tiff::file::TiffRead;
use crate::tiff::tags::{Make, Model, Tag};
use crate::tiff::{Ifd, TiffError};
use rawkit_proc_macros::Tag;
use std::io::{Read, Seek};

const COMPANY_NAMES: [&str; 22] = [
	"AgfaPhoto",
	"Canon",
	"Casio",
	"Epson",
	"Fujifilm",
	"Mamiya",
	"Minolta",
	"Motorola",
	"Kodak",
	"Konica",
	"Leica",
	"Nikon",
	"Nokia",
	"Olympus",
	"Ricoh",
	"Pentax",
	"Phase One",
	"Samsung",
	"Sigma",
	"Sinar",
	"Sony",
	"YI",
];

#[allow(dead_code)]
#[derive(Tag)]
struct CameraModelIfd {
	make: Make,
	model: Model,
}

pub struct CameraModel {
	pub make: String,
	pub model: String,
}

pub fn identify_camera_model<R: Read + Seek>(ifd: &Ifd, file: &mut TiffRead<R>) -> Option<CameraModel> {
	let mut ifd = ifd.get_value::<CameraModelIfd, _>(file).unwrap();

	ifd.make.make_ascii_lowercase();
	for company_name in COMPANY_NAMES {
		let lowercase_company_name = company_name.to_ascii_lowercase();
		if ifd.make.contains(&lowercase_company_name) {
			return Some(CameraModel {
				make: company_name.to_string(),
				model: ifd.model,
			});
		}
	}

	None
}
