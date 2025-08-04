use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use range_alloc::RangeAllocator;

fn repeatedly_alloc_page(c: &mut Criterion) {
    let mut a: RangeAllocator<()> = RangeAllocator::new();
    a.add_range(0x7ff000, 4096 * 4096, ())
        .expect("can add range");

    a.add_range(0xfff0000, 4096 * 128, ())
        .expect("can add range");
    c.bench_function("fib 20", |b| {
        b.iter(|| {
            let (_, x) = a.alloc(black_box(4096), 4096).expect("can allocate");
            a.free(x, 4096).expect("can free again");
        })
    });
}

criterion_group!(benches, repeatedly_alloc_page);
criterion_main!(benches);
