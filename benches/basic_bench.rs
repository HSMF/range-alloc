use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use range_alloc::tests;

fn repeatedly_alloc_page(c: &mut Criterion) {
    let mut a = tests::setup();
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
}

criterion_group!(benches, repeatedly_alloc_page);
criterion_main!(benches);
