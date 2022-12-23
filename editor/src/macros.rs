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

// Inspired by https://github.com/jquesada2016/clone-macro-rs
macro_rules! clone {
    () => {};
    ([$($tt:tt)*], $expr:expr) => {{
        clone!($($tt)*);
        $expr
    }};
    ($(,)? mut $ident:ident $($tt:tt)*) => {
        let mut $ident = ::core::clone::Clone::clone(&$ident);
        clone!($($tt)*);
    };
    ($(,)? $ident:ident $($tt:tt)*) => {
        let $ident = ::core::clone::Clone::clone(&$ident);
        clone!($($tt)*);
    };
    ($(,)?) => {};
}

macro_rules! widget_callback {
	([$($tt:tt)*], $expr:expr) => {{
        crate::messages::layout::utility_types::layout_widget::clone!($($tt)*);
        WidgetCallback::new($expr)
    }};
	($expr:expr) => {{
        WidgetCallback::new($expr)
    }};
}
