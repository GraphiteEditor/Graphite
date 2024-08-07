use crate::metadata::identify::CameraModel;
use build_camera_data::build_camera_data;

pub struct CameraData {
	pub black: u16,
	pub maximum: u16,
	pub camera_to_xyz: [i16; 9],
}

impl CameraData {
	const DEFAULT: CameraData = CameraData {
		black: 0,
		maximum: 0,
		camera_to_xyz: [0; 9],
	};
}

const CAMERA_DATA: [(&str, CameraData); 40] = build_camera_data!();

pub fn camera_to_xyz(camera_model: &CameraModel) -> Option<[f64; 9]> {
	let camera_name = camera_model.make.to_owned() + " " + &camera_model.model;
	CAMERA_DATA
		.iter()
		.find(|(camera_name_substring, _)| camera_name.len() >= camera_name_substring.len() && camera_name[..camera_name_substring.len()] == **camera_name_substring)
		.map(|(_, data)| data.camera_to_xyz.map(|x| (x as f64) / 10000.))
}
