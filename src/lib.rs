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

// #[cfg(test)]
pub mod tests {
    use std::hint::black_box;

    use super::*;

    pub fn setup() -> RangeAllocator<()> {
        let mut a: RangeAllocator<()> = RangeAllocator::new();
        a.add_range(0x7ff000, 4096 * 4096, ())
            .expect("can add range");

        a.add_range(0xfff0000, 4096 * 128, ())
            .expect("can add range");

        a
    }

    pub fn alloc_aligned(a: &mut RangeAllocator<()>) {
        let (_, x) = a.alloc(black_box(4096), 4096 * 4096).expect("can allocate");
        let (_, y) = a.alloc(black_box(4096), 4096 * 4096).expect("can allocate");
        a.free(x, 4096).expect("can free again");
        a.free(y, 4096).expect("can free again");
    }

    pub fn alloc_different_configurations(a: &mut RangeAllocator<()>) {
        const N: usize = 50;

        let sizes = [10, 3, 5, 6, 2, 9, 1, 4, 8, 7].map(|x| x * 4096);
        let alignments = [8, 2, 9, 1, 3, 0, 6, 5, 7].map(|x| 4096 << x);

        let mut sizes = sizes.iter().copied().cycle();
        let mut alignments = alignments.iter().copied().cycle();

        let mut positions = [(0, 0); N];

        for pos in positions.iter_mut() {
            let size = sizes.next().unwrap();
            let (_, x) = a
                .alloc(size, alignments.next().unwrap())
                .expect("can allocate");

            *pos = (x, size);
        }

        for pos in positions {
            a.free(pos.0, pos.1).expect("can free");
        }
    }

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

    #[test]
    fn alloc_2_aligned() {
        let mut a = setup();
        tests::alloc_aligned(&mut a);
    }

    #[test]
    fn test_alloc_different_configurations() {
        let mut a = setup();
        tests::alloc_different_configurations(&mut a);
    }
}
