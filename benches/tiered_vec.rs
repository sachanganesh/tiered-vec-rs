use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};
use tiered_vec::TieredVec;

fn insert_tiered_vec(mut rng: SmallRng, num_insertions: usize) {
    let mut tiered_vec = TieredVec::new(num_insertions.next_power_of_two());

    for i in 0..num_insertions {
        tiered_vec
            .insert(i, rng.next_u64() as usize)
            .expect("could not insert into tiered_vec");
    }
}

fn insert_vec(mut rng: SmallRng, num_insertions: usize) {
    let mut vec = Vec::with_capacity(num_insertions);

    for i in 0..num_insertions {
        vec.insert(i, rng.next_u64() as usize);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("insert TieredVec 1_000", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            insert_tiered_vec(rng, 1_000);
        })
    });

    c.bench_function("insert Vec 1_000", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            insert_vec(rng, 1_000);
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
