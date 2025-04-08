//! Any one import this crate should import `paste` crate as well.

#![no_std]
#![no_main]

#[macro_export]
// macro to generate methods to visit the fields of a struct with a lock
macro_rules! with_methods {
    ($($name:ident : $ty:ty),+) => {  // match name:type
        paste::paste! {
            $(
                pub fn [<with_ $name>]<T>(&self, f: impl FnOnce(&$ty) -> T) -> T {
                    f(&self.$name.lock())
                }
                pub fn [<with_mut_ $name>]<T>(&self, f: impl FnOnce(&mut $ty) -> T) -> T {
                    f(&mut self.$name.lock())
                }
            )+
        }
    };
}
