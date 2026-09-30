//! Benchmark suite for BVH raycasting performance.
//!
//! Performance target: <0.4ms for 100k triangle scenes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use glam::Vec3;
use omsi_geometry::bvh::{Bvh, BvhPrimitive, Ray};

fn generate_random_triangles(count: usize, seed: u64) -> Vec<BvhPrimitive> {
    let mut prims = Vec::with_capacity(count);
    
    // Simple LCG for deterministic generation
    let mut rng = seed;
    let next_random = |r: &mut u64| -> f32 {
        *r = r.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*r >> 32) as f32) / (u32::MAX as f32)
    };

    for i in 0..count {
        let x = next_random(&mut rng) * 1000.0 - 500.0;
        let y = next_random(&mut rng) * 1000.0 - 500.0;
        let z = next_random(&mut rng) * 100.0;
        
        let size = next_random(&mut rng) * 2.0 + 0.5;
        
        let v0 = Vec3::new(x, y, z);
        let v1 = Vec3::new(x + size, y, z);
        let v2 = Vec3::new(x + size * 0.5, y + size, z + size * 0.5);
        
        prims.push(BvhPrimitive::from_triangle(v0, v1, v2, i as u64));
    }
    
    prims
}

fn generate_grid_triangles(count: usize) -> Vec<BvhPrimitive> {
    let mut prims = Vec::with_capacity(count);
    let grid_size = (count as f32).sqrt().ceil() as usize;
    
    for i in 0..count {
        let x = (i % grid_size) as f32 * 2.0;
        let y = (i / grid_size) as f32 * 2.0;
        let z = 0.0;
        
        let v0 = Vec3::new(x, y, z);
        let v1 = Vec3::new(x + 1.0, y, z);
        let v2 = Vec3::new(x + 0.5, y + 1.0, z + 0.5);
        
        prims.push(BvhPrimitive::from_triangle(v0, v1, v2, i as u64));
    }
    
    prims
}

fn bench_bvh_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_build");
    
    for size in [100, 1_000, 10_000, 100_000].iter() {
        group.bench_with_input(BenchmarkId::new("random", size), size, |b, &size| {
            let prims = generate_random_triangles(size, 12345);
            b.iter(|| {
                let bvh = Bvh::new(black_box(prims.clone()));
                black_box(bvh);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("grid", size), size, |b, &size| {
            let prims = generate_grid_triangles(size);
            b.iter(|| {
                let bvh = Bvh::new(black_box(prims.clone()));
                black_box(bvh);
            });
        });
    }
    
    group.finish();
}

fn bench_bvh_raycast(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_raycast");
    
    for size in [100, 1_000, 10_000, 100_000].iter() {
        let prims = generate_random_triangles(*size, 12345);
        let bvh = Bvh::new(prims);
        
        group.bench_with_input(BenchmarkId::new("hit_center", size), size, |b, _| {
            let ray = Ray::new(Vec3::new(-100.0, 0.0, 50.0), Vec3::new(1.0, 0.0, 0.0));
            b.iter(|| {
                let hits = bvh.intersect_ray(black_box(&ray), black_box(256));
                black_box(hits);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("hit_sparse", size), size, |b, _| {
            let ray = Ray::new(Vec3::new(-600.0, -600.0, 150.0), Vec3::new(1.0, 1.0, -0.5).normalize());
            b.iter(|| {
                let hits = bvh.intersect_ray(black_box(&ray), black_box(256));
                black_box(hits);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("miss", size), size, |b, _| {
            let ray = Ray::new(Vec3::new(-1000.0, -1000.0, 500.0), Vec3::new(0.0, 0.0, 1.0));
            b.iter(|| {
                let hits = bvh.intersect_ray(black_box(&ray), black_box(256));
                black_box(hits);
            });
        });
    }
    
    group.finish();
}

fn bench_bvh_raycast_max_candidates(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_raycast_candidates");
    
    let prims = generate_random_triangles(100_000, 12345);
    let bvh = Bvh::new(prims);
    
    for max_hits in [1, 10, 32, 64, 128, 256].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(max_hits), max_hits, |b, &max_hits| {
            let ray = Ray::new(Vec3::new(-100.0, 0.0, 50.0), Vec3::new(1.0, 0.0, 0.0));
            b.iter(|| {
                let hits = bvh.intersect_ray(black_box(&ray), black_box(max_hits));
                black_box(hits);
            });
        });
    }
    
    group.finish();
}

fn bench_bvh_frustum_cull(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_frustum_cull");
    
    for size in [1_000, 10_000, 100_000].iter() {
        let prims = generate_random_triangles(*size, 12345);
        let bvh = Bvh::new(prims);
        
        group.bench_with_input(BenchmarkId::new("sphere", size), size, |b, _| {
            let center = Vec3::new(0.0, 0.0, 50.0);
            let radius = 100.0;
            b.iter(|| {
                let candidates = bvh.frustum_cull(
                    |aabb| {
                        let closest = aabb.center();
                        (closest - glam::Vec3A::from(center)).length() < radius
                    },
                    black_box(256),
                );
                black_box(candidates);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("box", size), size, |b, _| {
            let min = Vec3::new(-50.0, -50.0, 0.0);
            let max = Vec3::new(50.0, 50.0, 100.0);
            b.iter(|| {
                let candidates = bvh.frustum_cull(
                    |aabb| {
                        let center = aabb.center();
                        center.x >= min.x && center.x <= max.x
                            && center.y >= min.y && center.y <= max.y
                            && center.z >= min.z && center.z <= max.z
                    },
                    black_box(256),
                );
                black_box(candidates);
            });
        });
    }
    
    group.finish();
}

fn bench_bvh_quality_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("bvh_quality");
    
    for size in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(BenchmarkId::new("random", size), size, |b, &size| {
            let prims = generate_random_triangles(size, 12345);
            let mut bvh = Bvh::new(prims);
            b.iter(|| {
                black_box(bvh.check_quality());
            });
        });
        
        group.bench_with_input(BenchmarkId::new("grid", size), size, |b, &size| {
            let prims = generate_grid_triangles(size);
            let mut bvh = Bvh::new(prims);
            b.iter(|| {
                black_box(bvh.check_quality());
            });
        });
    }
    
    group.finish();
}

fn bench_worst_case_100k(c: &mut Criterion) {
    let mut group = c.benchmark_group("worst_case_100k_triangles");
    group.sample_size(100);
    
    let prims = generate_random_triangles(100_000, 12345);
    let bvh = Bvh::new(prims);
    
    group.bench_function("broadphase_256_candidates", |b| {
        let ray = Ray::new(Vec3::new(-100.0, 0.0, 50.0), Vec3::new(1.0, 0.0, 0.0));
        b.iter(|| {
            let hits = bvh.intersect_ray(black_box(&ray), black_box(256));
            black_box(hits);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_bvh_build,
    bench_bvh_raycast,
    bench_bvh_raycast_max_candidates,
    bench_bvh_frustum_cull,
    bench_bvh_quality_check,
    bench_worst_case_100k
);
criterion_main!(benches);
