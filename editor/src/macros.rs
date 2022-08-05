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
/// actions!(DocumentMessage;
///     Undo,
///     Redo,
/// );
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
///     actions!(…)
/// }
/// ```
macro_rules! advertise_actions {
	($($v:expr),* $(,)?) => {
		fn actions(&self) -> $crate::utility_traits::ActionList {
			actions!($($v),*)
		}
	};

	($name:ident; $($v:ident),* $(,)?) => {
		fn actions(&self) -> $crate::utility_traits::ActionList {
			actions!($name; $($v),*)
		}
	}
}
