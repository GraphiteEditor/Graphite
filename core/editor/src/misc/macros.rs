/// Counts args in the macro invocation by adding `+ 1` for every arg.
///
/// # Example
///
/// ```ignore
/// let x = count_args!(("example1"), (10), (25));
/// assert_eq!(x, 3);
/// ```
/// expands to
/// ```ignore
/// let x = 0 + 1 + 1 + 1;
/// assert_eq!(x, 3);
/// ```
macro_rules! count_args {
	(@one $($t:tt)*) => { 1 };
	($(($($x:tt)*)),*$(,)?) => {
		0 $(+ count_args!(@one $($x)*))*
	};
}

/// Generates a [`std::collections::HashMap`] for `ToolState`'s `tools` variable.
///
/// # Example
///
/// ```ignore
/// let tools = gen_tools_hash_map! {
/// 	Select => select::Select,
/// 	Crop => crop::Crop,
/// };
/// ```
/// expands to
/// ```ignore
/// let tools = {
/// 	let mut hash_map: std::collections::HashMap<crate::tool::ToolType, Box<dyn crate::tool::Tool>> = std::collections::HashMap::with_capacity(count_args!(/* Macro args */));
///
/// 	hash_map.insert(crate::tool::ToolType::Select, Box::new(select::Select::default()));
/// 	hash_map.insert(crate::tool::ToolType::Crop, Box::new(crop::Crop::default()));
///
/// 	hash_map
/// };
/// ```
macro_rules! gen_tools_hash_map {
	($($enum_variant:ident => $struct_path:ty),* $(,)?) => {{
		let mut hash_map: ::std::collections::HashMap<$crate::tool::ToolType, ::std::boxed::Box<dyn for<'a> $crate::message_prelude::MessageHandler<$crate::tool::tool_messages::ToolMessage,$crate::tool::ToolActionHandlerData<'a>>>> = ::std::collections::HashMap::with_capacity(count_args!($(($enum_variant)),*));
		$(hash_map.insert($crate::tool::ToolType::$enum_variant, ::std::boxed::Box::new(<$struct_path>::default()));)*

		hash_map
	}};
}

/// Creates a string representation of an enum value that exactly matches the given name of each enum variant
///
/// # Example
///
/// ```ignore
/// enum E {
/// 	A(u8),
/// 	B
/// }
///
/// // this line is important
/// use E::*;
///
/// let a = E::A(7);
/// let s = match_variant_name!(match (a) { A, B });
/// ```
///
/// expands to
///
/// ```ignore
/// // ...
///
/// let s = match a {
/// 	A { .. } => "A",
/// 	B { .. } => "B"
/// };
/// ```
macro_rules! match_variant_name {
    (match ($e:expr) { $($v:ident),* $(,)? }) => {
		match $e {
			$(
				$v { .. } => stringify!($v)
			),*
		}
	};
}

/// Syntax sugar for initializing an `ActionList`
///
/// # Example
///
/// ```ignore
/// actions!(DocumentMessage::Undo, DocumentMessage::Redo);
/// ```
///
/// expands to:
/// ```ignore
/// vec![vec![DocumentMessage::Undo, DocumentMessage::Redo]];
/// ```
///
/// and
/// ```ignore
/// actions!(DocumentMessage; Undo, Redo);
/// ```
///
/// expands to:
/// ```ignore
/// vec![vec![DocumentMessage::Undo, DocumentMessage::Redo]];
/// ```
///
macro_rules! actions {
	($($v:expr),* $(,)?) => {{
		 vec![$(vec![$v.into()]),*]
	}};
	($name:ident; $($v:ident),* $(,)?) => {{
		 vec![vec![$(($name::$v).into()),*]]
	}};
}

/// Does the same thing as the `actions!` macro but wraps everything in:
///
/// ```ignore
/// fn actions(&self) -> ActionList {
///		actions!(…)
/// }
/// ```
macro_rules! advertise_actions {
	($($v:expr),* $(,)?) => {
		fn actions(&self) -> $crate::communication::ActionList {
			 actions!($($v),*)
		}
	};
	($name:ident; $($v:ident),* $(,)?) => {
		fn actions(&self) -> $crate::communication::ActionList {
			 actions!($name; $($v),*)
		}
	}
}
