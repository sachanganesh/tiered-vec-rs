use ::tiered_vec::{FlatTieredVec, LinkedTieredVec};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::SmallRng;
use rand::SeedableRng;

// fixed size 1000, 10_000, 1 mil, 10 mil, 100 mil
//

mod linked_tiered_vec {
    use rand::{
        distributions::{Distribution, WeightedIndex},
        rngs::SmallRng,
        Rng,
    };
    use tiered_vec::LinkedTieredVec;

    pub fn insert(
        mut rng: SmallRng,
        tiered_vec: &mut LinkedTieredVec<usize>,
        num_insertions: usize,
    ) {
        let mut i = 0;

        for j in 0..num_insertions {
            tiered_vec.insert(i, i);

            i = rng.gen_range(0..=(j + 1));
        }
    }

    pub fn insert_at(index: usize, tiered_vec: &mut LinkedTieredVec<usize>, num_insertions: usize) {
        for _ in 0..num_insertions {
            tiered_vec.insert(index, index);
        }
    }

    pub fn update(mut rng: SmallRng, tiered_vec: &mut LinkedTieredVec<usize>, num_updates: usize) {
        for i in 0..num_updates {
            *tiered_vec
                .get_mut(rng.gen_range(0..tiered_vec.len()))
                .unwrap() = i;
        }
    }

    pub fn delete(mut rng: SmallRng, tiered_vec: &mut LinkedTieredVec<usize>, num_deletes: usize) {
        let mut len = tiered_vec.len();

        for _ in 0..num_deletes {
            let gen = rng.gen_range(0..len);

            tiered_vec.remove(gen);

            len -= 1;
        }
    }

    pub fn delete_at(index: usize, tiered_vec: &mut LinkedTieredVec<usize>, num_insertions: usize) {
        for _ in 0..num_insertions {
            tiered_vec.remove(index);
        }
    }

    pub fn random_mix(
        mut rng: SmallRng,
        tiered_vec: &mut LinkedTieredVec<usize>,
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

                    tiered_vec.insert(i, i);

                    len += 1;
                }

                // update
                1 => {
                    let i = rng.gen_range(0..len);

                    *tiered_vec.get_mut(i).unwrap() = i + 1;
                }

                // delete
                2 => {
                    let i = rng.gen_range(0..len);

                    tiered_vec.remove(i);
                    len -= 1;
                }

                _ => unreachable!(),
            }

            weights = [tiered_vec.capacity() - len, len, len];
            dist = WeightedIndex::new(&weights).unwrap();
        }
    }
}

mod flat_tiered_vec {
    use rand::{
        distributions::{Distribution, WeightedIndex},
        rngs::SmallRng,
        Rng,
    };
    use tiered_vec::FlatTieredVec;

    pub fn insert(mut rng: SmallRng, tiered_vec: &mut FlatTieredVec<usize>, num_insertions: usize) {
        let mut i = 0;

        for j in 0..num_insertions {
            tiered_vec.insert(i, i);

            i = rng.gen_range(0..=(j + 1));
        }
    }

    pub fn insert_at(index: usize, tiered_vec: &mut FlatTieredVec<usize>, num_insertions: usize) {
        for _ in 0..num_insertions {
            tiered_vec.insert(index, index);
        }
    }

    pub fn update(mut rng: SmallRng, tiered_vec: &mut FlatTieredVec<usize>, num_updates: usize) {
        let len = tiered_vec.len();

        for i in 0..num_updates {
            tiered_vec[rng.gen_range(0..len)] = i;
        }
    }

    pub fn delete(mut rng: SmallRng, tiered_vec: &mut FlatTieredVec<usize>, num_deletes: usize) {
        let mut len = tiered_vec.len();

        for _ in 0..num_deletes {
            let gen = rng.gen_range(0..len);

            tiered_vec.remove(gen);

            len -= 1;
        }
    }

    pub fn delete_at(index: usize, tiered_vec: &mut FlatTieredVec<usize>, num_insertions: usize) {
        for _ in 0..num_insertions {
            tiered_vec.remove(index);
        }
    }

    pub fn random_mix(
        mut rng: SmallRng,
        tiered_vec: &mut FlatTieredVec<usize>,
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

                    tiered_vec.insert(i, i);

                    len += 1;
                }

                // update
                1 => {
                    let i = rng.gen_range(0..len);

                    *tiered_vec.get_mut(i).unwrap() = i + 1;
                }

                // delete
                2 => {
                    let i = rng.gen_range(0..len);

                    tiered_vec.remove(i);
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

        for j in 0..num_insertions {
            vec.insert(i, i);

            i = rng.gen_range(0..=(j + 1));
        }
    }

    pub fn insert_at(index: usize, vec: &mut Vec<usize>, num_insertions: usize) {
        for _ in 0..num_insertions {
            vec.insert(index, index)
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

    pub fn delete_at(index: usize, vec: &mut Vec<usize>, num_insertions: usize) {
        for _ in 0..num_insertions {
            vec.remove(index);
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

fn bench_insert_worst(c: &mut Criterion) {
    let starting_size = 1_000;
    let vec_sizes = [1_000, 10_000, 100_000];

    for vec_size in vec_sizes {
        let mut group = c.benchmark_group(format!("Insertion Worst Case {}", vec_size));

        let mut tv = LinkedTieredVec::with_capacity(vec_size);
        let mut ftv: FlatTieredVec<usize> = FlatTieredVec::new(tv.tier_capacity());
        let mut v: Vec<usize> = Vec::with_capacity(tv.capacity());

        for i in 0..starting_size {
            tv.insert(i, i);
            ftv.insert(i, i);
            v.insert(i, i);
        }

        group.bench_function("Vec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                vec::insert_at(0, black_box(&mut v), vec_size);
            })
        });

        group.bench_function("LinkedTieredVec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                linked_tiered_vec::insert_at(0, black_box(&mut tv), vec_size);
            })
        });

        group.bench_function("FlatTieredVec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                flat_tiered_vec::insert_at(0, black_box(&mut ftv), vec_size);
            })
        });

        // It's recommended to call group.finish() explicitly at the end, but if you don't it will
        // be called automatically when the group is dropped.
        group.finish();
    }
}

fn bench_insert_best(c: &mut Criterion) {
    let starting_size = 1_000;
    let vec_sizes = [1_000, 10_000, 100_000];

    for vec_size in vec_sizes {
        let mut group = c.benchmark_group(format!("Insertion Best Case {}", vec_size));

        let mut tv = LinkedTieredVec::with_capacity(vec_size);
        let mut ftv: FlatTieredVec<usize> = FlatTieredVec::new(tv.tier_capacity());
        let mut v: Vec<usize> = Vec::with_capacity(tv.capacity());

        for i in 0..starting_size {
            tv.insert(i, i);
            ftv.insert(i, i);
            v.insert(i, i);
        }

        group.bench_function("Vec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                vec::insert_at(v.len(), black_box(&mut v), vec_size);
            })
        });

        group.bench_function("LinkedTieredVec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                linked_tiered_vec::insert_at(tv.len(), black_box(&mut tv), vec_size);
            })
        });

        group.bench_function("FlatTieredVec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                flat_tiered_vec::insert_at(ftv.len(), black_box(&mut ftv), vec_size);
            })
        });

        // It's recommended to call group.finish() explicitly at the end, but if you don't it will
        // be called automatically when the group is dropped.
        group.finish();
    }
}

fn bench_insert_random(c: &mut Criterion) {
    let starting_size = 1_000;
    let vec_sizes = [1_000, 10_000, 100_000];

    for vec_size in vec_sizes {
        let mut group = c.benchmark_group(format!("Insertion Random {}", vec_size));

        let mut tv = LinkedTieredVec::with_capacity(vec_size);
        let mut ftv: FlatTieredVec<usize> = FlatTieredVec::new(tv.tier_capacity());
        let mut v: Vec<usize> = Vec::with_capacity(tv.capacity());

        for i in 0..starting_size {
            tv.insert(i, i);
            ftv.insert(i, i);
            v.insert(i, i);
        }

        group.bench_function("Vec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                vec::insert(black_box(rng), black_box(&mut v), vec_size);
            })
        });

        group.bench_function("LinkedTieredVec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                linked_tiered_vec::insert(black_box(rng), black_box(&mut tv), vec_size);
            })
        });

        group.bench_function("FlatTieredVec", |b| {
            b.iter(|| {
                let rng = SmallRng::seed_from_u64(256);
                flat_tiered_vec::insert(black_box(rng), black_box(&mut ftv), vec_size);
            })
        });

        // It's recommended to call group.finish() explicitly at the end, but if you don't it will
        // be called automatically when the group is dropped.
        group.finish();
    }
}

fn bench_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("Update");

    let vec_size: usize = 1_000;
    let mut tv = LinkedTieredVec::with_capacity(vec_size);
    let mut ftv = FlatTieredVec::new(tv.tier_capacity());
    let mut v: Vec<_> = Vec::with_capacity(tv.capacity());

    for i in 0..vec_size {
        v.insert(i, i);
        tv.insert(i, i);
        ftv.insert(i, i);
    }

    group.bench_function("Vec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::update(black_box(rng), black_box(&mut v), vec_size / 2);
        })
    });

    group.bench_function("Vec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::update(black_box(rng), black_box(&mut v), vec_size * 3 / 4);
        })
    });

    group.bench_function("LinkedTieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            linked_tiered_vec::update(black_box(rng), black_box(&mut tv), vec_size / 2);
        })
    });

    group.bench_function("LinkedTieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            linked_tiered_vec::update(black_box(rng), black_box(&mut tv), vec_size * 3 / 4);
        })
    });

    group.bench_function("FlatTieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            flat_tiered_vec::update(black_box(rng), black_box(&mut ftv), vec_size / 2);
        })
    });

    group.bench_function("FlatTieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            flat_tiered_vec::update(black_box(rng), black_box(&mut ftv), vec_size * 3 / 4);
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("Delete");

    let vec_size: usize = 1_000;
    let mut tv = LinkedTieredVec::with_capacity(vec_size);
    let mut ftv = FlatTieredVec::new(tv.tier_capacity());
    let mut v: Vec<_> = Vec::with_capacity(tv.capacity());

    for i in 0..vec_size {
        v.insert(i, i);
        tv.insert(i, i);
        ftv.insert(i, i);
    }

    group.bench_function("Vec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::delete(black_box(rng), black_box(&mut v.clone()), vec_size / 2);
        })
    });

    group.bench_function("Vec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::delete(black_box(rng), black_box(&mut v.clone()), vec_size * 3 / 4);
        })
    });

    group.bench_function("LinkedTieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            linked_tiered_vec::delete(black_box(rng), black_box(&mut tv.clone()), vec_size / 2);
        })
    });

    group.bench_function("LinkedTieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            linked_tiered_vec::delete(black_box(rng), black_box(&mut tv.clone()), vec_size * 3 / 4);
        })
    });

    group.bench_function("FlatTieredVec 50%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            flat_tiered_vec::delete(black_box(rng), black_box(&mut ftv.clone()), vec_size / 2);
        })
    });

    group.bench_function("FlatTieredVec 75%", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            flat_tiered_vec::delete(
                black_box(rng),
                black_box(&mut ftv.clone()),
                vec_size * 3 / 4,
            );
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

fn bench_random_mix(c: &mut Criterion) {
    let mut group = c.benchmark_group("Random Mix");

    let vec_size: usize = 1_000;
    let tv: LinkedTieredVec<usize> = LinkedTieredVec::with_capacity(vec_size);
    let ftv: FlatTieredVec<usize> = FlatTieredVec::new(tv.tier_capacity());
    let mut v: Vec<usize> = Vec::with_capacity(tv.capacity());

    group.bench_function("Vec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            vec::random_mix(black_box(rng), black_box(&mut v), 1_000_000);
        })
    });

    group.bench_function("LinkedTieredVec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            linked_tiered_vec::random_mix(black_box(rng), black_box(&mut tv.clone()), 1_000_000);
        })
    });

    group.bench_function("FlatTieredVec", |b| {
        b.iter(|| {
            let rng = SmallRng::seed_from_u64(256);
            flat_tiered_vec::random_mix(black_box(rng), black_box(&mut ftv.clone()), 1_000_000);
        })
    });

    // It's recommended to call group.finish() explicitly at the end, but if you don't it will
    // be called automatically when the group is dropped.
    group.finish();
}

criterion_group!(
    benches,
    bench_insert_worst,
    bench_insert_best,
    bench_insert_random,
    // bench_delete,
    // bench_update,
    // bench_random_mix_half_update,
    // bench_random_mix
);
criterion_main!(benches);
