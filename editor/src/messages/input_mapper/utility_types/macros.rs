/// Constructs a `KeyStates` bit vector and sets the bit flags for all the given modifier `Key`s.
macro_rules! modifiers {
	($($m:ident),*) => {{
		#[allow(unused_mut)]
		let mut state = KeyStates::new();
		$(
		state.set(Key::$m as usize);
		)*
		state
	}};
}

/// Builds a slice of `MappingEntry` struct(s) that are used to:
/// - ...dispatch the given `action_dispatch` as an output `Message` if its discriminant is a currently available action
/// - ...when the `InputMapperMessage` enum variant, as specified at the start and followed by a semicolon, is received
/// - ...while the optional `modifiers` being pressed.
///
/// Syntax:
/// ```rs
/// entry_for_layout!(Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message)
/// ```
///
/// The actions system controls which actions are currently available. Those are provided by the different message handlers based on the current application state and context.
/// Each handler adds or removes actions in the form of message discriminants. Here, we tie an input condition (such as a hotkey) to an action's full message.
/// When an action is currently available, and the user enters that input, the action's message is dispatched on the message bus.
macro_rules! entry {
	($input:expr; $(modifiers=[$($modifier:ident),*],)? $(refresh_keys=[$($refresh:ident),* $(,)?],)? action_dispatch=$action_dispatch:expr$(,)?) => {
		&[&[
			// Cause the `action_dispatch` message to be sent when the specified input occurs.
			MappingEntry {
				action: $action_dispatch.into(),
				input: $input,
				modifiers: modifiers!($($($modifier),*)?),
			},

			// Also cause the `action_dispatch` message to be sent when any of the specified refresh keys change.
			//
			// For example, a snapping state bound to the Shift key may change if the user presses or releases that key.
			// In that case, we want to dispatch the action's message even though the pointer didn't necessarily move so
			// the input handler can update the snapping state without making the user move the mouse to see the change.
			$(
			$(
			MappingEntry {
				action: $action_dispatch.into(),
				input: InputMapperMessage::KeyDown(Key::$refresh),
				modifiers: modifiers!(),
			},
			MappingEntry {
				action: $action_dispatch.into(),
				input: InputMapperMessage::KeyUp(Key::$refresh),
				modifiers: modifiers!(),
			},
			MappingEntry {
				action: $action_dispatch.into(),
				input: InputMapperMessage::KeyDownNoRepeat(Key::$refresh),
				modifiers: modifiers!(),
			},
			MappingEntry {
				action: $action_dispatch.into(),
				input: InputMapperMessage::KeyUpNoRepeat(Key::$refresh),
				modifiers: modifiers!(),
			},
			)*
			)*
		]]
	};
}

/// Constructs a `KeyMappingEntries` list for each input type and inserts every given entry into the list corresponding to its input type.
/// Returns a tuple of `KeyMappingEntries` in the order:
/// ```rs
/// (key_up, key_down, double_click, wheel_scroll, pointer_move)
/// ```
macro_rules! mapping {
	[$($entry:expr),* $(,)?] => {{
		let mut key_up = KeyMappingEntries::key_array();
		let mut key_down = KeyMappingEntries::key_array();
		let mut key_up_no_repeat = KeyMappingEntries::key_array();
		let mut key_down_no_repeat = KeyMappingEntries::key_array();
		let mut double_click = KeyMappingEntries::mouse_buttons_arrays();
		let mut wheel_scroll = KeyMappingEntries::new();
		let mut pointer_move = KeyMappingEntries::new();

		$(
		// Each of the many entry slices, one specified per action
		for entry_slice in $entry {
			// Each entry in the slice (usually just one, except when `refresh_keys` adds additional key entries)
			for entry in entry_slice.into_iter() {
				let corresponding_list = match entry.input {
					InputMapperMessage::KeyDown(key) => &mut key_down[key as usize],
					InputMapperMessage::KeyUp(key) => &mut key_up[key as usize],
					InputMapperMessage::KeyDownNoRepeat(key) => &mut key_down_no_repeat[key as usize],
					InputMapperMessage::KeyUpNoRepeat(key) => &mut key_up_no_repeat[key as usize],
					InputMapperMessage::DoubleClick(key) => &mut double_click[key as usize],
					InputMapperMessage::WheelScroll => &mut wheel_scroll,
					InputMapperMessage::PointerMove => &mut pointer_move,
				};
				// Push each entry to the corresponding `KeyMappingEntries` list for its input type
				corresponding_list.push(entry.clone());
			}
		}
		)*

		(key_up, key_down, key_up_no_repeat, key_down_no_repeat, double_click, wheel_scroll, pointer_move)
	}};
}

/// Constructs an `ActionKeys` macro with a certain `Action` variant, conveniently wrapped in `Some()`.
macro_rules! action_keys {
	($action:expr) => {
		Some(crate::messages::input_mapper::utility_types::misc::ActionKeys::Action($action.into()))
	};
}

pub(crate) use action_keys;
pub(crate) use entry;
pub(crate) use mapping;
pub(crate) use modifiers;
