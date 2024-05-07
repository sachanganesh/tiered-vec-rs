use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};
use tiered_vec::TieredVec;

fn insert_tiered_vec(mut rng: SmallRng, num_insertions: usize) {
    let mut tiered_vec = TieredVec::new(num_insertions.next_power_of_two());

    let mut i = 0;

    for _ in 0..num_insertions {
        tiered_vec
            .insert(i, i)
            .expect("could not insert into tiered_vec");

        i = rng.gen_range(0..=(i + 1));
    }
}

fn insert_vec(mut rng: SmallRng, num_insertions: usize) {
    let mut vec = Vec::with_capacity(num_insertions);

    let mut i = 0;

    for _ in 0..num_insertions {
        vec.insert(i, i);

        i = rng.gen_range(0..=(i + 1));
    }
}

fn bench_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("Insertion");

    group.bench_function("Vec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            insert_vec(rng, 100_000);
        })
    });

    group.bench_function("TieredVec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            insert_tiered_vec(rng, 100_000);
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

criterion_group!(benches, bench_insertion);
criterion_main!(benches);
