macro_rules! count_args {
    (@one $($t:tt)*) => { 1 };
    ($(($($x:tt)*)),*$(,)?) => {
        0 $(+ count_args!(@one $($x)*))*
    };
}

macro_rules! gen_tools_hash_map {
	($($enum_variant:ident => $struct_path:ty),* $(,)?) => {{
        let mut hash_map: ::std::collections::HashMap<$crate::tools::ToolType, Box<dyn $crate::tools::Tool>> = ::std::collections::HashMap::with_capacity(count_args!($(($enum_variant)),*));
        $(hash_map.insert($crate::tools::ToolType::$enum_variant, Box::new(<$struct_path>::default()));)*

        hash_map
    }};
}
