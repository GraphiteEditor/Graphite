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
/// - ...while any further conditions are met, like the optional `modifiers` being pressed or `layout` matching the OS.
///
/// Syntax:
/// ```rs
/// entry_for_layout!(Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message, layout: Option<KeyboardPlatformLayout>)
/// ```
///
/// To avoid having to specify the final `layout` argument, instead use the wrapper macros: [entry]!, [standard]!, and [mac]!.
/// The former sets the layout to `None` which means the key mapping is layout-agnostic and compatible with all platforms.
///
/// The actions system controls which actions are currently available. Those are provided by the different message handlers based on the current application state and context.
/// Each handler adds or removes actions in the form of message discriminants. Here, we tie an input condition (such as a hotkey) to an action's full message.
/// When an action is currently available, and the user enters that input, the action's message is dispatched on the message bus.
macro_rules! entry_for_layout {
	($input:expr; $(modifiers=[$($modifier:ident),*],)? $(refresh_keys=[$($refresh:ident),* $(,)?],)? action_dispatch=$action_dispatch:expr,$(,)? layout=$layout:expr) => {
		&[
			// Cause the `action_dispatch` message to be sent when the specified input occurs.
			MappingEntry {
				action: $action_dispatch.into(),
				input: $input,
				modifiers: modifiers!($($($modifier),*)?),
				platform_layout: $layout,
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
				platform_layout: $layout,
			},
			MappingEntry {
				action: $action_dispatch.into(),
				input: InputMapperMessage::KeyUp(Key::$refresh),
				modifiers: modifiers!(),
				platform_layout: $layout,
			},
			)*
			)*
		]
	};
}

/// Wraps [entry_for_layout]! and calls it with an agnostic (`None`) keyboard platform `layout` to avoid having to specify that argument.
///
/// Syntax:
/// ```rs
/// entry!(Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message)
/// ```
macro_rules! entry {
	($($arg:tt)*) => {
		&[entry_for_layout!($($arg)*, layout=None)]
	};
}

/// Wraps [entry_for_layout]! and calls it with a `Standard` keyboard platform `layout` to avoid having to specify that argument.
///
/// Syntax:
/// ```rs
/// standard!(Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message)
/// ```
macro_rules! standard {
	($($arg:tt)*) => {
		entry_for_layout!($($arg)*, layout=Some(KeyboardPlatformLayout::Standard))
	};
}

/// Wraps [entry_for_layout]! and calls it with a `Mac` keyboard platform `layout` to avoid having to specify that argument.
///
/// Syntax:
/// ```rs
/// mac_only!(Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message)
/// ```
macro_rules! mac_only {
	($($arg:tt)*) => {
		entry_for_layout!($($arg)*, layout=Some(KeyboardPlatformLayout::Mac))
	};
}

/// Groups multiple related entries for different platforms.
/// When a keyboard shortcut is not platform-agnostic, this should be used to contain a [mac]! and/or [standard]! entry.
///
/// Syntax:
///
/// ```rs
/// entry_multiplatform!(
///     standard!(Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message),
///     mac_only!(Key; modifiers?: Key[], refresh_keys?: Key[], action_dispatch: Message),
/// )
/// ```
macro_rules! entry_multiplatform {
	{$($arg:expr),*,} => {
		&[$($arg ),*]
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
		let mut double_click = KeyMappingEntries::new();
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
					InputMapperMessage::DoubleClick => &mut double_click,
					InputMapperMessage::WheelScroll => &mut wheel_scroll,
					InputMapperMessage::PointerMove => &mut pointer_move,
				};
				// Push each entry to the corresponding `KeyMappingEntries` list for its input type
				corresponding_list.push(entry.clone());
			}
		}
		)*

		(key_up, key_down, double_click, wheel_scroll, pointer_move)
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
pub(crate) use entry_for_layout;
pub(crate) use entry_multiplatform;
pub(crate) use mac_only;
pub(crate) use mapping;
pub(crate) use modifiers;
pub(crate) use standard;
