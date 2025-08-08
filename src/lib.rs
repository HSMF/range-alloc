#![allow(unused)]
mod btree;
pub mod collections;
mod linear;

use core::panic;

pub use linear::RangeAllocator;

pub trait RangeAlloc {
    type Tag;
    fn add_range(&mut self, base: usize, size: usize, range_tag: Self::Tag) -> Result<()>;

    fn alloc(&mut self, min_size: usize, alignment: usize) -> Result<(Self::Tag, usize)>;

    fn free(&mut self, base: usize, size: usize) -> Result<()>;

    fn total_space(&self) -> usize;

    fn space(&self) -> usize;
}

// TODO: this should obviously not just be a string. For sketching this is enough
#[derive(Debug)]
pub struct Error(String);

impl Error {
    #[track_caller]
    pub fn cause(msg: &str) -> Error {
        Error(format!(
            "{} returning error: {msg}",
            panic::Location::caller()
        ))
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
    use std::{
        collections::{HashMap, HashSet},
        hint::black_box,
        ops::Range,
    };

    use super::*;

    pub fn new_linear() -> linear::RangeAllocator<()> {
        linear::RangeAllocator::new()
    }

    pub fn new_btree() -> btree::RangeAllocator<()> {
        btree::RangeAllocator::new()
    }

    pub fn setup(a: &mut impl RangeAlloc<Tag = ()>) {
        a.add_range(0x7ff000, 4096 * 4096, ())
            .expect("can add range");

        a.add_range(0xfff0000, 4096 * 128, ())
            .expect("can add range");
    }

    /// allocate n times from a, picking the sizes and alignments from `sizes` and `alignments`
    /// both iterators are made to wrap around
    pub fn allocate_n(
        a: &mut impl RangeAlloc<Tag = ()>,
        sizes: impl Iterator<Item = usize> + Clone,
        alignments: impl Iterator<Item = usize> + Clone,
        n: usize,
    ) -> Vec<(usize, usize)> {
        let mut sizes = sizes.cycle();
        let mut alignments = alignments.cycle();

        let mut positions = Vec::with_capacity(n);
        for _ in 0..n {
            let size = sizes.next().unwrap();
            let Ok((_, x)) = a.alloc(size, alignments.next().unwrap()) else {
                continue;
            };

            positions.push((x, size));
        }

        positions
    }

    pub fn alloc_aligned(a: &mut impl RangeAlloc<Tag = ()>) {
        let (_, x) = a.alloc(black_box(4096), 4096 * 4096).expect("can allocate");
        let (_, y) = a.alloc(black_box(4096), 4096 * 4096).expect("can allocate");
        a.free(x, 4096).expect("can free again");
        a.free(y, 4096).expect("can free again");
    }

    pub fn alloc_different_configurations(a: &mut impl RangeAlloc<Tag = ()>) {
        const N: usize = 500;

        let sizes = [10, 3, 5, 6, 2, 9, 1, 4, 8, 7].map(|x| x * 4096);
        let alignments = [8, 2, 9, 1, 3, 0, 6, 5, 7].map(|x| 4096 << x);

        let mut sizes = sizes.iter().copied().cycle();
        let mut alignments = alignments.iter().copied().cycle();

        let mut positions = [(0, 0); N];

        for pos in positions.iter_mut() {
            let size = sizes.next().unwrap();
            let Ok((_, x)) = a.alloc(size, alignments.next().unwrap()) else {
                continue;
            };
            // .expect("can allocate");

            *pos = (x, size);
        }

        for pos in positions {
            if pos.0 == 0 {
                continue;
            }
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

    macro_rules! both_tests {
        ($linear:ident, $btree:ident, $a:ident => $case:expr) => {
            #[test]
            fn $linear() {
                let mut $a: linear::RangeAllocator<()> = new_linear();
                $case
            }

            #[test]
            fn $btree() {
                let mut $a: btree::RangeAllocator<()> = new_btree();
                $case
            }
        };
    }

    both_tests!(linear_alloc_2_aligned, btree_alloc_2_aligned, a => {
        setup(&mut a);
        tests::alloc_aligned(&mut a);
    });

    both_tests!(linear_alloc_different_configurations, btree_alloc_different_configurations, a => {
        setup(&mut a);
        tests::alloc_different_configurations(&mut a);
    });

    fn overlap(a: (usize, usize), b: (usize, usize)) -> bool {
        if a.0 < b.0 {
            a.0 + a.1 > b.0
        } else {
            b.0 + b.1 > a.0
        }
    }

    fn run_trace(mut a: impl RangeAlloc<Tag = u64>, trace: &str) {
        // add <region-id> <start> <size>
        // alloc <allocation-id> <size> <alignment> fail
        // free <allocation-id>

        let mut regions = HashSet::new();
        let mut allocations = HashMap::new();

        let mut errors = 0;
        let mut error = |msg: &dyn std::fmt::Display| {
            errors += 1;
            eprintln!("{msg}")
        };

        for line in trace.lines() {
            let mut l = line.split_whitespace();
            let Some(cmd) = l.next() else { continue };

            macro_rules! next_int {
                (let $f:ident <- $l:expr) => {
                    let $f = $l
                        .next()
                        .expect(concat!("has ", stringify!($f)))
                        .parse::<u64>()
                        .expect(concat!(stringify!($f), " is u64"));
                };
            }

            match cmd {
                "add" => {
                    next_int!(let region_id <- l);
                    next_int!(let start <- l);
                    next_int!(let size <- l);
                    assert!(regions.insert(region_id), "duplicate region id {region_id}");

                    a.add_range(
                        start.try_into().unwrap(),
                        size.try_into().unwrap(),
                        region_id,
                    );
                }
                "alloc" => {
                    next_int!(let allocation_id <- l);
                    next_int!(let size <- l);
                    next_int!(let alignment <- l);
                    let fail = l.next().is_some_and(|x| x == "fail");

                    let size = size.try_into().unwrap();
                    // maybe instead of failing on error we should keep going, it can be caused by
                    // a suboptimal allocator, which is not necessarily incorrect
                    match a.alloc(size, alignment.try_into().unwrap()) {
                        Err(_) if fail => {}
                        Err(e) => error(&"unexpected error {e:?} {line}"),
                        Ok(_) if fail => error(&"did not expect to succeed"),
                        Ok((tag, base)) => {
                            assert!(
                                regions.contains(&tag),
                                "tag {tag} was not added to allocator"
                            );
                            allocations.insert(allocation_id, (base, size));

                            for (id, &other) in allocations.iter() {
                                if *id == allocation_id {
                                    continue;
                                }
                                assert!(!overlap((base, size), other));
                            }
                        }
                    }
                }
                "free" => {
                    next_int!(let allocation_id <- l);

                    let Some((base, size)) = allocations.remove(&allocation_id) else {
                        continue;
                    };

                    a.free(base, size).expect("can free");
                }
                _ => core::panic!("unknown command {cmd}"),
            }
        }
    }

    macro_rules! trace_test {
        ($trace:ident) => {
            #[test]
            fn $trace() {
                let a = linear::RangeAllocator::new();
                run_trace(a, include_str!(concat!("testdata/", stringify!($trace))))
            }
        };
    }

    trace_test!(basic_trace);
    trace_test!(gen1);
    trace_test!(gen2);
}
