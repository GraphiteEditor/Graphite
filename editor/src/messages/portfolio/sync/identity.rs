//! A person's colour and stand-in name. Both derive from the user id, so all peers show the same ones.

use document_graph_storage::UserId;

fn scramble(user: UserId) -> u64 {
	user.0.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

pub(crate) fn anonymous_name(user: UserId) -> String {
	let index = (scramble(user) >> 40) as usize % ANONYMOUS_ANIMALS.len();
	format!("Anonymous {}", ANONYMOUS_ANIMALS[index])
}

const ANONYMOUS_ANIMALS: [&str; 64] = [
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
