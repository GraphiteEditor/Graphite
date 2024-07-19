use crate::metadata::identify::CameraModel;

pub struct CameraData {
	black: u16,
	maximum: u16,
	camera_to_xyz: [i16; 12],
}

pub fn camera_to_xyz(camera_model: &CameraModel) -> Option<[f64; 12]> {
	let camera_name = camera_model.make.to_owned() + " " + &camera_model.model;
	CAMERA_DATA
		.iter()
		.find(|(camera_name_substring, _)| camera_name.len() >= camera_name_substring.len() && camera_name[..camera_name_substring.len()] == **camera_name_substring)
		.map(|(_, data)| data.camera_to_xyz.map(|x| (x as f64) / 10000.))
}

pub const CAMERA_DATA: [(&str, CameraData); 573] = [
	(
		"AgfaPhoto DC-833m",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [11438, -3762, -1115, -2409, 9914, 2497, -1227, 2295, 5300, 0, 0, 0],
		},
	),
	(
		"Apple QuickTake",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [21392, -5653, -3353, 2406, 8010, -415, 7166, 1427, 2078, 0, 0, 0],
		},
	),
	(
		"Canon EOS D2000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [24542, -10860, -3401, -1490, 11370, -297, 2858, -605, 3225, 0, 0, 0],
		},
	),
	(
		"Canon EOS D6000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [20482, -7172, -3125, -1033, 10410, -285, 2542, 226, 3136, 0, 0, 0],
		},
	),
	(
		"Canon EOS D30",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9805, -2689, -1312, -5803, 13064, 3068, -2438, 3075, 8775, 0, 0, 0],
		},
	),
	(
		"Canon EOS D60",
		CameraData {
			black: 0,
			maximum: 0xfa0,
			camera_to_xyz: [6188, -1341, -890, -7168, 14489, 2937, -2640, 3228, 8483, 0, 0, 0],
		},
	),
	(
		"Canon EOS 5DS",
		CameraData {
			black: 0,
			maximum: 0x3c96,
			camera_to_xyz: [6250, -711, -808, -5153, 12794, 2636, -1249, 2198, 5610, 0, 0, 0],
		},
	),
	(
		"Canon EOS 5D Mark IV",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6446, -366, -864, -4436, 12204, 2513, -952, 2496, 6348, 0, 0, 0],
		},
	),
	(
		"Canon EOS 5D Mark III",
		CameraData {
			black: 0,
			maximum: 0x3c80,
			camera_to_xyz: [6722, -635, -963, -4287, 12460, 2028, -908, 2162, 5668, 0, 0, 0],
		},
	),
	(
		"Canon EOS 5D Mark II",
		CameraData {
			black: 0,
			maximum: 0x3cf0,
			camera_to_xyz: [4716, 603, -830, -7798, 15474, 2480, -1496, 1937, 6651, 0, 0, 0],
		},
	),
	(
		"Canon EOS 5D",
		CameraData {
			black: 0,
			maximum: 0xe6c,
			camera_to_xyz: [6347, -479, -972, -8297, 15954, 2480, -1968, 2131, 7649, 0, 0, 0],
		},
	),
	(
		"Canon EOS 6D Mark II",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6875, -970, -932, -4691, 12459, 2501, -874, 1953, 5809, 0, 0, 0],
		},
	),
	(
		"Canon EOS 6D",
		CameraData {
			black: 0,
			maximum: 0x3c82,
			camera_to_xyz: [7034, -804, -1014, -4420, 12564, 2058, -851, 1994, 5758, 0, 0, 0],
		},
	),
	(
		"Canon EOS 7D Mark II",
		CameraData {
			black: 0,
			maximum: 0x3510,
			camera_to_xyz: [7268, -1082, -969, -4186, 11839, 2663, -825, 2029, 5839, 0, 0, 0],
		},
	),
	(
		"Canon EOS 7D",
		CameraData {
			black: 0,
			maximum: 0x3510,
			camera_to_xyz: [6844, -996, -856, -3876, 11761, 2396, -593, 1772, 6198, 0, 0, 0],
		},
	),
	(
		"Canon EOS 10D",
		CameraData {
			black: 0,
			maximum: 0xfa0,
			camera_to_xyz: [8197, -2000, -1118, -6714, 14335, 2592, -2536, 3178, 8266, 0, 0, 0],
		},
	),
	(
		"Canon EOS 20Da",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [14155, -5065, -1382, -6550, 14633, 2039, -1623, 1824, 6561, 0, 0, 0],
		},
	),
	(
		"Canon EOS 20D",
		CameraData {
			black: 0,
			maximum: 0xfff,
			camera_to_xyz: [6599, -537, -891, -8071, 15783, 2424, -1983, 2234, 7462, 0, 0, 0],
		},
	),
	(
		"Canon EOS 30D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6257, -303, -1000, -7880, 15621, 2396, -1714, 1904, 7046, 0, 0, 0],
		},
	),
	(
		"Canon EOS 40D",
		CameraData {
			black: 0,
			maximum: 0x3f60,
			camera_to_xyz: [6071, -747, -856, -7653, 15365, 2441, -2025, 2553, 7315, 0, 0, 0],
		},
	),
	(
		"Canon EOS 50D",
		CameraData {
			black: 0,
			maximum: 0x3d93,
			camera_to_xyz: [4920, 616, -593, -6493, 13964, 2784, -1774, 3178, 7005, 0, 0, 0],
		},
	),
	(
		"Canon EOS 60D",
		CameraData {
			black: 0,
			maximum: 0x2ff7,
			camera_to_xyz: [6719, -994, -925, -4408, 12426, 2211, -887, 2129, 6051, 0, 0, 0],
		},
	),
	(
		"Canon EOS 70D",
		CameraData {
			black: 0,
			maximum: 0x3bc7,
			camera_to_xyz: [7034, -804, -1014, -4420, 12564, 2058, -851, 1994, 5758, 0, 0, 0],
		},
	),
	(
		"Canon EOS 77D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7377, -742, -998, -4235, 11981, 2549, -673, 1918, 5538, 0, 0, 0],
		},
	),
	(
		"Canon EOS 80D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7457, -671, -937, -4849, 12495, 2643, -1213, 2354, 5492, 0, 0, 0],
		},
	),
	(
		"Canon EOS 100D",
		CameraData {
			black: 0,
			maximum: 0x350f,
			camera_to_xyz: [6602, -841, -939, -4472, 12458, 2247, -975, 2039, 6148, 0, 0, 0],
		},
	),
	(
		"Canon EOS 200D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7377, -742, -998, -4235, 11981, 2549, -673, 1918, 5538, 0, 0, 0],
		},
	),
	(
		"Canon EOS 300D",
		CameraData {
			black: 0,
			maximum: 0xfa0,
			camera_to_xyz: [8197, -2000, -1118, -6714, 14335, 2592, -2536, 3178, 8266, 0, 0, 0],
		},
	),
	(
		"Canon EOS 350D",
		CameraData {
			black: 0,
			maximum: 0xfff,
			camera_to_xyz: [6018, -617, -965, -8645, 15881, 2975, -1530, 1719, 7642, 0, 0, 0],
		},
	),
	(
		"Canon EOS 400D",
		CameraData {
			black: 0,
			maximum: 0xe8e,
			camera_to_xyz: [7054, -1501, -990, -8156, 15544, 2812, -1278, 1414, 7796, 0, 0, 0],
		},
	),
	(
		"Canon EOS 450D",
		CameraData {
			black: 0,
			maximum: 0x390d,
			camera_to_xyz: [5784, -262, -821, -7539, 15064, 2672, -1982, 2681, 7427, 0, 0, 0],
		},
	),
	(
		"Canon EOS 500D",
		CameraData {
			black: 0,
			maximum: 0x3479,
			camera_to_xyz: [4763, 712, -646, -6821, 14399, 2640, -1921, 3276, 6561, 0, 0, 0],
		},
	),
	(
		"Canon EOS 550D",
		CameraData {
			black: 0,
			maximum: 0x3dd7,
			camera_to_xyz: [6941, -1164, -857, -3825, 11597, 2534, -416, 1540, 6039, 0, 0, 0],
		},
	),
	(
		"Canon EOS 600D",
		CameraData {
			black: 0,
			maximum: 0x3510,
			camera_to_xyz: [6461, -907, -882, -4300, 12184, 2378, -819, 1944, 5931, 0, 0, 0],
		},
	),
	(
		"Canon EOS 650D",
		CameraData {
			black: 0,
			maximum: 0x354d,
			camera_to_xyz: [6602, -841, -939, -4472, 12458, 2247, -975, 2039, 6148, 0, 0, 0],
		},
	),
	(
		"Canon EOS 700D",
		CameraData {
			black: 0,
			maximum: 0x3c00,
			camera_to_xyz: [6602, -841, -939, -4472, 12458, 2247, -975, 2039, 6148, 0, 0, 0],
		},
	),
	(
		"Canon EOS 750D",
		CameraData {
			black: 0,
			maximum: 0x368e,
			camera_to_xyz: [6362, -823, -847, -4426, 12109, 2616, -743, 1857, 5635, 0, 0, 0],
		},
	),
	(
		"Canon EOS 760D",
		CameraData {
			black: 0,
			maximum: 0x350f,
			camera_to_xyz: [6362, -823, -847, -4426, 12109, 2616, -743, 1857, 5635, 0, 0, 0],
		},
	),
	(
		"Canon EOS 800D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6970, -512, -968, -4425, 12161, 2553, -739, 1982, 5601, 0, 0, 0],
		},
	),
	(
		"Canon EOS 1000D",
		CameraData {
			black: 0,
			maximum: 0xe43,
			camera_to_xyz: [6771, -1139, -977, -7818, 15123, 2928, -1244, 1437, 7533, 0, 0, 0],
		},
	),
	(
		"Canon EOS 1100D",
		CameraData {
			black: 0,
			maximum: 0x3510,
			camera_to_xyz: [6444, -904, -893, -4563, 12308, 2535, -903, 2016, 6728, 0, 0, 0],
		},
	),
	(
		"Canon EOS 1200D",
		CameraData {
			black: 0,
			maximum: 0x37c2,
			camera_to_xyz: [6461, -907, -882, -4300, 12184, 2378, -819, 1944, 5931, 0, 0, 0],
		},
	),
	(
		"Canon EOS 1300D",
		CameraData {
			black: 0,
			maximum: 0x3510,
			camera_to_xyz: [6939, -1016, -866, -4428, 12473, 2177, -1175, 2178, 6162, 0, 0, 0],
		},
	),
	(
		"Canon EOS 1500D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8532, -701, -1167, -4095, 11879, 2508, -797, 2424, 7010, 0, 0, 0],
		},
	),
	(
		"Canon EOS 3000D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6939, -1016, -866, -4428, 12473, 2177, -1175, 2178, 6162, 0, 0, 0],
		},
	),
	(
		"Canon EOS M6",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8532, -701, -1167, -4095, 11879, 2508, -797, 2424, 7010, 0, 0, 0],
		},
	),
	(
		"Canon EOS M5",
		CameraData {
			black: 0,
			maximum: 0, /* also M50 */
			camera_to_xyz: [8532, -701, -1167, -4095, 11879, 2508, -797, 2424, 7010, 0, 0, 0],
		},
	),
	(
		"Canon EOS M3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6362, -823, -847, -4426, 12109, 2616, -743, 1857, 5635, 0, 0, 0],
		},
	),
	(
		"Canon EOS M100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8532, -701, -1167, -4095, 11879, 2508, -797, 2424, 7010, 0, 0, 0],
		},
	),
	(
		"Canon EOS M10",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6400, -480, -888, -5294, 13416, 2047, -1296, 2203, 6137, 0, 0, 0],
		},
	),
	(
		"Canon EOS M",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6602, -841, -939, -4472, 12458, 2247, -975, 2039, 6148, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1Ds Mark III",
		CameraData {
			black: 0,
			maximum: 0x3bb0,
			camera_to_xyz: [5859, -211, -930, -8255, 16017, 2353, -1732, 1887, 7448, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1Ds Mark II",
		CameraData {
			black: 0,
			maximum: 0xe80,
			camera_to_xyz: [6517, -602, -867, -8180, 15926, 2378, -1618, 1771, 7633, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D Mark IV",
		CameraData {
			black: 0,
			maximum: 0x3bb0,
			camera_to_xyz: [6014, -220, -795, -4109, 12014, 2361, -561, 1824, 5787, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D Mark III",
		CameraData {
			black: 0,
			maximum: 0x3bb0,
			camera_to_xyz: [6291, -540, -976, -8350, 16145, 2311, -1714, 1858, 7326, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D Mark II N",
		CameraData {
			black: 0,
			maximum: 0xe80,
			camera_to_xyz: [6240, -466, -822, -8180, 15825, 2500, -1801, 1938, 8042, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D Mark II",
		CameraData {
			black: 0,
			maximum: 0xe80,
			camera_to_xyz: [6264, -582, -724, -8312, 15948, 2504, -1744, 1919, 8664, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1DS",
		CameraData {
			black: 0,
			maximum: 0xe20,
			camera_to_xyz: [4374, 3631, -1743, -7520, 15212, 2472, -2892, 3632, 8161, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D C",
		CameraData {
			black: 0,
			maximum: 0x3c4e,
			camera_to_xyz: [6847, -614, -1014, -4669, 12737, 2139, -1197, 2488, 6846, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D X Mark II",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7596, -978, -967, -4808, 12571, 2503, -1398, 2567, 5752, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D X",
		CameraData {
			black: 0,
			maximum: 0x3c4e,
			camera_to_xyz: [6847, -614, -1014, -4669, 12737, 2139, -1197, 2488, 6846, 0, 0, 0],
		},
	),
	(
		"Canon EOS-1D",
		CameraData {
			black: 0,
			maximum: 0xe20,
			camera_to_xyz: [6806, -179, -1020, -8097, 16415, 1687, -3267, 4236, 7690, 0, 0, 0],
		},
	),
	(
		"Canon EOS C500",
		CameraData {
			black: 853,
			maximum: 0, /* DJC */
			camera_to_xyz: [17851, -10604, 922, -7425, 16662, 763, -3660, 3636, 22278, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A530",
		CameraData {
			black: 0,
			maximum: 0,                                          /* DJC */
			camera_to_xyz: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], /* don't want the A5 matrix */
		},
	),
	(
		"Canon PowerShot A50",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-5300, 9846, 1776, 3436, 684, 3939, -5540, 9879, 6200, -1404, 11175, 217],
		},
	),
	(
		"Canon PowerShot A5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-4801, 9475, 1952, 2926, 1611, 4094, -5259, 10164, 5947, -1554, 10883, 547],
		},
	),
	(
		"Canon PowerShot G10",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11093, -3906, -1028, -5047, 12492, 2879, -1003, 1750, 5561, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G11",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12177, -4817, -1069, -1612, 9864, 2049, -98, 850, 4471, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G12",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13244, -5501, -1248, -1508, 9858, 1935, -270, 1083, 4366, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G15",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7474, -2301, -567, -4056, 11456, 2975, -222, 716, 4181, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G16",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8020, -2687, -682, -3704, 11879, 2052, -965, 1921, 5556, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G1 X Mark III",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8532, -701, -1167, -4095, 11879, 2508, -797, 2424, 7010, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G1 X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7378, -1255, -1043, -4088, 12251, 2048, -876, 1946, 5805, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-4778, 9467, 2172, 4743, -1141, 4344, -5146, 9908, 6077, -1566, 11051, 557],
		},
	),
	(
		"Canon PowerShot G2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9087, -2693, -1049, -6715, 14382, 2537, -2291, 2819, 7790, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G3 X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9701, -3857, -921, -3149, 11537, 1817, -786, 1817, 5147, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9212, -2781, -1073, -6573, 14189, 2605, -2300, 2844, 7664, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G5 X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9602, -3823, -937, -2984, 11495, 1675, -407, 1415, 5049, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9757, -2872, -933, -5972, 13861, 2301, -1622, 2328, 7212, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G6",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9877, -3775, -871, -7613, 14807, 3072, -1448, 1305, 7485, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G7 X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9602, -3823, -937, -2984, 11495, 1675, -407, 1415, 5049, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G9 X Mark II",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10056, -4131, -944, -2576, 11143, 1625, -238, 1294, 5179, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G9 X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9602, -3823, -937, -2984, 11495, 1675, -407, 1415, 5049, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot G9",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7368, -2141, -598, -5621, 13254, 2625, -1418, 1696, 5743, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot Pro1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10062, -3522, -999, -7643, 15117, 2730, -765, 817, 7323, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot Pro70",
		CameraData {
			black: 34,
			maximum: 0,
			camera_to_xyz: [-4155, 9818, 1529, 3939, -25, 4522, -5521, 9870, 6610, -2238, 10873, 1342],
		},
	),
	(
		"Canon PowerShot Pro90",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-4963, 9896, 2235, 4642, -987, 4294, -5162, 10011, 5859, -1770, 11230, 577],
		},
	),
	(
		"Canon PowerShot S30",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10566, -3652, -1129, -6552, 14662, 2006, -2197, 2581, 7670, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S40",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8510, -2487, -940, -6869, 14231, 2900, -2318, 2829, 9013, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S45",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8163, -2333, -955, -6682, 14174, 2751, -2077, 2597, 8041, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S50",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8882, -2571, -863, -6348, 14234, 2288, -1516, 2172, 6569, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S60",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8795, -2482, -797, -7804, 15403, 2573, -1422, 1996, 7082, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S70",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9976, -3810, -832, -7115, 14463, 2906, -901, 989, 7889, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S90",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12374, -5016, -1049, -1677, 9902, 2078, -83, 852, 4683, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S95",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13440, -5896, -1279, -1236, 9598, 1931, -180, 1001, 4651, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7968, -2565, -636, -2873, 10697, 2513, 180, 667, 4211, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S110",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8039, -2643, -654, -3783, 11230, 2930, -206, 690, 4194, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S120",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6961, -1685, -695, -4625, 12945, 1836, -1114, 2152, 5518, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot SX1 IS",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6578, -259, -502, -5974, 13030, 3309, -308, 1058, 4970, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot SX50 HS",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12432, -4753, -1247, -2110, 10691, 1629, -412, 1623, 4926, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot SX60 HS",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13161, -5451, -1344, -1989, 10654, 1531, -47, 1271, 4955, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A3300",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [10826, -3654, -1023, -3215, 11310, 1906, 0, 999, 4960, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A470",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [12513, -4407, -1242, -2680, 10276, 2405, -878, 2215, 4734, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A610",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [15591, -6402, -1592, -5365, 13198, 2168, -1300, 1824, 5075, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A620",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [15265, -6193, -1558, -4125, 12116, 2010, -888, 1639, 5220, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A630",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [14201, -5308, -1757, -6087, 14472, 1617, -2191, 3105, 5348, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A640",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [13124, -5329, -1390, -3602, 11658, 1944, -1612, 2863, 4885, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A650",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [9427, -3036, -959, -2581, 10671, 1911, -1039, 1982, 4430, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot A720",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [14573, -5482, -1546, -1266, 9799, 1468, -1040, 1912, 3810, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot S3 IS",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [14062, -5199, -1446, -4712, 12470, 2243, -1286, 2028, 4836, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot SX110 IS",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [14134, -5576, -1527, -1991, 10719, 1273, -1158, 1929, 3581, 0, 0, 0],
		},
	),
	(
		"Canon PowerShot SX220",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [13898, -5076, -1447, -1405, 10109, 1297, -244, 1860, 3687, 0, 0, 0],
		},
	),
	(
		"Canon IXUS 160",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [11657, -3781, -1136, -3544, 11262, 2283, -160, 1219, 4700, 0, 0, 0],
		},
	),
	(
		"Casio EX-S20",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [11634, -3924, -1128, -4968, 12954, 2015, -1588, 2648, 7206, 0, 0, 0],
		},
	),
	(
		"Casio EX-Z750",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [10819, -3873, -1099, -4903, 13730, 1175, -1755, 3751, 4632, 0, 0, 0],
		},
	),
	(
		"Casio EX-Z10",
		CameraData {
			black: 128,
			maximum: 0xfff, /* DJC */
			camera_to_xyz: [9790, -3338, -603, -2321, 10222, 2099, -344, 1273, 4799, 0, 0, 0],
		},
	),
	(
		"CINE 650",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [3390, 480, -500, -800, 3610, 340, -550, 2336, 1192, 0, 0, 0],
		},
	),
	(
		"CINE 660",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [3390, 480, -500, -800, 3610, 340, -550, 2336, 1192, 0, 0, 0],
		},
	),
	(
		"CINE",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [20183, -4295, -423, -3940, 15330, 3985, -280, 4870, 9800, 0, 0, 0],
		},
	),
	(
		"Contax N Digital",
		CameraData {
			black: 0,
			maximum: 0xf1e,
			camera_to_xyz: [7777, 1285, -1053, -9280, 16543, 2916, -3677, 5679, 7060, 0, 0, 0],
		},
	),
	(
		"DXO ONE",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6596, -2079, -562, -4782, 13016, 1933, -970, 1581, 5181, 0, 0, 0],
		},
	),
	(
		"Epson R-D1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6827, -1878, -732, -8429, 16012, 2564, -704, 592, 7145, 0, 0, 0],
		},
	),
	(
		"Fujifilm E550",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11044, -3888, -1120, -7248, 15168, 2208, -1531, 2277, 8069, 0, 0, 0],
		},
	),
	(
		"Fujifilm E900",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9183, -2526, -1078, -7461, 15071, 2574, -2022, 2440, 8639, 0, 0, 0],
		},
	),
	(
		"Fujifilm F5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13690, -5358, -1474, -3369, 11600, 1998, -132, 1554, 4395, 0, 0, 0],
		},
	),
	(
		"Fujifilm F6",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13690, -5358, -1474, -3369, 11600, 1998, -132, 1554, 4395, 0, 0, 0],
		},
	),
	(
		"Fujifilm F77",
		CameraData {
			black: 0,
			maximum: 0xfe9,
			camera_to_xyz: [13690, -5358, -1474, -3369, 11600, 1998, -132, 1554, 4395, 0, 0, 0],
		},
	),
	(
		"Fujifilm F7",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10004, -3219, -1201, -7036, 15047, 2107, -1863, 2565, 7736, 0, 0, 0],
		},
	),
	(
		"Fujifilm F8",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13690, -5358, -1474, -3369, 11600, 1998, -132, 1554, 4395, 0, 0, 0],
		},
	),
	(
		"Fujifilm GFX 50S",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11756, -4754, -874, -3056, 11045, 2305, -381, 1457, 6006, 0, 0, 0],
		},
	),
	(
		"Fujifilm S100FS",
		CameraData {
			black: 514,
			maximum: 0,
			camera_to_xyz: [11521, -4355, -1065, -6524, 13767, 3058, -1466, 1984, 6045, 0, 0, 0],
		},
	),
	(
		"Fujifilm S1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12297, -4882, -1202, -2106, 10691, 1623, -88, 1312, 4790, 0, 0, 0],
		},
	),
	(
		"Fujifilm S20Pro",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10004, -3219, -1201, -7036, 15047, 2107, -1863, 2565, 7736, 0, 0, 0],
		},
	),
	(
		"Fujifilm S20",
		CameraData {
			black: 512,
			maximum: 0x3fff,
			camera_to_xyz: [11401, -4498, -1312, -5088, 12751, 2613, -838, 1568, 5941, 0, 0, 0],
		},
	),
	(
		"Fujifilm S2Pro",
		CameraData {
			black: 128,
			maximum: 0xf15,
			camera_to_xyz: [12492, -4690, -1402, -7033, 15423, 1647, -1507, 2111, 7697, 0, 0, 0],
		},
	),
	(
		"Fujifilm S3Pro",
		CameraData {
			black: 0,
			maximum: 0x3dff,
			camera_to_xyz: [11807, -4612, -1294, -8927, 16968, 1988, -2120, 2741, 8006, 0, 0, 0],
		},
	),
	(
		"Fujifilm S5Pro",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12300, -5110, -1304, -9117, 17143, 1998, -1947, 2448, 8100, 0, 0, 0],
		},
	),
	(
		"Fujifilm S5000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8754, -2732, -1019, -7204, 15069, 2276, -1702, 2334, 6982, 0, 0, 0],
		},
	),
	(
		"Fujifilm S5100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11940, -4431, -1255, -6766, 14428, 2542, -993, 1165, 7421, 0, 0, 0],
		},
	),
	(
		"Fujifilm S5500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11940, -4431, -1255, -6766, 14428, 2542, -993, 1165, 7421, 0, 0, 0],
		},
	),
	(
		"Fujifilm S5200",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9636, -2804, -988, -7442, 15040, 2589, -1803, 2311, 8621, 0, 0, 0],
		},
	),
	(
		"Fujifilm S5600",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9636, -2804, -988, -7442, 15040, 2589, -1803, 2311, 8621, 0, 0, 0],
		},
	),
	(
		"Fujifilm S6",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12628, -4887, -1401, -6861, 14996, 1962, -2198, 2782, 7091, 0, 0, 0],
		},
	),
	(
		"Fujifilm S7000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10190, -3506, -1312, -7153, 15051, 2238, -2003, 2399, 7505, 0, 0, 0],
		},
	),
	(
		"Fujifilm S9000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10491, -3423, -1145, -7385, 15027, 2538, -1809, 2275, 8692, 0, 0, 0],
		},
	),
	(
		"Fujifilm S9500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10491, -3423, -1145, -7385, 15027, 2538, -1809, 2275, 8692, 0, 0, 0],
		},
	),
	(
		"Fujifilm S9100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12343, -4515, -1285, -7165, 14899, 2435, -1895, 2496, 8800, 0, 0, 0],
		},
	),
	(
		"Fujifilm S9600",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12343, -4515, -1285, -7165, 14899, 2435, -1895, 2496, 8800, 0, 0, 0],
		},
	),
	(
		"Fujifilm SL1000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11705, -4262, -1107, -2282, 10791, 1709, -555, 1713, 4945, 0, 0, 0],
		},
	),
	(
		"Fujifilm IS-1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [21461, -10807, -1441, -2332, 10599, 1999, 289, 875, 7703, 0, 0, 0],
		},
	),
	(
		"Fujifilm IS Pro",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12300, -5110, -1304, -9117, 17143, 1998, -1947, 2448, 8100, 0, 0, 0],
		},
	),
	(
		"Fujifilm HS10 HS11",
		CameraData {
			black: 0,
			maximum: 0xf68,
			camera_to_xyz: [12440, -3954, -1183, -1123, 9674, 1708, -83, 1614, 4086, 0, 0, 0],
		},
	),
	(
		"Fujifilm HS2",
		CameraData {
			black: 0,
			maximum: 0xfef,
			camera_to_xyz: [13690, -5358, -1474, -3369, 11600, 1998, -132, 1554, 4395, 0, 0, 0],
		},
	),
	(
		"Fujifilm HS3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13690, -5358, -1474, -3369, 11600, 1998, -132, 1554, 4395, 0, 0, 0],
		},
	),
	(
		"Fujifilm HS50EXR",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12085, -4727, -953, -3257, 11489, 2002, -511, 2046, 4592, 0, 0, 0],
		},
	),
	(
		"Fujifilm F900EXR",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12085, -4727, -953, -3257, 11489, 2002, -511, 2046, 4592, 0, 0, 0],
		},
	),
	(
		"Fujifilm X100F",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11434, -4948, -1210, -3746, 12042, 1903, -666, 1479, 5235, 0, 0, 0],
		},
	),
	(
		"Fujifilm X100S",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10592, -4262, -1008, -3514, 11355, 2465, -870, 2025, 6386, 0, 0, 0],
		},
	),
	(
		"Fujifilm X100T",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10592, -4262, -1008, -3514, 11355, 2465, -870, 2025, 6386, 0, 0, 0],
		},
	),
	(
		"Fujifilm X100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12161, -4457, -1069, -5034, 12874, 2400, -795, 1724, 6904, 0, 0, 0],
		},
	),
	(
		"Fujifilm X10",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13509, -6199, -1254, -4430, 12733, 1865, -331, 1441, 5022, 0, 0, 0],
		},
	),
	(
		"Fujifilm X20",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11768, -4971, -1133, -4904, 12927, 2183, -480, 1723, 4605, 0, 0, 0],
		},
	),
	(
		"Fujifilm X30",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12328, -5256, -1144, -4469, 12927, 1675, -87, 1291, 4351, 0, 0, 0],
		},
	),
	(
		"Fujifilm X70",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10450, -4329, -878, -3217, 11105, 2421, -752, 1758, 6519, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-Pro1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10413, -3996, -993, -3721, 11640, 2361, -733, 1540, 6011, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-Pro2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11434, -4948, -1210, -3746, 12042, 1903, -666, 1479, 5235, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-A10",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11540, -4999, -991, -2949, 10963, 2278, -382, 1049, 5605, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-A20",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11540, -4999, -991, -2949, 10963, 2278, -382, 1049, 5605, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-A1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11086, -4555, -839, -3512, 11310, 2517, -815, 1341, 5940, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-A2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10763, -4560, -917, -3346, 11311, 2322, -475, 1135, 5843, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-A3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12407, -5222, -1086, -2971, 11116, 2120, -294, 1029, 5284, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-A5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11673, -4760, -1041, -3988, 12058, 2166, -771, 1417, 5569, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-E1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10413, -3996, -993, -3721, 11640, 2361, -733, 1540, 6011, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-E2S",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11562, -5118, -961, -3022, 11007, 2311, -525, 1569, 6097, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-E2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8458, -2451, -855, -4597, 12447, 2407, -1475, 2482, 6526, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-E3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11434, -4948, -1210, -3746, 12042, 1903, -666, 1479, 5235, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-H1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11434, -4948, -1210, -3746, 12042, 1903, -666, 1479, 5235, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-M1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10413, -3996, -993, -3721, 11640, 2361, -733, 1540, 6011, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-S1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13509, -6199, -1254, -4430, 12733, 1865, -331, 1441, 5022, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-T1",
		CameraData {
			black: 0,
			maximum: 0, /* also X-T10 */
			camera_to_xyz: [8458, -2451, -855, -4597, 12447, 2407, -1475, 2482, 6526, 0, 0, 0],
		},
	),
	(
		"Fujifilm X-T2",
		CameraData {
			black: 0,
			maximum: 0, /* also X-T20 */
			camera_to_xyz: [11434, -4948, -1210, -3746, 12042, 1903, -666, 1479, 5235, 0, 0, 0],
		},
	),
	(
		"Fujifilm XF1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13509, -6199, -1254, -4430, 12733, 1865, -331, 1441, 5022, 0, 0, 0],
		},
	),
	(
		"Fujifilm XQ",
		CameraData {
			black: 0,
			maximum: 0, /* XQ1 and XQ2 */
			camera_to_xyz: [9252, -2704, -1064, -5893, 14265, 1717, -1101, 2341, 4349, 0, 0, 0],
		},
	),
	(
		"GoPro HERO5 Black",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10344, -4210, -620, -2315, 10625, 1948, 93, 1058, 5541, 0, 0, 0],
		},
	),
	(
		"Imacon Ixpress",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [7025, -1415, -704, -5188, 13765, 1424, -1248, 2742, 6038, 0, 0, 0],
		},
	),
	(
		"Kodak NC2000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [13891, -6055, -803, -465, 9919, 642, 2121, 82, 1291, 0, 0, 0],
		},
	),
	(
		"Kodak DCS315C",
		CameraData {
			black: 8,
			maximum: 0,
			camera_to_xyz: [17523, -4827, -2510, 756, 8546, -137, 6113, 1649, 2250, 0, 0, 0],
		},
	),
	(
		"Kodak DCS330C",
		CameraData {
			black: 8,
			maximum: 0,
			camera_to_xyz: [20620, -7572, -2801, -103, 10073, -396, 3551, -233, 2220, 0, 0, 0],
		},
	),
	(
		"Kodak DCS420",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10868, -1852, -644, -1537, 11083, 484, 2343, 628, 2216, 0, 0, 0],
		},
	),
	(
		"Kodak DCS460",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10592, -2206, -967, -1944, 11685, 230, 2206, 670, 1273, 0, 0, 0],
		},
	),
	(
		"Kodak EOSDCS1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10592, -2206, -967, -1944, 11685, 230, 2206, 670, 1273, 0, 0, 0],
		},
	),
	(
		"Kodak EOSDCS3B",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9898, -2700, -940, -2478, 12219, 206, 1985, 634, 1031, 0, 0, 0],
		},
	),
	(
		"Kodak DCS520C",
		CameraData {
			black: 178,
			maximum: 0,
			camera_to_xyz: [24542, -10860, -3401, -1490, 11370, -297, 2858, -605, 3225, 0, 0, 0],
		},
	),
	(
		"Kodak DCS560C",
		CameraData {
			black: 177,
			maximum: 0,
			camera_to_xyz: [20482, -7172, -3125, -1033, 10410, -285, 2542, 226, 3136, 0, 0, 0],
		},
	),
	(
		"Kodak DCS620C",
		CameraData {
			black: 177,
			maximum: 0,
			camera_to_xyz: [23617, -10175, -3149, -2054, 11749, -272, 2586, -489, 3453, 0, 0, 0],
		},
	),
	(
		"Kodak DCS620X",
		CameraData {
			black: 176,
			maximum: 0,
			camera_to_xyz: [13095, -6231, 154, 12221, -21, -2137, 895, 4602, 2258, 0, 0, 0],
		},
	),
	(
		"Kodak DCS660C",
		CameraData {
			black: 173,
			maximum: 0,
			camera_to_xyz: [18244, -6351, -2739, -791, 11193, -521, 3711, -129, 2802, 0, 0, 0],
		},
	),
	(
		"Kodak DCS720X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11775, -5884, 950, 9556, 1846, -1286, -1019, 6221, 2728, 0, 0, 0],
		},
	),
	(
		"Kodak DCS760C",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [16623, -6309, -1411, -4344, 13923, 323, 2285, 274, 2926, 0, 0, 0],
		},
	),
	(
		"Kodak DCS Pro SLR",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5494, 2393, -232, -6427, 13850, 2846, -1876, 3997, 5445, 0, 0, 0],
		},
	),
	(
		"Kodak DCS Pro 14nx",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5494, 2393, -232, -6427, 13850, 2846, -1876, 3997, 5445, 0, 0, 0],
		},
	),
	(
		"Kodak DCS Pro 14",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7791, 3128, -776, -8588, 16458, 2039, -2455, 4006, 6198, 0, 0, 0],
		},
	),
	(
		"Kodak ProBack645",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [16414, -6060, -1470, -3555, 13037, 473, 2545, 122, 4948, 0, 0, 0],
		},
	),
	(
		"Kodak ProBack",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [21179, -8316, -2918, -915, 11019, -165, 3477, -180, 4210, 0, 0, 0],
		},
	),
	(
		"Kodak P712",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9658, -3314, -823, -5163, 12695, 2768, -1342, 1843, 6044, 0, 0, 0],
		},
	),
	(
		"Kodak P850",
		CameraData {
			black: 0,
			maximum: 0xf7c,
			camera_to_xyz: [10511, -3836, -1102, -6946, 14587, 2558, -1481, 1792, 6246, 0, 0, 0],
		},
	),
	(
		"Kodak P880",
		CameraData {
			black: 0,
			maximum: 0xfff,
			camera_to_xyz: [12805, -4662, -1376, -7480, 15267, 2360, -1626, 2194, 7904, 0, 0, 0],
		},
	),
	(
		"Kodak EasyShare Z980",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11313, -3559, -1101, -3893, 11891, 2257, -1214, 2398, 4908, 0, 0, 0],
		},
	),
	(
		"Kodak EasyShare Z981",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12729, -4717, -1188, -1367, 9187, 2582, 274, 860, 4411, 0, 0, 0],
		},
	),
	(
		"Kodak EasyShare Z990",
		CameraData {
			black: 0,
			maximum: 0xfed,
			camera_to_xyz: [11749, -4048, -1309, -1867, 10572, 1489, -138, 1449, 4522, 0, 0, 0],
		},
	),
	(
		"Kodak EASYSHARE Z1015",
		CameraData {
			black: 0,
			maximum: 0xef1,
			camera_to_xyz: [11265, -4286, -992, -4694, 12343, 2647, -1090, 1523, 5447, 0, 0, 0],
		},
	),
	(
		"Leaf CMost",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [3952, 2189, 449, -6701, 14585, 2275, -4536, 7349, 6536, 0, 0, 0],
		},
	),
	(
		"Leaf Valeo 6",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [3952, 2189, 449, -6701, 14585, 2275, -4536, 7349, 6536, 0, 0, 0],
		},
	),
	(
		"Leaf Aptus 54S",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8236, 1746, -1314, -8251, 15953, 2428, -3673, 5786, 5771, 0, 0, 0],
		},
	),
	(
		"Leaf Aptus 65",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7914, 1414, -1190, -8777, 16582, 2280, -2811, 4605, 5562, 0, 0, 0],
		},
	),
	(
		"Leaf Aptus 75",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7914, 1414, -1190, -8777, 16582, 2280, -2811, 4605, 5562, 0, 0, 0],
		},
	),
	(
		"Leaf",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8236, 1746, -1314, -8251, 15953, 2428, -3673, 5786, 5771, 0, 0, 0],
		},
	),
	(
		"Mamiya ZD",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7645, 2579, -1363, -8689, 16717, 2015, -3712, 5941, 5961, 0, 0, 0],
		},
	),
	(
		"Micron 2010",
		CameraData {
			black: 110,
			maximum: 0, /* DJC */
			camera_to_xyz: [16695, -3761, -2151, 155, 9682, 163, 3433, 951, 4904, 0, 0, 0],
		},
	),
	(
		"Minolta DiMAGE 5",
		CameraData {
			black: 0,
			maximum: 0xf7d,
			camera_to_xyz: [8983, -2942, -963, -6556, 14476, 2237, -2426, 2887, 8014, 0, 0, 0],
		},
	),
	(
		"Minolta DiMAGE 7Hi",
		CameraData {
			black: 0,
			maximum: 0xf7d,
			camera_to_xyz: [11368, -3894, -1242, -6521, 14358, 2339, -2475, 3056, 7285, 0, 0, 0],
		},
	),
	(
		"Minolta DiMAGE 7",
		CameraData {
			black: 0,
			maximum: 0xf7d,
			camera_to_xyz: [9144, -2777, -998, -6676, 14556, 2281, -2470, 3019, 7744, 0, 0, 0],
		},
	),
	(
		"Minolta DiMAGE A1",
		CameraData {
			black: 0,
			maximum: 0xf8b,
			camera_to_xyz: [9274, -2547, -1167, -8220, 16323, 1943, -2273, 2720, 8340, 0, 0, 0],
		},
	),
	(
		"Minolta DiMAGE A200",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8560, -2487, -986, -8112, 15535, 2771, -1209, 1324, 7743, 0, 0, 0],
		},
	),
	(
		"Minolta DiMAGE A2",
		CameraData {
			black: 0,
			maximum: 0xf8f,
			camera_to_xyz: [9097, -2726, -1053, -8073, 15506, 2762, -966, 981, 7763, 0, 0, 0],
		},
	),
	(
		"Minolta DiMAGE Z2",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [11280, -3564, -1370, -4655, 12374, 2282, -1423, 2168, 5396, 0, 0, 0],
		},
	),
	(
		"Minolta DYNAX 5",
		CameraData {
			black: 0,
			maximum: 0xffb,
			camera_to_xyz: [10284, -3283, -1086, -7957, 15762, 2316, -829, 882, 6644, 0, 0, 0],
		},
	),
	(
		"Minolta DYNAX 7",
		CameraData {
			black: 0,
			maximum: 0xffb,
			camera_to_xyz: [10239, -3104, -1099, -8037, 15727, 2451, -927, 925, 6871, 0, 0, 0],
		},
	),
	(
		"Motorola PIXL",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [8898, -989, -1033, -3292, 11619, 1674, -661, 3178, 5216, 0, 0, 0],
		},
	),
	(
		"Nikon D100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5902, -933, -782, -8983, 16719, 2354, -1402, 1455, 6464, 0, 0, 0],
		},
	),
	(
		"Nikon D1H",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7577, -2166, -926, -7454, 15592, 1934, -2377, 2808, 8606, 0, 0, 0],
		},
	),
	(
		"Nikon D1X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7702, -2245, -975, -9114, 17242, 1875, -2679, 3055, 8521, 0, 0, 0],
		},
	),
	(
		"Nikon D1",
		CameraData {
			black: 0,
			maximum: 0, /* multiplied by 2.218750, 1.0, 1.148438 */
			camera_to_xyz: [16772, -4726, -2141, -7611, 15713, 1972, -2846, 3494, 9521, 0, 0, 0],
		},
	),
	(
		"Nikon D200",
		CameraData {
			black: 0,
			maximum: 0xfbc,
			camera_to_xyz: [8367, -2248, -763, -8758, 16447, 2422, -1527, 1550, 8053, 0, 0, 0],
		},
	),
	(
		"Nikon D2H",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5710, -901, -615, -8594, 16617, 2024, -2975, 4120, 6830, 0, 0, 0],
		},
	),
	(
		"Nikon D2X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10231, -2769, -1255, -8301, 15900, 2552, -797, 680, 7148, 0, 0, 0],
		},
	),
	(
		"Nikon D3000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8736, -2458, -935, -9075, 16894, 2251, -1354, 1242, 8263, 0, 0, 0],
		},
	),
	(
		"Nikon D3100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7911, -2167, -813, -5327, 13150, 2408, -1288, 2483, 7968, 0, 0, 0],
		},
	),
	(
		"Nikon D3200",
		CameraData {
			black: 0,
			maximum: 0xfb9,
			camera_to_xyz: [7013, -1408, -635, -5268, 12902, 2640, -1470, 2801, 7379, 0, 0, 0],
		},
	),
	(
		"Nikon D3300",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6988, -1384, -714, -5631, 13410, 2447, -1485, 2204, 7318, 0, 0, 0],
		},
	),
	(
		"Nikon D3400",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6988, -1384, -714, -5631, 13410, 2447, -1485, 2204, 7318, 0, 0, 0],
		},
	),
	(
		"Nikon D300",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9030, -1992, -715, -8465, 16302, 2255, -2689, 3217, 8069, 0, 0, 0],
		},
	),
	(
		"Nikon D3X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7171, -1986, -648, -8085, 15555, 2718, -2170, 2512, 7457, 0, 0, 0],
		},
	),
	(
		"Nikon D3S",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8828, -2406, -694, -4874, 12603, 2541, -660, 1509, 7587, 0, 0, 0],
		},
	),
	(
		"Nikon D3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8139, -2171, -663, -8747, 16541, 2295, -1925, 2008, 8093, 0, 0, 0],
		},
	),
	(
		"Nikon D40X",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8819, -2543, -911, -9025, 16928, 2151, -1329, 1213, 8449, 0, 0, 0],
		},
	),
	(
		"Nikon D40",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6992, -1668, -806, -8138, 15748, 2543, -874, 850, 7897, 0, 0, 0],
		},
	),
	(
		"Nikon D4S",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8598, -2848, -857, -5618, 13606, 2195, -1002, 1773, 7137, 0, 0, 0],
		},
	),
	(
		"Nikon D4",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8598, -2848, -857, -5618, 13606, 2195, -1002, 1773, 7137, 0, 0, 0],
		},
	),
	(
		"Nikon Df",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8598, -2848, -857, -5618, 13606, 2195, -1002, 1773, 7137, 0, 0, 0],
		},
	),
	(
		"Nikon D5000",
		CameraData {
			black: 0,
			maximum: 0xf00,
			camera_to_xyz: [7309, -1403, -519, -8474, 16008, 2622, -2433, 2826, 8064, 0, 0, 0],
		},
	),
	(
		"Nikon D5100",
		CameraData {
			black: 0,
			maximum: 0x3de6,
			camera_to_xyz: [8198, -2239, -724, -4871, 12389, 2798, -1043, 2050, 7181, 0, 0, 0],
		},
	),
	(
		"Nikon D5200",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8322, -3112, -1047, -6367, 14342, 2179, -988, 1638, 6394, 0, 0, 0],
		},
	),
	(
		"Nikon D5300",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6988, -1384, -714, -5631, 13410, 2447, -1485, 2204, 7318, 0, 0, 0],
		},
	),
	(
		"Nikon D5500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8821, -2938, -785, -4178, 12142, 2287, -824, 1651, 6860, 0, 0, 0],
		},
	),
	(
		"Nikon D5600",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8821, -2938, -785, -4178, 12142, 2287, -824, 1651, 6860, 0, 0, 0],
		},
	),
	(
		"Nikon D500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8813, -3210, -1036, -4703, 12868, 2021, -1054, 1940, 6129, 0, 0, 0],
		},
	),
	(
		"Nikon D50",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7732, -2422, -789, -8238, 15884, 2498, -859, 783, 7330, 0, 0, 0],
		},
	),
	(
		"Nikon D5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9200, -3522, -992, -5755, 13803, 2117, -753, 1486, 6338, 0, 0, 0],
		},
	),
	(
		"Nikon D600",
		CameraData {
			black: 0,
			maximum: 0x3e07,
			camera_to_xyz: [8178, -2245, -609, -4857, 12394, 2776, -1207, 2086, 7298, 0, 0, 0],
		},
	),
	(
		"Nikon D610",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8178, -2245, -609, -4857, 12394, 2776, -1207, 2086, 7298, 0, 0, 0],
		},
	),
	(
		"Nikon D60",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8736, -2458, -935, -9075, 16894, 2251, -1354, 1242, 8263, 0, 0, 0],
		},
	),
	(
		"Nikon D7000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8198, -2239, -724, -4871, 12389, 2798, -1043, 2050, 7181, 0, 0, 0],
		},
	),
	(
		"Nikon D7100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8322, -3112, -1047, -6367, 14342, 2179, -988, 1638, 6394, 0, 0, 0],
		},
	),
	(
		"Nikon D7200",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8322, -3112, -1047, -6367, 14342, 2179, -988, 1638, 6394, 0, 0, 0],
		},
	),
	(
		"Nikon D7500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8813, -3210, -1036, -4703, 12868, 2021, -1054, 1940, 6129, 0, 0, 0],
		},
	),
	(
		"Nikon D750",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9020, -2890, -715, -4535, 12436, 2348, -934, 1919, 7086, 0, 0, 0],
		},
	),
	(
		"Nikon D700",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8139, -2171, -663, -8747, 16541, 2295, -1925, 2008, 8093, 0, 0, 0],
		},
	),
	(
		"Nikon D70",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7732, -2422, -789, -8238, 15884, 2498, -859, 783, 7330, 0, 0, 0],
		},
	),
	(
		"Nikon D850",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10405, -3755, -1270, -5461, 13787, 1793, -1040, 2015, 6785, 0, 0, 0],
		},
	),
	(
		"Nikon D810",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9369, -3195, -791, -4488, 12430, 2301, -893, 1796, 6872, 0, 0, 0],
		},
	),
	(
		"Nikon D800",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7866, -2108, -555, -4869, 12483, 2681, -1176, 2069, 7501, 0, 0, 0],
		},
	),
	(
		"Nikon D80",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8629, -2410, -883, -9055, 16940, 2171, -1490, 1363, 8520, 0, 0, 0],
		},
	),
	(
		"Nikon D90",
		CameraData {
			black: 0,
			maximum: 0xf00,
			camera_to_xyz: [7309, -1403, -519, -8474, 16008, 2622, -2434, 2826, 8064, 0, 0, 0],
		},
	),
	(
		"Nikon E700",
		CameraData {
			black: 0,
			maximum: 0x3dd, /* DJC */
			camera_to_xyz: [-3746, 10611, 1665, 9621, -1734, 2114, -2389, 7082, 3064, 3406, 6116, -244],
		},
	),
	(
		"Nikon E800",
		CameraData {
			black: 0,
			maximum: 0x3dd, /* DJC */
			camera_to_xyz: [-3746, 10611, 1665, 9621, -1734, 2114, -2389, 7082, 3064, 3406, 6116, -244],
		},
	),
	(
		"Nikon E950",
		CameraData {
			black: 0,
			maximum: 0x3dd, /* DJC */
			camera_to_xyz: [-3746, 10611, 1665, 9621, -1734, 2114, -2389, 7082, 3064, 3406, 6116, -244],
		},
	),
	(
		"Nikon E995",
		CameraData {
			black: 0,
			maximum: 0, /* copied from E5000 */
			camera_to_xyz: [-5547, 11762, 2189, 5814, -558, 3342, -4924, 9840, 5949, 688, 9083, 96],
		},
	),
	(
		"Nikon E2100",
		CameraData {
			black: 0,
			maximum: 0, /* copied from Z2, new white balance */
			camera_to_xyz: [13142, -4152, -1596, -4655, 12374, 2282, -1769, 2696, 6711, 0, 0, 0],
		},
	),
	(
		"Nikon E2500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-5547, 11762, 2189, 5814, -558, 3342, -4924, 9840, 5949, 688, 9083, 96],
		},
	),
	(
		"Nikon E3200",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [9846, -2085, -1019, -3278, 11109, 2170, -774, 2134, 5745, 0, 0, 0],
		},
	),
	(
		"Nikon E4300",
		CameraData {
			black: 0,
			maximum: 0, /* copied from Minolta DiMAGE Z2 */
			camera_to_xyz: [11280, -3564, -1370, -4655, 12374, 2282, -1423, 2168, 5396, 0, 0, 0],
		},
	),
	(
		"Nikon E4500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-5547, 11762, 2189, 5814, -558, 3342, -4924, 9840, 5949, 688, 9083, 96],
		},
	),
	(
		"Nikon E5000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-5547, 11762, 2189, 5814, -558, 3342, -4924, 9840, 5949, 688, 9083, 96],
		},
	),
	(
		"Nikon E5400",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9349, -2987, -1001, -7919, 15766, 2266, -2098, 2680, 6839, 0, 0, 0],
		},
	),
	(
		"Nikon E5700",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [-5368, 11478, 2368, 5537, -113, 3148, -4969, 10021, 5782, 778, 9028, 211],
		},
	),
	(
		"Nikon E8400",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7842, -2320, -992, -8154, 15718, 2599, -1098, 1342, 7560, 0, 0, 0],
		},
	),
	(
		"Nikon E8700",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8489, -2583, -1036, -8051, 15583, 2643, -1307, 1407, 7354, 0, 0, 0],
		},
	),
	(
		"Nikon E8800",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7971, -2314, -913, -8451, 15762, 2894, -1442, 1520, 7610, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX A",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8198, -2239, -724, -4871, 12389, 2798, -1043, 2050, 7181, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX B700",
		CameraData {
			black: 200,
			maximum: 0,
			camera_to_xyz: [14387, -6014, -1299, -1357, 9975, 1616, 467, 1047, 4744, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX P330",
		CameraData {
			black: 200,
			maximum: 0,
			camera_to_xyz: [10321, -3920, -931, -2750, 11146, 1824, -442, 1545, 5539, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX P340",
		CameraData {
			black: 200,
			maximum: 0,
			camera_to_xyz: [10321, -3920, -931, -2750, 11146, 1824, -442, 1545, 5539, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX P6000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9698, -3367, -914, -4706, 12584, 2368, -837, 968, 5801, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX P7000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11432, -3679, -1111, -3169, 11239, 2202, -791, 1380, 4455, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX P7100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11053, -4269, -1024, -1976, 10182, 2088, -526, 1263, 4469, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX P7700",
		CameraData {
			black: 200,
			maximum: 0,
			camera_to_xyz: [10321, -3920, -931, -2750, 11146, 1824, -442, 1545, 5539, 0, 0, 0],
		},
	),
	(
		"Nikon COOLPIX P7800",
		CameraData {
			black: 200,
			maximum: 0,
			camera_to_xyz: [10321, -3920, -931, -2750, 11146, 1824, -442, 1545, 5539, 0, 0, 0],
		},
	),
	(
		"Nikon 1 V3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5958, -1559, -571, -4021, 11453, 2939, -634, 1548, 5087, 0, 0, 0],
		},
	),
	(
		"Nikon 1 J4",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5958, -1559, -571, -4021, 11453, 2939, -634, 1548, 5087, 0, 0, 0],
		},
	),
	(
		"Nikon 1 J5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7520, -2518, -645, -3844, 12102, 1945, -913, 2249, 6835, 0, 0, 0],
		},
	),
	(
		"Nikon 1 S2",
		CameraData {
			black: 200,
			maximum: 0,
			camera_to_xyz: [6612, -1342, -618, -3338, 11055, 2623, -174, 1792, 5075, 0, 0, 0],
		},
	),
	(
		"Nikon 1 V2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6588, -1305, -693, -3277, 10987, 2634, -355, 2016, 5106, 0, 0, 0],
		},
	),
	(
		"Nikon 1 J3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6588, -1305, -693, -3277, 10987, 2634, -355, 2016, 5106, 0, 0, 0],
		},
	),
	(
		"Nikon 1 AW1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6588, -1305, -693, -3277, 10987, 2634, -355, 2016, 5106, 0, 0, 0],
		},
	),
	(
		"Nikon 1 ",
		CameraData {
			black: 0,
			maximum: 0, /* J1, J2, S1, V1 */
			camera_to_xyz: [8994, -2667, -865, -4594, 12324, 2552, -699, 1786, 6260, 0, 0, 0],
		},
	),
	(
		"Olympus AIR A01",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8992, -3093, -639, -2563, 10721, 2122, -437, 1270, 5473, 0, 0, 0],
		},
	),
	(
		"Olympus C5050",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10508, -3124, -1273, -6079, 14294, 1901, -1653, 2306, 6237, 0, 0, 0],
		},
	),
	(
		"Olympus C5060",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10445, -3362, -1307, -7662, 15690, 2058, -1135, 1176, 7602, 0, 0, 0],
		},
	),
	(
		"Olympus C7070",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10252, -3531, -1095, -7114, 14850, 2436, -1451, 1723, 6365, 0, 0, 0],
		},
	),
	(
		"Olympus C70",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10793, -3791, -1146, -7498, 15177, 2488, -1390, 1577, 7321, 0, 0, 0],
		},
	),
	(
		"Olympus C80",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8606, -2509, -1014, -8238, 15714, 2703, -942, 979, 7760, 0, 0, 0],
		},
	),
	(
		"Olympus E-10",
		CameraData {
			black: 0,
			maximum: 0xffc,
			camera_to_xyz: [12745, -4500, -1416, -6062, 14542, 1580, -1934, 2256, 6603, 0, 0, 0],
		},
	),
	(
		"Olympus E-1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11846, -4767, -945, -7027, 15878, 1089, -2699, 4122, 8311, 0, 0, 0],
		},
	),
	(
		"Olympus E-20",
		CameraData {
			black: 0,
			maximum: 0xffc,
			camera_to_xyz: [13173, -4732, -1499, -5807, 14036, 1895, -2045, 2452, 7142, 0, 0, 0],
		},
	),
	(
		"Olympus E-300",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7828, -1761, -348, -5788, 14071, 1830, -2853, 4518, 6557, 0, 0, 0],
		},
	),
	(
		"Olympus E-330",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8961, -2473, -1084, -7979, 15990, 2067, -2319, 3035, 8249, 0, 0, 0],
		},
	),
	(
		"Olympus E-30",
		CameraData {
			black: 0,
			maximum: 0xfbc,
			camera_to_xyz: [8144, -1861, -1111, -7763, 15894, 1929, -1865, 2542, 7607, 0, 0, 0],
		},
	),
	(
		"Olympus E-3",
		CameraData {
			black: 0,
			maximum: 0xf99,
			camera_to_xyz: [9487, -2875, -1115, -7533, 15606, 2010, -1618, 2100, 7389, 0, 0, 0],
		},
	),
	(
		"Olympus E-400",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6169, -1483, -21, -7107, 14761, 2536, -2904, 3580, 8568, 0, 0, 0],
		},
	),
	(
		"Olympus E-410",
		CameraData {
			black: 0,
			maximum: 0xf6a,
			camera_to_xyz: [8856, -2582, -1026, -7761, 15766, 2082, -2009, 2575, 7469, 0, 0, 0],
		},
	),
	(
		"Olympus E-420",
		CameraData {
			black: 0,
			maximum: 0xfd7,
			camera_to_xyz: [8746, -2425, -1095, -7594, 15612, 2073, -1780, 2309, 7416, 0, 0, 0],
		},
	),
	(
		"Olympus E-450",
		CameraData {
			black: 0,
			maximum: 0xfd2,
			camera_to_xyz: [8745, -2425, -1095, -7594, 15613, 2073, -1780, 2309, 7416, 0, 0, 0],
		},
	),
	(
		"Olympus E-500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8136, -1968, -299, -5481, 13742, 1871, -2556, 4205, 6630, 0, 0, 0],
		},
	),
	(
		"Olympus E-510",
		CameraData {
			black: 0,
			maximum: 0xf6a,
			camera_to_xyz: [8785, -2529, -1033, -7639, 15624, 2112, -1783, 2300, 7817, 0, 0, 0],
		},
	),
	(
		"Olympus E-520",
		CameraData {
			black: 0,
			maximum: 0xfd2,
			camera_to_xyz: [8344, -2322, -1020, -7596, 15635, 2048, -1748, 2269, 7287, 0, 0, 0],
		},
	),
	(
		"Olympus E-5",
		CameraData {
			black: 0,
			maximum: 0xeec,
			camera_to_xyz: [11200, -3783, -1325, -4576, 12593, 2206, -695, 1742, 7504, 0, 0, 0],
		},
	),
	(
		"Olympus E-600",
		CameraData {
			black: 0,
			maximum: 0xfaf,
			camera_to_xyz: [8453, -2198, -1092, -7609, 15681, 2008, -1725, 2337, 7824, 0, 0, 0],
		},
	),
	(
		"Olympus E-620",
		CameraData {
			black: 0,
			maximum: 0xfaf,
			camera_to_xyz: [8453, -2198, -1092, -7609, 15681, 2008, -1725, 2337, 7824, 0, 0, 0],
		},
	),
	(
		"Olympus E-P1",
		CameraData {
			black: 0,
			maximum: 0xffd,
			camera_to_xyz: [8343, -2050, -1021, -7715, 15705, 2103, -1831, 2380, 8235, 0, 0, 0],
		},
	),
	(
		"Olympus E-P2",
		CameraData {
			black: 0,
			maximum: 0xffd,
			camera_to_xyz: [8343, -2050, -1021, -7715, 15705, 2103, -1831, 2380, 8235, 0, 0, 0],
		},
	),
	(
		"Olympus E-P3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7575, -2159, -571, -3722, 11341, 2725, -1434, 2819, 6271, 0, 0, 0],
		},
	),
	(
		"Olympus E-P5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8380, -2630, -639, -2887, 10725, 2496, -627, 1427, 5438, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL1s",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11409, -3872, -1393, -4572, 12757, 2003, -709, 1810, 7415, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11408, -4289, -1215, -4286, 12385, 2118, -387, 1467, 7787, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL2",
		CameraData {
			black: 0,
			maximum: 0xcf3,
			camera_to_xyz: [15030, -5552, -1806, -3987, 12387, 1767, -592, 1670, 7023, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7575, -2159, -571, -3722, 11341, 2725, -1434, 2819, 6271, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL5",
		CameraData {
			black: 0,
			maximum: 0xfcb,
			camera_to_xyz: [8380, -2630, -639, -2887, 10725, 2496, -627, 1427, 5438, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL6",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8380, -2630, -639, -2887, 10725, 2496, -627, 1427, 5438, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL7",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9197, -3190, -659, -2606, 10830, 2039, -458, 1250, 5458, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL8",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9197, -3190, -659, -2606, 10830, 2039, -458, 1250, 5458, 0, 0, 0],
		},
	),
	(
		"Olympus E-PL9",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8380, -2630, -639, -2887, 10725, 2496, -627, 1427, 5438, 0, 0, 0],
		},
	),
	(
		"Olympus E-PM1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7575, -2159, -571, -3722, 11341, 2725, -1434, 2819, 6271, 0, 0, 0],
		},
	),
	(
		"Olympus E-PM2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8380, -2630, -639, -2887, 10725, 2496, -627, 1427, 5438, 0, 0, 0],
		},
	),
	(
		"Olympus E-M10",
		CameraData {
			black: 0,
			maximum: 0, /* also E-M10 Mark II & III */
			camera_to_xyz: [8380, -2630, -639, -2887, 10725, 2496, -627, 1427, 5438, 0, 0, 0],
		},
	),
	(
		"Olympus E-M1Mark II",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9383, -3170, -763, -2457, 10702, 2020, -384, 1236, 5552, 0, 0, 0],
		},
	),
	(
		"Olympus E-M1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7687, -1984, -606, -4327, 11928, 2721, -1381, 2339, 6452, 0, 0, 0],
		},
	),
	(
		"Olympus E-M5MarkII",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9422, -3258, -711, -2655, 10898, 2015, -512, 1354, 5512, 0, 0, 0],
		},
	),
	(
		"Olympus E-M5",
		CameraData {
			black: 0,
			maximum: 0xfe1,
			camera_to_xyz: [8380, -2630, -639, -2887, 10725, 2496, -627, 1427, 5438, 0, 0, 0],
		},
	),
	(
		"Olympus PEN-F",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9476, -3182, -765, -2613, 10958, 1893, -449, 1315, 5268, 0, 0, 0],
		},
	),
	(
		"Olympus SH-2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10156, -3425, -1077, -2611, 11177, 1624, -385, 1592, 5080, 0, 0, 0],
		},
	),
	(
		"Olympus SP350",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12078, -4836, -1069, -6671, 14306, 2578, -786, 939, 7418, 0, 0, 0],
		},
	),
	(
		"Olympus SP3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11766, -4445, -1067, -6901, 14421, 2707, -1029, 1217, 7572, 0, 0, 0],
		},
	),
	(
		"Olympus SP500UZ",
		CameraData {
			black: 0,
			maximum: 0xfff,
			camera_to_xyz: [9493, -3415, -666, -5211, 12334, 3260, -1548, 2262, 6482, 0, 0, 0],
		},
	),
	(
		"Olympus SP510UZ",
		CameraData {
			black: 0,
			maximum: 0xffe,
			camera_to_xyz: [10593, -3607, -1010, -5881, 13127, 3084, -1200, 1805, 6721, 0, 0, 0],
		},
	),
	(
		"Olympus SP550UZ",
		CameraData {
			black: 0,
			maximum: 0xffe,
			camera_to_xyz: [11597, -4006, -1049, -5432, 12799, 2957, -1029, 1750, 6516, 0, 0, 0],
		},
	),
	(
		"Olympus SP560UZ",
		CameraData {
			black: 0,
			maximum: 0xff9,
			camera_to_xyz: [10915, -3677, -982, -5587, 12986, 2911, -1168, 1968, 6223, 0, 0, 0],
		},
	),
	(
		"Olympus SP570UZ",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11522, -4044, -1146, -4736, 12172, 2904, -988, 1829, 6039, 0, 0, 0],
		},
	),
	(
		"Olympus STYLUS1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8360, -2420, -880, -3928, 12353, 1739, -1381, 2416, 5173, 0, 0, 0],
		},
	),
	(
		"Olympus TG-4",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11426, -4159, -1126, -2066, 10678, 1593, -120, 1327, 4998, 0, 0, 0],
		},
	),
	(
		"Olympus TG-5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10899, -3833, -1082, -2112, 10736, 1575, -267, 1452, 5269, 0, 0, 0],
		},
	),
	(
		"Olympus XZ-10",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9777, -3483, -925, -2886, 11297, 1800, -602, 1663, 5134, 0, 0, 0],
		},
	),
	(
		"Olympus XZ-1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10901, -4095, -1074, -1141, 9208, 2293, -62, 1417, 5158, 0, 0, 0],
		},
	),
	(
		"Olympus XZ-2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9777, -3483, -925, -2886, 11297, 1800, -602, 1663, 5134, 0, 0, 0],
		},
	),
	(
		"OmniVision",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [12782, -4059, -379, -478, 9066, 1413, 1340, 1513, 5176, 0, 0, 0],
		},
	),
	(
		"Pentax *ist DL2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10504, -2438, -1189, -8603, 16207, 2531, -1022, 863, 12242, 0, 0, 0],
		},
	),
	(
		"Pentax *ist DL",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10829, -2838, -1115, -8339, 15817, 2696, -837, 680, 11939, 0, 0, 0],
		},
	),
	(
		"Pentax *ist DS2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10504, -2438, -1189, -8603, 16207, 2531, -1022, 863, 12242, 0, 0, 0],
		},
	),
	(
		"Pentax *ist DS",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10371, -2333, -1206, -8688, 16231, 2602, -1230, 1116, 11282, 0, 0, 0],
		},
	),
	(
		"Pentax *ist D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9651, -2059, -1189, -8881, 16512, 2487, -1460, 1345, 10687, 0, 0, 0],
		},
	),
	(
		"Pentax K10D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9566, -2863, -803, -7170, 15172, 2112, -818, 803, 9705, 0, 0, 0],
		},
	),
	(
		"Pentax K1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11095, -3157, -1324, -8377, 15834, 2720, -1108, 947, 11688, 0, 0, 0],
		},
	),
	(
		"Pentax K20D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9427, -2714, -868, -7493, 16092, 1373, -2199, 3264, 7180, 0, 0, 0],
		},
	),
	(
		"Pentax K200D",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9186, -2678, -907, -8693, 16517, 2260, -1129, 1094, 8524, 0, 0, 0],
		},
	),
	(
		"Pentax K2000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11057, -3604, -1155, -5152, 13046, 2329, -282, 375, 8104, 0, 0, 0],
		},
	),
	(
		"Pentax K-m",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11057, -3604, -1155, -5152, 13046, 2329, -282, 375, 8104, 0, 0, 0],
		},
	),
	(
		"Pentax K-x",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8843, -2837, -625, -5025, 12644, 2668, -411, 1234, 7410, 0, 0, 0],
		},
	),
	(
		"Pentax K-r",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9895, -3077, -850, -5304, 13035, 2521, -883, 1768, 6936, 0, 0, 0],
		},
	),
	(
		"Pentax K-1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8596, -2981, -639, -4202, 12046, 2431, -685, 1424, 6122, 0, 0, 0],
		},
	),
	(
		"Pentax K-30",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8710, -2632, -1167, -3995, 12301, 1881, -981, 1719, 6535, 0, 0, 0],
		},
	),
	(
		"Pentax K-3 II",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8626, -2607, -1155, -3995, 12301, 1881, -1039, 1822, 6925, 0, 0, 0],
		},
	),
	(
		"Pentax K-3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7415, -2052, -721, -5186, 12788, 2682, -1446, 2157, 6773, 0, 0, 0],
		},
	),
	(
		"Pentax K-5 II",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8170, -2725, -639, -4440, 12017, 2744, -771, 1465, 6599, 0, 0, 0],
		},
	),
	(
		"Pentax K-5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8713, -2833, -743, -4342, 11900, 2772, -722, 1543, 6247, 0, 0, 0],
		},
	),
	(
		"Pentax K-70",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8270, -2117, -1299, -4359, 12953, 1515, -1078, 1933, 5975, 0, 0, 0],
		},
	),
	(
		"Pentax K-7",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9142, -2947, -678, -8648, 16967, 1663, -2224, 2898, 8615, 0, 0, 0],
		},
	),
	(
		"Pentax K-S1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8512, -3211, -787, -4167, 11966, 2487, -638, 1288, 6054, 0, 0, 0],
		},
	),
	(
		"Pentax K-S2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8662, -3280, -798, -3928, 11771, 2444, -586, 1232, 6054, 0, 0, 0],
		},
	),
	(
		"Pentax KP",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8617, -3228, -1034, -4674, 12821, 2044, -803, 1577, 5728, 0, 0, 0],
		},
	),
	(
		"Pentax Q-S1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [12995, -5593, -1107, -1879, 10139, 2027, -64, 1233, 4919, 0, 0, 0],
		},
	),
	(
		"Pentax 645D",
		CameraData {
			black: 0,
			maximum: 0x3e00,
			camera_to_xyz: [10646, -3593, -1158, -3329, 11699, 1831, -667, 2874, 6287, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-CM1",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8770, -3194, -820, -2871, 11281, 1803, -513, 1552, 4434, 0, 0, 0],
		},
	),
	(
		"Panasonic DC-FZ80",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8550, -2908, -842, -3195, 11529, 1881, -338, 1603, 4631, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ8",
		CameraData {
			black: 0,
			maximum: 0xf7f,
			camera_to_xyz: [8986, -2755, -802, -6341, 13575, 3077, -1476, 2144, 6379, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ18",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [9932, -3060, -935, -5809, 13331, 2753, -1267, 2155, 5575, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ28",
		CameraData {
			black: 15,
			maximum: 0xf96,
			camera_to_xyz: [10109, -3488, -993, -5412, 12812, 2916, -1305, 2140, 5543, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ2500",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7386, -2443, -743, -3437, 11864, 1757, -608, 1660, 4766, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ330",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8378, -2798, -769, -3068, 11410, 1877, -538, 1792, 4623, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ300",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8378, -2798, -769, -3068, 11410, 1877, -538, 1792, 4623, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ30",
		CameraData {
			black: 0,
			maximum: 0xf94,
			camera_to_xyz: [10976, -4029, -1141, -7918, 15491, 2600, -1670, 2071, 8246, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ3",
		CameraData {
			black: 15,
			maximum: 0, /* FZ35, FZ38 */
			camera_to_xyz: [9938, -2780, -890, -4604, 12393, 2480, -1117, 2304, 4620, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ4",
		CameraData {
			black: 15,
			maximum: 0, /* FZ40, FZ45 */
			camera_to_xyz: [13639, -5535, -1371, -1698, 9633, 2430, 316, 1152, 4108, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ50",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7906, -2709, -594, -6231, 13351, 3220, -1922, 2631, 6537, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ7",
		CameraData {
			black: 15,
			maximum: 0, /* FZ70, FZ72 */
			camera_to_xyz: [11532, -4324, -1066, -2375, 10847, 1749, -564, 1699, 4351, 0, 0, 0],
		},
	),
	(
		"Leica V-LUX1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7906, -2709, -594, -6231, 13351, 3220, -1922, 2631, 6537, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-L10",
		CameraData {
			black: 15,
			maximum: 0xf96,
			camera_to_xyz: [8025, -1942, -1050, -7920, 15904, 2100, -2456, 3005, 7039, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-L1",
		CameraData {
			black: 0,
			maximum: 0xf7f,
			camera_to_xyz: [8054, -1885, -1025, -8349, 16367, 2040, -2805, 3542, 7629, 0, 0, 0],
		},
	),
	(
		"Leica DIGILUX 3",
		CameraData {
			black: 0,
			maximum: 0xf7f,
			camera_to_xyz: [8054, -1885, -1025, -8349, 16367, 2040, -2805, 3542, 7629, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LC1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11340, -4069, -1275, -7555, 15266, 2448, -2960, 3426, 7685, 0, 0, 0],
		},
	),
	(
		"Leica DIGILUX 2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11340, -4069, -1275, -7555, 15266, 2448, -2960, 3426, 7685, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LX100",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8844, -3538, -768, -3709, 11762, 2200, -698, 1792, 5220, 0, 0, 0],
		},
	),
	(
		"Leica D-LUX (Typ 109)",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8844, -3538, -768, -3709, 11762, 2200, -698, 1792, 5220, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LF1",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [9379, -3267, -816, -3227, 11560, 1881, -926, 1928, 5340, 0, 0, 0],
		},
	),
	(
		"Leica C (Typ 112)",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [9379, -3267, -816, -3227, 11560, 1881, -926, 1928, 5340, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LX1",
		CameraData {
			black: 0,
			maximum: 0xf7f,
			camera_to_xyz: [10704, -4187, -1230, -8314, 15952, 2501, -920, 945, 8927, 0, 0, 0],
		},
	),
	(
		"Leica D-LUX2",
		CameraData {
			black: 0,
			maximum: 0xf7f,
			camera_to_xyz: [10704, -4187, -1230, -8314, 15952, 2501, -920, 945, 8927, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LX2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8048, -2810, -623, -6450, 13519, 3272, -1700, 2146, 7049, 0, 0, 0],
		},
	),
	(
		"Leica D-LUX3",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8048, -2810, -623, -6450, 13519, 3272, -1700, 2146, 7049, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LX3",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8128, -2668, -655, -6134, 13307, 3161, -1782, 2568, 6083, 0, 0, 0],
		},
	),
	(
		"Leica D-LUX 4",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8128, -2668, -655, -6134, 13307, 3161, -1782, 2568, 6083, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LX5",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [10909, -4295, -948, -1333, 9306, 2399, 22, 1738, 4582, 0, 0, 0],
		},
	),
	(
		"Leica D-LUX 5",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [10909, -4295, -948, -1333, 9306, 2399, 22, 1738, 4582, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LX7",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [10148, -3743, -991, -2837, 11366, 1659, -701, 1893, 4899, 0, 0, 0],
		},
	),
	(
		"Leica D-LUX 6",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [10148, -3743, -991, -2837, 11366, 1659, -701, 1893, 4899, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-LX9",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7790, -2736, -755, -3452, 11870, 1769, -628, 1647, 4898, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ1000",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7830, -2696, -763, -3325, 11667, 1866, -641, 1712, 4824, 0, 0, 0],
		},
	),
	(
		"Leica V-LUX (Typ 114)",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7830, -2696, -763, -3325, 11667, 1866, -641, 1712, 4824, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ100",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [16197, -6146, -1761, -2393, 10765, 1869, 366, 2238, 5248, 0, 0, 0],
		},
	),
	(
		"Leica V-LUX 2",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [16197, -6146, -1761, -2393, 10765, 1869, 366, 2238, 5248, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ150",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [11904, -4541, -1189, -2355, 10899, 1662, -296, 1586, 4289, 0, 0, 0],
		},
	),
	(
		"Leica V-LUX 3",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [11904, -4541, -1189, -2355, 10899, 1662, -296, 1586, 4289, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FZ200",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [8112, -2563, -740, -3730, 11784, 2197, -941, 2075, 4933, 0, 0, 0],
		},
	),
	(
		"Leica V-LUX 4",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [8112, -2563, -740, -3730, 11784, 2197, -941, 2075, 4933, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-FX150",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [9082, -2907, -925, -6119, 13377, 3058, -1797, 2641, 5609, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G10",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10113, -3400, -1114, -4765, 12683, 2317, -377, 1437, 6710, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G1",
		CameraData {
			black: 15,
			maximum: 0xf94,
			camera_to_xyz: [8199, -2065, -1056, -8124, 16156, 2033, -2458, 3022, 7220, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G2",
		CameraData {
			black: 15,
			maximum: 0xf3c,
			camera_to_xyz: [10113, -3400, -1114, -4765, 12683, 2317, -377, 1437, 6710, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G3",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [6763, -1919, -863, -3868, 11515, 2684, -1216, 2387, 5879, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G5",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [7798, -2562, -740, -3879, 11584, 2613, -1055, 2248, 5434, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G6",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [8294, -2891, -651, -3869, 11590, 2595, -1183, 2267, 5352, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G7",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [7610, -2780, -576, -4614, 12195, 2733, -1375, 2393, 6490, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-G8",
		CameraData {
			black: 15,
			maximum: 0xfff, /* G8, G80, G81, G85 */
			camera_to_xyz: [7610, -2780, -576, -4614, 12195, 2733, -1375, 2393, 6490, 0, 0, 0],
		},
	),
	(
		"Panasonic DC-G9",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [7685, -2375, -634, -3687, 11700, 2249, -748, 1546, 5111, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GF1",
		CameraData {
			black: 15,
			maximum: 0xf92,
			camera_to_xyz: [7888, -1902, -1011, -8106, 16085, 2099, -2353, 2866, 7330, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GF2",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [7888, -1902, -1011, -8106, 16085, 2099, -2353, 2866, 7330, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GF3",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [9051, -2468, -1204, -5212, 13276, 2121, -1197, 2510, 6890, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GF5",
		CameraData {
			black: 15,
			maximum: 0xfff,
			camera_to_xyz: [8228, -2945, -660, -3938, 11792, 2430, -1094, 2278, 5793, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GF6",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8130, -2801, -946, -3520, 11289, 2552, -1314, 2511, 5791, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GF7",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7610, -2780, -576, -4614, 12195, 2733, -1375, 2393, 6490, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GF8",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7610, -2780, -576, -4614, 12195, 2733, -1375, 2393, 6490, 0, 0, 0],
		},
	),
	(
		"Panasonic DC-GF9",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7610, -2780, -576, -4614, 12195, 2733, -1375, 2393, 6490, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GH1",
		CameraData {
			black: 15,
			maximum: 0xf92,
			camera_to_xyz: [6299, -1466, -532, -6535, 13852, 2969, -2331, 3112, 5984, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GH2",
		CameraData {
			black: 15,
			maximum: 0xf95,
			camera_to_xyz: [7780, -2410, -806, -3913, 11724, 2484, -1018, 2390, 5298, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GH3",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [6559, -1752, -491, -3672, 11407, 2586, -962, 1875, 5130, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GH4",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7122, -2108, -512, -3155, 11201, 2231, -541, 1423, 5045, 0, 0, 0],
		},
	),
	(
		"Panasonic DC-GH5S",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [6929, -2355, -708, -4192, 12534, 1828, -1097, 1989, 5195, 0, 0, 0],
		},
	),
	(
		"Panasonic DC-GH5",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7641, -2336, -605, -3218, 11299, 2187, -485, 1338, 5121, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GM1",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [6770, -1895, -744, -5232, 13145, 2303, -1664, 2691, 5703, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GM5",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8238, -3244, -679, -3921, 11814, 2384, -836, 2022, 5852, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GX1",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [6763, -1919, -863, -3868, 11515, 2684, -1216, 2387, 5879, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GX7",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7610, -2780, -576, -4614, 12195, 2733, -1375, 2393, 6490, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GX85",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7771, -3020, -629, -4029, 11950, 2345, -821, 1977, 6119, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-GX8",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7564, -2263, -606, -3148, 11239, 2177, -540, 1435, 4853, 0, 0, 0],
		},
	),
	(
		"Panasonic DC-GX9",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7564, -2263, -606, -3148, 11239, 2177, -540, 1435, 4853, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-ZS100",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7790, -2736, -755, -3452, 11870, 1769, -628, 1647, 4898, 0, 0, 0],
		},
	),
	(
		"Panasonic DC-ZS200",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [7790, -2736, -755, -3452, 11870, 1769, -628, 1647, 4898, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-ZS40",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8607, -2822, -808, -3755, 11930, 2049, -820, 2060, 5224, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-ZS50",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8802, -3135, -789, -3151, 11468, 1904, -550, 1745, 4810, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-TZ82",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8550, -2908, -842, -3195, 11529, 1881, -338, 1603, 4631, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-ZS6",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [8550, -2908, -842, -3195, 11529, 1881, -338, 1603, 4631, 0, 0, 0],
		},
	),
	(
		"Panasonic DMC-ZS70",
		CameraData {
			black: 15,
			maximum: 0,
			camera_to_xyz: [9052, -3117, -883, -3045, 11346, 1927, -205, 1520, 4730, 0, 0, 0],
		},
	),
	(
		"Leica S (Typ 007)",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6063, -2234, -231, -5210, 13787, 1500, -1043, 2866, 6997, 0, 0, 0],
		},
	),
	(
		"Leica X",
		CameraData {
			black: 0,
			maximum: 0, /* X and X-U, both (Typ 113) */
			camera_to_xyz: [7712, -2059, -653, -3882, 11494, 2726, -710, 1332, 5958, 0, 0, 0],
		},
	),
	(
		"Leica Q (Typ 116)",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11865, -4523, -1441, -5423, 14458, 935, -1587, 2687, 4830, 0, 0, 0],
		},
	),
	(
		"Leica M (Typ 262)",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6653, -1486, -611, -4221, 13303, 929, -881, 2416, 7226, 0, 0, 0],
		},
	),
	(
		"Leica SL (Typ 601)",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [11865, -4523, -1441, -5423, 14458, 935, -1587, 2687, 4830, 0, 0, 0],
		},
	),
	(
		"Leica TL2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5836, -1626, -647, -5384, 13326, 2261, -1207, 2129, 5861, 0, 0, 0],
		},
	),
	(
		"Leica TL",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5463, -988, -364, -4634, 12036, 2946, -766, 1389, 6522, 0, 0, 0],
		},
	),
	(
		"Leica CL",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7414, -2393, -840, -5127, 13180, 2138, -1585, 2468, 5064, 0, 0, 0],
		},
	),
	(
		"Leica M10",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8249, -2849, -620, -5415, 14756, 565, -957, 3074, 6517, 0, 0, 0],
		},
	),
	(
		"Phase One H 20",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [1313, 1855, -109, -6715, 15908, 808, -327, 1840, 6020, 0, 0, 0],
		},
	),
	(
		"Phase One H 25",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [2905, 732, -237, -8134, 16626, 1476, -3038, 4253, 7517, 0, 0, 0],
		},
	),
	(
		"Phase One P 2",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [2905, 732, -237, -8134, 16626, 1476, -3038, 4253, 7517, 0, 0, 0],
		},
	),
	(
		"Phase One P 30",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [4516, -245, -37, -7020, 14976, 2173, -3206, 4671, 7087, 0, 0, 0],
		},
	),
	(
		"Phase One P 45",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5053, -24, -117, -5684, 14076, 1702, -2619, 4492, 5849, 0, 0, 0],
		},
	),
	(
		"Phase One P40",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8035, 435, -962, -6001, 13872, 2320, -1159, 3065, 5434, 0, 0, 0],
		},
	),
	(
		"Phase One P65",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8035, 435, -962, -6001, 13872, 2320, -1159, 3065, 5434, 0, 0, 0],
		},
	),
	(
		"Photron BC2-HD",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [14603, -4122, -528, -1810, 9794, 2017, -297, 2763, 5936, 0, 0, 0],
		},
	),
	(
		"Red One",
		CameraData {
			black: 704,
			maximum: 0xffff, /* DJC */
			camera_to_xyz: [21014, -7891, -2613, -3056, 12201, 856, -2203, 5125, 8042, 0, 0, 0],
		},
	),
	(
		"Ricoh GR II",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [4630, -834, -423, -4977, 12805, 2417, -638, 1467, 6115, 0, 0, 0],
		},
	),
	(
		"Ricoh GR",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [3708, -543, -160, -5381, 12254, 3556, -1471, 1929, 8234, 0, 0, 0],
		},
	),
	(
		"Samsung EX1",
		CameraData {
			black: 0,
			maximum: 0x3e00,
			camera_to_xyz: [8898, -2498, -994, -3144, 11328, 2066, -760, 1381, 4576, 0, 0, 0],
		},
	),
	(
		"Samsung EX2F",
		CameraData {
			black: 0,
			maximum: 0x7ff,
			camera_to_xyz: [10648, -3897, -1055, -2022, 10573, 1668, -492, 1611, 4742, 0, 0, 0],
		},
	),
	(
		"Samsung EK-GN120",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7557, -2522, -739, -4679, 12949, 1894, -840, 1777, 5311, 0, 0, 0],
		},
	),
	(
		"Samsung NX mini",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [5222, -1196, -550, -6540, 14649, 2009, -1666, 2819, 5657, 0, 0, 0],
		},
	),
	(
		"Samsung NX3300",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8060, -2933, -761, -4504, 12890, 1762, -630, 1489, 5227, 0, 0, 0],
		},
	),
	(
		"Samsung NX3000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [8060, -2933, -761, -4504, 12890, 1762, -630, 1489, 5227, 0, 0, 0],
		},
	),
	(
		"Samsung NX30",
		CameraData {
			black: 0,
			maximum: 0, /* NX30, NX300, NX300M */
			camera_to_xyz: [7557, -2522, -739, -4679, 12949, 1894, -840, 1777, 5311, 0, 0, 0],
		},
	),
	(
		"Samsung NX2000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7557, -2522, -739, -4679, 12949, 1894, -840, 1777, 5311, 0, 0, 0],
		},
	),
	(
		"Samsung NX2",
		CameraData {
			black: 0,
			maximum: 0xfff, /* NX20, NX200, NX210 */
			camera_to_xyz: [6933, -2268, -753, -4921, 13387, 1647, -803, 1641, 6096, 0, 0, 0],
		},
	),
	(
		"Samsung NX1000",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6933, -2268, -753, -4921, 13387, 1647, -803, 1641, 6096, 0, 0, 0],
		},
	),
	(
		"Samsung NX1100",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [6933, -2268, -753, -4921, 13387, 1647, -803, 1641, 6096, 0, 0, 0],
		},
	),
	(
		"Samsung NX11",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10332, -3234, -1168, -6111, 14639, 1520, -1352, 2647, 8331, 0, 0, 0],
		},
	),
	(
		"Samsung NX10",
		CameraData {
			black: 0,
			maximum: 0, /* also NX100 */
			camera_to_xyz: [10332, -3234, -1168, -6111, 14639, 1520, -1352, 2647, 8331, 0, 0, 0],
		},
	),
	(
		"Samsung NX500",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10686, -4042, -1052, -3595, 13238, 276, -464, 1259, 5931, 0, 0, 0],
		},
	),
	(
		"Samsung NX5",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10332, -3234, -1168, -6111, 14639, 1520, -1352, 2647, 8331, 0, 0, 0],
		},
	),
	(
		"Samsung NX1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10686, -4042, -1052, -3595, 13238, 276, -464, 1259, 5931, 0, 0, 0],
		},
	),
	(
		"Samsung WB2000",
		CameraData {
			black: 0,
			maximum: 0xfff,
			camera_to_xyz: [12093, -3557, -1155, -1000, 9534, 1733, -22, 1787, 4576, 0, 0, 0],
		},
	),
	(
		"Samsung GX-1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [10504, -2438, -1189, -8603, 16207, 2531, -1022, 863, 12242, 0, 0, 0],
		},
	),
	(
		"Samsung GX20",
		CameraData {
			black: 0,
			maximum: 0, /* copied from Pentax K20D */
			camera_to_xyz: [9427, -2714, -868, -7493, 16092, 1373, -2199, 3264, 7180, 0, 0, 0],
		},
	),
	(
		"Samsung S85",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [11885, -3968, -1473, -4214, 12299, 1916, -835, 1655, 5549, 0, 0, 0],
		},
	),
	(
		"Sinar",
		CameraData {
			black: 0,
			maximum: 0, /* DJC */
			camera_to_xyz: [16442, -2956, -2422, -2877, 12128, 750, -1136, 6066, 4559, 0, 0, 0],
		},
	),
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
	(
		"YI M1",
		CameraData {
			black: 0,
			maximum: 0,
			camera_to_xyz: [7712, -2059, -653, -3882, 11494, 2726, -710, 1332, 5958, 0, 0, 0],
		},
	),
];
