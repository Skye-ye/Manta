#![no_std]
#![no_main]

extern crate user_lib;

use user_lib::{exit, println};

#[unsafe(no_mangle)]
fn main() {
    println!("hello world");
    exit(3)
}
