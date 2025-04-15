use super::utils::register_mut_const;

pub const MAX_HARTS: usize = 4;
register_mut_const!(pub HARTS, usize, 1);
register_mut_const!(pub CLOCK_FREQ, usize, 10000000);
