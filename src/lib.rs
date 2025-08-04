#![allow(unused)]
mod btree;
mod linear;

use core::panic;

pub use linear::RangeAllocator;

#[derive(Debug)]
pub struct Error;

impl Error {
    #[track_caller]
    pub fn cause(msg: &str) -> Error {
        eprintln!("{} returning error: {msg}", panic::Location::caller());
        Error
    }

    #[track_caller]
    pub fn unimplemented() -> Error {
        Error::cause("unimplemented")
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[macro_export]
macro_rules! round_up {
    ($n:expr, $size:expr) => {{
        let n = $n;
        let size = $size;
        (n + size - 1) & (!(size - 1))
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let mut allocator: RangeAllocator<()> = RangeAllocator::new();
        let n = 8;

        allocator
            .add_range(0xa00000, 4096 * n, ())
            .expect("can add range");

        let x: Vec<_> = (0..n)
            .map(|_| allocator.alloc(4096, 4096).expect("can allocate").1)
            .collect();

        for (i, a) in x.iter().enumerate() {
            for (j, b) in x.iter().enumerate() {
                if i == j {
                    continue;
                }

                assert_ne!(a, b, "cannot allocate same region twice");
                if a < b {
                    assert!(b - a >= 4096, "cannot overlap");
                } else {
                    assert!(a - b >= 4096, "cannot overlap");
                }
            }
        }

        for base in x {
            allocator.free(base, 4096).expect("can free");
        }
    }
}
