use pprof::criterion::{Output, PProfProfiler};
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use range_alloc::{RangeAlloc, tests};

fn repeatedly_alloc_page(c: &mut Criterion) {
    let mut a = tests::new_linear();
    tests::setup(&mut a);

    c.bench_function("alloc-and-immediately-free", |b| {
        b.iter(|| {
            let (_, x) = a.alloc(black_box(4096), 4096).expect("can allocate");
            a.free(x, 4096).expect("can free again");
        })
    });

    c.bench_function("alloc-2-and-immediately-free", |b| {
        b.iter(|| {
            let (_, x) = a.alloc(black_box(4096), 4096).expect("can allocate");
            let (_, y) = a.alloc(black_box(4096), 4096).expect("can allocate");
            a.free(x, 4096).expect("can free again");
            a.free(y, 4096).expect("can free again");
        })
    });

    c.bench_function("alloc-aligned-2-and-immediately-free", |b| {
        b.iter(|| {
            tests::alloc_aligned(&mut a);
        })
    });

    c.bench_function("alloc_different_configurations", |b| {
        b.iter(|| {
            tests::alloc_different_configurations(&mut a);
        });
    });

    macro_rules! compare {
        ($group:expr, $alloc:ident => $setup:expr ; $e:expr) => {{
            let mut group = c.benchmark_group($group);

            let mut $alloc = tests::new_linear();
            $setup;
            group.bench_function(BenchmarkId::new("linear", 1), |b| {
                b.iter(|| $e);
            });

            let mut $alloc = tests::new_btree();
            $setup;
            group.bench_function(BenchmarkId::new("btree", 1), |b| {
                b.iter(|| $e);
            });
        }};
    }

    compare!("alloc_different_configurations", a => tests::setup(&mut a); tests::alloc_different_configurations(&mut a));
    compare!("alloc_aligned", a => {
        a.add_range(0x7ff000, 4096 * 4096, ())
            .expect("can add range");

        a.add_range(0xfff0000, 4096 * 128, ())
            .expect("can add range");

        a.add_range(0xffff0000, 4096 * 4096 * 4096, ())
            .expect("can add range");
        let alignments = [8, 2, 9, 1, 3, 0, 6, 5, 7].map(|x| 4096 << x);
        tests::allocate_n(&mut a, std::iter::once(4096), alignments.into_iter(), 50000);
        // panic!("{:?} / {:?}", a.space(), a.total_space());
    }; tests::alloc_aligned(&mut a));
}

criterion_group!(
    name=benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets=repeatedly_alloc_page

);
criterion_main!(benches);
