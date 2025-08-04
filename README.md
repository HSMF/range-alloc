Experimental range allocation implementation for barrelfish

Capability allocation and paging allocations on a high level both are "given this set of ranges, allocate a range contained by them that satisfies certain constraints"

Possible constraints are
- size
- alignment
- a range in which the allocated range must lie

Additionally, the same range (or an overlapping part) must not be allocated again before it has been freed.


## Development

It is probably a good idea to run tests with miri

```sh
cargo +nightly miri test
```


Benchmarks
```sh
cargo bench
```
