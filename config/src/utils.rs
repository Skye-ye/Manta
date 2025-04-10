macro_rules! register_mut_const {
    ($(#[$meta:meta])*$name:ident, $type:ty, $value:expr) => {
        $(#[$meta])*
        static mut $name: $type = $value;
        paste::paste! {
            $(#[$meta])*
            pub fn [<$name:lower>]() -> $type {
                unsafe { $name }
            }
        }
        paste::paste! {
            pub fn [<set_ $name:lower>](num: $type) {
                unsafe {
                    $name = num;
                }
            }
        }
    };
    ($(#[$meta:meta])*pub $name:ident, $type:ty, $value:expr) => {
        $(#[$meta])*
        pub static mut $name: $type = $value;
        paste::paste! {
            $(#[$meta])*
            pub fn [<$name:lower>]() -> $type {
                unsafe { $name }
            }
        }
        paste::paste! {
            pub fn [<set_ $name:lower>](num: $type) {
                unsafe {
                    $name = num;
                }
            }
        }
    };
    () => {};
}

pub(super) use register_mut_const;
