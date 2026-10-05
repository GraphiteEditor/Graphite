//! How a person is shown wherever the editor names one: the stand-in for someone unnamed, and the colour that
//! follows a user id through the cursors, the Session panel and the History panel.

use document_graph_storage::UserId;

/// A stand-in for a person who has not given a name, the same on every device of theirs and everywhere
/// in the room, since it comes from the user id alone.
pub(crate) fn anonymous_name(user: UserId) -> String {
	let index = (user.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as usize % ANONYMOUS_ANIMALS.len();
	format!("Anonymous {}", ANONYMOUS_ANIMALS[index])
}

pub(crate) const ANONYMOUS_ANIMALS: [&str; 64] = [
	"Alligator",
	"Anteater",
	"Armadillo",
	"Axolotl",
	"Badger",
	"Bat",
	"Beaver",
	"Buffalo",
	"Camel",
	"Capybara",
	"Chameleon",
	"Cheetah",
	"Chinchilla",
	"Chipmunk",
	"Cormorant",
	"Coyote",
	"Crow",
	"Dingo",
	"Dolphin",
	"Duck",
	"Elephant",
	"Ferret",
	"Fox",
	"Frog",
	"Giraffe",
	"Gopher",
	"Grizzly",
	"Hedgehog",
	"Heron",
	"Hippo",
	"Hyena",
	"Ibex",
	"Iguana",
	"Jackal",
	"Jaguar",
	"Kangaroo",
	"Koala",
	"Kraken",
	"Lemur",
	"Leopard",
	"Liger",
	"Llama",
	"Manatee",
	"Mink",
	"Monkey",
	"Moose",
	"Narwhal",
	"Nyan Cat",
	"Orangutan",
	"Otter",
	"Panda",
	"Penguin",
	"Platypus",
	"Python",
	"Quagga",
	"Rabbit",
	"Raccoon",
	"Rhino",
	"Sheep",
	"Shrew",
	"Skunk",
	"Squirrel",
	"Tiger",
	"Turtle",
];

/// A colour for a person derived from their user id, so every peer sees the same one without anything on the
/// wire, and one person has one colour across their tabs and in the history.
pub(crate) fn user_color(user: UserId) -> String {
	let hue = (user.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) % 360;
	let (h, s, l): (f64, f64, f64) = (hue as f64 / 60., 0.65, 0.5);
	let c = (1. - (2. * l - 1.).abs()) * s;
	let x = c * (1. - (h % 2. - 1.).abs());
	let m = l - c / 2.;
	let (r, g, b) = match h as u32 {
		0 => (c, x, 0.),
		1 => (x, c, 0.),
		2 => (0., c, x),
		3 => (0., x, c),
		4 => (x, 0., c),
		_ => (c, 0., x),
	};
	let channel = |value: f64| ((value + m) * 255.).round() as u8;
	format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b))
}
