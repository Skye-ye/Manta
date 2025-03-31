//! Adapted from Titanix

#![no_std]
#![no_main]
#![feature(negative_impls)]
#![feature(sync_unsafe_cell)]

extern crate alloc;

pub mod cell;
pub mod mutex;
