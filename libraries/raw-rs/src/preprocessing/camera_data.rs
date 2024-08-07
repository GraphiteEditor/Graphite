use crate::metadata::identify::CameraModel;
use build_camera_data::build_camera_data;

pub struct CameraData {
	pub black: u16,
	pub maximum: u16,
	pub camera_to_xyz: [i16; 9],
}

impl CameraData {
	const DEFAULT: CameraData = CameraData { black: 0, maximum: 0, camera_to_xyz: [0; 9] };
}

const CAMERA_DATA: [(&str, CameraData); 40] = build_camera_data!();

pub fn camera_to_xyz(camera_model: &CameraModel) -> Option<[f64; 9]> {
	let camera_name = camera_model.make.to_owned() + " " + &camera_model.model;
	CAMERA_DATA
		.iter()
		.find(|(camera_name_substring, _)| camera_name.len() >= camera_name_substring.len() && camera_name[..camera_name_substring.len()] == **camera_name_substring)
		.map(|(_, data)| data.camera_to_xyz.map(|x| (x as f64) / 10000.))
}

/*
pub const OLD_CAMERA_DATA: [(&str, CameraData); 60] = [
	(
		"Sony DSC-F828",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7924, -1910, -777, -8226, 15459, 2998, -1517, 2199, 6818, -7242, 11401, 3481],
		},
	),
	(
		"Sony DSC-R1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8512, -2641, -694, -8042, 15670, 2526, -1821, 2117, 7414, 0, 0, 0],
		},
	),
	(
		"Sony DSC-V3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7511, -2571, -692, -7894, 15088, 3060, -948, 1111, 8128, 0, 0, 0],
		},
	),
	(
		"Sony DSC-RX100M",
		CameraData {
			black: 0,
			maximum: 0, /* M2, M3, M4, and M5 */
			camera_to_xyz: [6596, -2079, -562, -4782, 13016, 1933, -970, 1581, 5181, 0, 0, 0],
		},
	),
	(
		"Sony DSC-RX100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8651, -2754, -1057, -3464, 12207, 1373, -568, 1398, 4434, 0, 0, 0],
		},
	),
	(
		"Sony DSC-RX10M4",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7699, -2566, -629, -2967, 11270, 1928, -378, 1286, 4807, 0, 0, 0],
		},
	),
	(
		"Sony DSC-RX10",
		CameraData {
			black: 0,
			maximum: 0, /* also RX10M2, RX10M3 */
			camera_to_xyz: [6679, -1825, -745, -5047, 13256, 1953, -1580, 2422, 5183, 0, 0, 0],
		},
	),
	(
		"Sony DSC-RX1RM2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6629, -1900, -483, -4618, 12349, 2550, -622, 1381, 6514, 0, 0, 0],
		},
	),
	(
		"Sony DSC-RX1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6344, -1612, -462, -4863, 12477, 2681, -865, 1786, 6899, 0, 0, 0],
		},
	),
	(
		"Sony DSC-RX0",
		CameraData {
			black: 200,
			maximum: 0,
			camera_to_xyz: [9396, -3507, -843, -2497, 11111, 1572, -343, 1355, 5089, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A100",
		CameraData {
			black: 0,
			maximum: 0xfeb,
			camera_to_xyz: [9437, -2811, -774, -8405, 16215, 2290, -710, 596, 7181, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A290",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6038, -1484, -579, -9145, 16746, 2512, -875, 746, 7218, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9847, -3091, -928, -8485, 16345, 2225, -715, 595, 7103, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A300",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9847, -3091, -928, -8485, 16345, 2225, -715, 595, 7103, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A330",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9847, -3091, -929, -8485, 16346, 2225, -714, 595, 7103, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A350",
		CameraData {
			black: 0,
			maximum: 0xffc,
			camera_to_xyz: [6038, -1484, -578, -9146, 16746, 2513, -875, 746, 7217, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A380",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6038, -1484, -579, -9145, 16746, 2512, -875, 746, 7218, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A390",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6038, -1484, -579, -9145, 16746, 2512, -875, 746, 7218, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A450",
		CameraData {
			black: 0,
			maximum: 0xfeb,
			camera_to_xyz: [4950, -580, -103, -5228, 12542, 3029, -709, 1435, 7371, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A580",
		CameraData {
			black: 0,
			maximum: 0xfeb,
			camera_to_xyz: [5932, -1492, -411, -4813, 12285, 2856, -741, 1524, 6739, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A500",
		CameraData {
			black: 0,
			maximum: 0xfeb,
			camera_to_xyz: [6046, -1127, -278, -5574, 13076, 2786, -691, 1419, 7625, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A5",
		CameraData {
			black: 0,
			maximum: 0xfeb,
			camera_to_xyz: [4950, -580, -103, -5228, 12542, 3029, -709, 1435, 7371, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A700",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5775, -805, -359, -8574, 16295, 2391, -1943, 2341, 7249, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A850",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5413, -1162, -365, -5665, 13098, 2866, -608, 1179, 8440, 0, 0, 0],
		},
	),
	(
		"Sony DSLR-A900",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5209, -1072, -397, -8845, 16120, 2919, -1618, 1803, 8654, 0, 0, 0],
		},
	),
	(
		"Sony ILCA-68",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6435, -1903, -536, -4722, 12449, 2550, -663, 1363, 6517, 0, 0, 0],
		},
	),
	(
		"Sony ILCA-77M2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5991, -1732, -443, -4100, 11989, 2381, -704, 1467, 5992, 0, 0, 0],
		},
	),
	(
		"Sony ILCA-99M2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6660, -1918, -471, -4613, 12398, 2485, -649, 1433, 6447, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-6",
		CameraData {
			black: 0,
			maximum: 0, /* 6300, 6500 */
			camera_to_xyz: [5973, -1695, -419, -3826, 11797, 2293, -639, 1398, 5789, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7M2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5271, -712, -347, -6153, 13653, 2763, -1601, 2366, 7242, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7M3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7374, -2389, -551, -5435, 13162, 2519, -1006, 1795, 6552, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7S",
		CameraData {
			black: 0,
			maximum: 0, /* also ILCE-7SM2 */
			camera_to_xyz: [5838, -1430, -246, -3497, 11477, 2297, -748, 1885, 5778, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7RM5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8200, -2976, -719, -4296, 12053, 2532, -429, 1282, 5774, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7RM4",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7662, -2686, -660, -5240, 12965, 2530, -796, 1508, 6167, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7RM3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6640, -1847, -503, -5238, 13010, 2474, -993, 1673, 6527, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7RM2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6629, -1900, -483, -4618, 12349, 2550, -622, 1381, 6514, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7R",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [4913, -541, -202, -6130, 13513, 2906, -1564, 2151, 7183, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-7",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5271, -712, -347, -6153, 13653, 2763, -1601, 2366, 7242, 0, 0, 0],
		},
	),
	(
		"Sony ILCE-9",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6389, -1703, -378, -4562, 12265, 2587, -670, 1489, 6550, 0, 0, 0],
		},
	),
	(
		"Sony ILCE",
		CameraData {
			black: 0,
			maximum: 0, /* 3000, 5000, 5100, 6000, and QX1 */
			camera_to_xyz: [5991, -1456, -455, -4764, 12135, 2980, -707, 1425, 6701, 0, 0, 0],
		},
	),
	(
		"Sony NEX-5N",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5991, -1456, -455, -4764, 12135, 2980, -707, 1425, 6701, 0, 0, 0],
		},
	),
	(
		"Sony NEX-5R",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6129, -1545, -418, -4930, 12490, 2743, -977, 1693, 6615, 0, 0, 0],
		},
	),
	(
		"Sony NEX-5T",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6129, -1545, -418, -4930, 12490, 2743, -977, 1693, 6615, 0, 0, 0],
		},
	),
	(
		"Sony NEX-3N",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6129, -1545, -418, -4930, 12490, 2743, -977, 1693, 6615, 0, 0, 0],
		},
	),
	(
		"Sony NEX-3",
		CameraData {
			black: 138,
			maximum: 0, /* DJC */
			camera_to_xyz: [6907, -1256, -645, -4940, 12621, 2320, -1710, 2581, 6230, 0, 0, 0],
		},
	),
	(
		"Sony NEX-5",
		CameraData {
			black: 116,
			maximum: 0, /* DJC */
			camera_to_xyz: [6807, -1350, -342, -4216, 11649, 2567, -1089, 2001, 6420, 0, 0, 0],
		},
	),
	(
		"Sony NEX-3",
		CameraData {
			black: 0,
			maximum: 0, /* Adobe */
			camera_to_xyz: [6549, -1550, -436, -4880, 12435, 2753, -854, 1868, 6976, 0, 0, 0],
		},
	),
	(
		"Sony NEX-5",
		CameraData {
			black: 0,
			maximum: 0, /* Adobe */
			camera_to_xyz: [6549, -1550, -436, -4880, 12435, 2753, -854, 1868, 6976, 0, 0, 0],
		},
	),
	(
		"Sony NEX-6",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6129, -1545, -418, -4930, 12490, 2743, -977, 1693, 6615, 0, 0, 0],
		},
	),
	(
		"Sony NEX-7",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5491, -1192, -363, -4951, 12342, 2948, -911, 1722, 7192, 0, 0, 0],
		},
	),
	(
		"Sony NEX",
		CameraData {
			black: 0,
			maximum: 0, /* NEX-C3, NEX-F3 */
			camera_to_xyz: [5991, -1456, -455, -4764, 12135, 2980, -707, 1425, 6701, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A33",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6069, -1221, -366, -5221, 12779, 2734, -1024, 2066, 6834, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A35",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5986, -1618, -415, -4557, 11820, 3120, -681, 1404, 6971, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A37",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5991, -1456, -455, -4764, 12135, 2980, -707, 1425, 6701, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A55",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5932, -1492, -411, -4813, 12285, 2856, -741, 1524, 6739, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A57",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5991, -1456, -455, -4764, 12135, 2980, -707, 1425, 6701, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A58",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5991, -1456, -455, -4764, 12135, 2980, -707, 1425, 6701, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A65",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5491, -1192, -363, -4951, 12342, 2948, -911, 1722, 7192, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A77",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5491, -1192, -363, -4951, 12342, 2948, -911, 1722, 7192, 0, 0, 0],
		},
	),
	(
		"Sony SLT-A99",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6344, -1612, -462, -4863, 12477, 2681, -865, 1786, 6899, 0, 0, 0],
		},
	),
];
*/
