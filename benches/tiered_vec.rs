use ::tiered_vec::{ImplicitTieredVec, TieredVec};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::SmallRng;
use rand::SeedableRng;

mod tiered_vec {
    use rand::{
        distributions::{Distribution, WeightedIndex},
        rngs::SmallRng,
        Rng,
    };
    use tiered_vec::TieredVec;

    pub fn insert(mut rng: SmallRng, tiered_vec: &mut TieredVec<usize>, num_insertions: usize) {
        let mut i = 0;

        for _ in 0..num_insertions {
            tiered_vec
                .insert(i, i)
                .expect("could not insert into tiered_vec");

            i = rng.gen_range(0..=(i + 1));
        }
    }

    pub fn update(mut rng: SmallRng, tiered_vec: &mut TieredVec<usize>, num_updates: usize) {
        for i in 0..num_updates {
            *tiered_vec
                .get_mut_by_rank(rng.gen_range(0..tiered_vec.len()))
                .unwrap() = i;
        }
    }

    pub fn delete(mut rng: SmallRng, tiered_vec: &mut TieredVec<usize>, num_deletes: usize) {
        let mut len = tiered_vec.len();

        for i in 0..num_deletes {
            let gen = rng.gen_range(0..len);

            tiered_vec
                .remove(gen)
                .expect("could not remove element at known index");

            len -= 1;
        }
    }

    pub fn random_mix(mut rng: SmallRng, tiered_vec: &mut TieredVec<usize>, num_operations: usize) {
        let mut len = tiered_vec.len();
        let mut weights = [tiered_vec.capacity() - len, len, len];
        let mut dist = WeightedIndex::new(&weights).unwrap();

        for _ in 0..num_operations {
            match dist.sample(&mut rng) {
                // insert
                0 => {
                    let i = rng.gen_range(0..=len);

                    tiered_vec
                        .insert(i, i)
                        .expect("could not insert into tiered_vec");

                    len += 1;
                }

                // update
                1 => {
                    let i = rng.gen_range(0..len);

                    *tiered_vec.get_mut_by_rank(i).unwrap() = i + 1;
                }

                // delete
                2 => {
                    let i = rng.gen_range(0..len);

                    tiered_vec
                        .remove(i)
                        .expect("could not remove element at known index");
                    len -= 1;
                }

                _ => unreachable!(),
            }

            weights = [tiered_vec.capacity() - len, len, len];
            dist = WeightedIndex::new(&weights).unwrap();
        }
    }
}

mod implicit_tiered_vec {
    use rand::{
        distributions::{Distribution, WeightedIndex},
        rngs::SmallRng,
        Rng,
    };
    use tiered_vec::ImplicitTieredVec;

    pub fn insert(
        mut rng: SmallRng,
        tiered_vec: &mut ImplicitTieredVec<usize>,
        num_insertions: usize,
    ) {
        let mut i = 0;

        for _ in 0..num_insertions {
            tiered_vec
                .insert(i, i)
                .expect("could not insert into tiered_vec");

            i = rng.gen_range(0..=(i + 1));
        }
    }

    pub fn update(
        mut rng: SmallRng,
        tiered_vec: &mut ImplicitTieredVec<usize>,
        num_updates: usize,
    ) {
        for i in 0..num_updates {
            *tiered_vec
                .get_mut_by_rank(rng.gen_range(0..tiered_vec.len()))
                .unwrap() = i;
        }
    }

    pub fn delete(
        mut rng: SmallRng,
        tiered_vec: &mut ImplicitTieredVec<usize>,
        num_deletes: usize,
    ) {
        let mut len = tiered_vec.len();

        for i in 0..num_deletes {
            let gen = rng.gen_range(0..len);

            tiered_vec
                .remove(gen)
                .expect("could not remove element at known index");

            len -= 1;
        }
    }

    pub fn random_mix(
        mut rng: SmallRng,
        tiered_vec: &mut ImplicitTieredVec<usize>,
        num_operations: usize,
    ) {
        let mut len = tiered_vec.len();
        let mut weights = [tiered_vec.capacity() - len, len, len];
        let mut dist = WeightedIndex::new(&weights).unwrap();

        for _ in 0..num_operations {
            match dist.sample(&mut rng) {
                // insert
                0 => {
                    let i = rng.gen_range(0..=len);

                    tiered_vec
                        .insert(i, i)
                        .expect("could not insert into tiered_vec");

                    len += 1;
                }

                // update
                1 => {
                    let i = rng.gen_range(0..len);

                    *tiered_vec.get_mut_by_rank(i).unwrap() = i + 1;
                }

                // delete
                2 => {
                    let i = rng.gen_range(0..len);

                    tiered_vec
                        .remove(i)
                        .expect("could not remove element at known index");
                    len -= 1;
                }

                _ => unreachable!(),
            }

            weights = [tiered_vec.capacity() - len, len, len];
            dist = WeightedIndex::new(&weights).unwrap();
        }
    }
}

mod vec {
    use rand::{
        distributions::{Distribution, WeightedIndex},
        rngs::SmallRng,
        Rng,
    };

    pub fn insert(mut rng: SmallRng, vec: &mut Vec<usize>, num_insertions: usize) {
        let mut i = 0;

        for _ in 0..num_insertions {
            vec.insert(i, i);

            i = rng.gen_range(0..=(i + 1));
        }
    }

    pub fn update(mut rng: SmallRng, vec: &mut Vec<usize>, num_updates: usize) {
        let len = vec.len();

        for i in 0..num_updates {
            *vec.get_mut(rng.gen_range(0..len)).unwrap() = i;
        }
    }

    pub fn delete(mut rng: SmallRng, vec: &mut Vec<usize>, num_deletes: usize) {
        let mut len = vec.len();

        for _ in 0..num_deletes {
            vec.remove(rng.gen_range(0..len));
            len -= 1;
        }
    }

    pub fn random_mix(mut rng: SmallRng, vec: &mut Vec<usize>, num_operations: usize) {
        let mut len = vec.len();
        let mut weights = [vec.capacity() - len, len, len];
        let mut dist = WeightedIndex::new(&weights).unwrap();

        for _ in 0..num_operations {
            match dist.sample(&mut rng) {
                // insert
                0 => {
                    let i = rng.gen_range(0..=len);
                    vec.insert(i, i);

                    len += 1;
                }

                // update
                1 => {
                    let i = rng.gen_range(0..=len);
                    *vec.get_mut(rng.gen_range(0..len)).unwrap() = i + 1;
                }

                // delete
                2 => {
                    vec.remove(rng.gen_range(0..len));
                    len -= 1;
                }

                _ => unreachable!(),
            }

            weights = [vec.capacity() - len, len, len];
            dist = WeightedIndex::new(&weights).unwrap();
        }
    }
}

fn bench_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("Insertion");

    let vector_size: usize = 100_000;
    let mut tv = TieredVec::with_minimum_capacity(vector_size);
    let mut itv = ImplicitTieredVec::new(tv.tier_size());
    let mut v: Vec<_> = Vec::with_capacity(tv.capacity());

    group.bench_function("Vec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::insert(black_box(rng), black_box(&mut v), vector_size);
        })
    });

    group.bench_function("TieredVec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            tiered_vec::insert(black_box(rng), black_box(&mut tv), vector_size);
        })
    });

    group.bench_function("ImplicitTieredVec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            implicit_tiered_vec::insert(black_box(rng), black_box(&mut itv), vector_size);
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

fn bench_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("Update");

    let vector_size: usize = 100_000;
    let mut tv = TieredVec::with_minimum_capacity(vector_size);
    let mut itv = ImplicitTieredVec::new(tv.tier_size());
    let mut v: Vec<_> = Vec::with_capacity(tv.capacity());

    for i in 0..vector_size {
        v.insert(i, i);
        tv.insert(i, i).expect("could not insert into tiered_vec");
        itv.insert(i, i).expect("could not insert into tiered_vec");
    }

    group.bench_function("Vec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::update(black_box(rng), black_box(&mut v), vector_size / 2);
        })
    });

    group.bench_function("Vec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::update(black_box(rng), black_box(&mut v), vector_size * 3 / 4);
        })
    });

    group.bench_function("TieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            tiered_vec::update(black_box(rng), black_box(&mut tv), vector_size / 2);
        })
    });

    group.bench_function("TieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            tiered_vec::update(black_box(rng), black_box(&mut tv), vector_size * 3 / 4);
        })
    });

    group.bench_function("ImplicitTieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            implicit_tiered_vec::update(black_box(rng), black_box(&mut itv), vector_size / 2);
        })
    });

    group.bench_function("ImplicitTieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            implicit_tiered_vec::update(black_box(rng), black_box(&mut itv), vector_size * 3 / 4);
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("Delete");

    let vector_size: usize = 10_000;
    let mut tv = TieredVec::with_minimum_capacity(vector_size);
    let mut itv = ImplicitTieredVec::new(tv.tier_size());
    let mut v: Vec<_> = Vec::with_capacity(tv.capacity());

    for i in 0..vector_size {
        v.insert(i, i);
        tv.insert(i, i).expect("could not insert into tiered_vec");
        itv.insert(i, i).expect("could not insert into tiered_vec");
    }

    group.bench_function("Vec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::delete(black_box(rng), black_box(&mut v.clone()), vector_size / 2);
        })
    });

    group.bench_function("Vec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::delete(
                black_box(rng),
                black_box(&mut v.clone()),
                vector_size * 3 / 4,
            );
        })
    });

    group.bench_function("TieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            tiered_vec::delete(black_box(rng), black_box(&mut tv.clone()), vector_size / 2);
        })
    });

    group.bench_function("TieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            tiered_vec::delete(
                black_box(rng),
                black_box(&mut tv.clone()),
                vector_size * 3 / 4,
            );
        })
    });

    group.bench_function("ImplicitTieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            implicit_tiered_vec::delete(
                black_box(rng),
                black_box(&mut itv.clone()),
                vector_size / 2,
            );
        })
    });

    group.bench_function("ImplicitTieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            implicit_tiered_vec::delete(
                black_box(rng),
                black_box(&mut itv.clone()),
                vector_size * 3 / 4,
            );
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

fn bench_random_mix(c: &mut Criterion) {
    let mut group = c.benchmark_group("Random Mix");

    let vector_size: usize = 50;
    let tv: TieredVec<usize> = TieredVec::with_minimum_capacity(vector_size);
    let itv: ImplicitTieredVec<usize> = ImplicitTieredVec::new(tv.tier_size());
    let mut v: Vec<usize> = Vec::with_capacity(tv.capacity());

    group.bench_function("Vec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::random_mix(black_box(rng), black_box(&mut v), 1_000_000);
        })
    });

    group.bench_function("TieredVec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            tiered_vec::random_mix(black_box(rng), black_box(&mut tv.clone()), 1_000_000);
        })
    });

    group.bench_function("ImplicitTieredVec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            implicit_tiered_vec::random_mix(black_box(rng), black_box(&mut itv.clone()), 1_000_000);
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

criterion_group!(benches, bench_random_mix);
criterion_main!(benches);
