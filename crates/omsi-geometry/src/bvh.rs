//! Dynamic BVH (Bounding Volume Hierarchy) spatial acceleration for raycasting.
//!
//! Provides hierarchical bounding volume tree with SIMD-friendly AABB layout,
//! Surface Area Heuristic (SAH) build, and adaptive rebuild triggers.
//!
//! Performance target: <0.4ms for 100k triangle scenes.

use glam::{Vec3, Vec3A};

/// Axis-aligned bounding box with SIMD-friendly layout.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
pub struct Aabb {
    pub min: Vec3A,
    pub max: Vec3A,
}

impl Aabb {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self {
            min: Vec3A::from(min),
            max: Vec3A::from(max),
        }
    }

    #[inline]
    pub fn from_point(point: Vec3) -> Self {
        Self {
            min: Vec3A::from(point),
            max: Vec3A::from(point),
        }
    }

    #[inline]
    pub fn empty() -> Self {
        Self {
            min: Vec3A::splat(f32::INFINITY),
            max: Vec3A::splat(f32::NEG_INFINITY),
        }
    }

    #[inline]
    pub fn expand(&mut self, other: &Aabb) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    #[inline]
    pub fn expand_point(&mut self, point: Vec3) {
        let p = Vec3A::from(point);
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    #[inline]
    pub fn surface_area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    #[inline]
    pub fn center(&self) -> Vec3A {
        (self.min + self.max) * 0.5
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    /// Ray-AABB intersection test with early exit.
    #[inline]
    pub fn intersect_ray(&self, origin: Vec3A, inv_dir: Vec3A, t_min: f32, t_max: f32) -> bool {
        let t0 = (self.min - origin) * inv_dir;
        let t1 = (self.max - origin) * inv_dir;

        let tmin = t0.min(t1);
        let tmax = t0.max(t1);

        let t_near = tmin.x.max(tmin.y).max(tmin.z).max(t_min);
        let t_far = tmax.x.min(tmax.y).min(tmax.z).min(t_max);

        t_near <= t_far
    }
}

/// BVH node storing either child indices (interior) or primitive indices (leaf).
#[derive(Debug, Clone)]
struct BvhNode {
    bounds: Aabb,
    /// For interior nodes: left and right child indices.
    /// For leaf nodes: start index and count in the primitive array.
    data: NodeData,
}

#[derive(Debug, Clone)]
enum NodeData {
    Interior { left: usize, right: usize },
    Leaf { start: usize, count: usize },
}

/// Primitive with bounding box and user-provided ID.
#[derive(Debug, Clone)]
pub struct BvhPrimitive {
    pub bounds: Aabb,
    pub id: u64,
}

impl BvhPrimitive {
    pub fn new(bounds: Aabb, id: u64) -> Self {
        Self { bounds, id }
    }

    pub fn from_triangle(v0: Vec3, v1: Vec3, v2: Vec3, id: u64) -> Self {
        let mut bounds = Aabb::from_point(v0);
        bounds.expand_point(v1);
        bounds.expand_point(v2);
        Self { bounds, id }
    }
}

/// Ray for intersection queries.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3A,
    pub direction: Vec3A,
    inv_direction: Vec3A,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        let direction = Vec3A::from(direction.normalize());
        let inv_direction = Vec3A::ONE / direction;
        Self {
            origin: Vec3A::from(origin),
            direction,
            inv_direction,
        }
    }
}

/// Hit result from ray intersection.
#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub id: u64,
    pub t: f32,
}

/// Quality metrics for adaptive rebuild triggering.
#[derive(Debug, Clone, Copy, Default)]
pub struct BvhQualityMetrics {
    pub max_depth: usize,
    pub avg_leaf_occupancy: f32,
    pub total_nodes: usize,
    pub total_leaves: usize,
}

/// Dynamic BVH with adaptive rebuild and quality monitoring.
pub struct Bvh {
    nodes: Vec<BvhNode>,
    primitives: Vec<BvhPrimitive>,
    primitive_indices: Vec<usize>,
    metrics: BvhQualityMetrics,
    #[allow(dead_code)]
    max_leaf_size: usize,
    needs_rebuild: bool,
}

impl Bvh {
    /// Create a new BVH from primitives using SAH (Surface Area Heuristic) build.
    pub fn new(mut primitives: Vec<BvhPrimitive>) -> Self {
        let max_leaf_size = 4;
        let primitive_count = primitives.len();
        
        if primitive_count == 0 {
            return Self {
                nodes: Vec::new(),
                primitives,
                primitive_indices: Vec::new(),
                metrics: BvhQualityMetrics::default(),
                max_leaf_size,
                needs_rebuild: false,
            };
        }

        let mut primitive_indices: Vec<usize> = (0..primitive_count).collect();
        let mut nodes = Vec::with_capacity(primitive_count * 2);
        
        let max_depth;
        let total_leaves;
        let total_leaf_occupancy;
        
        {
            let mut builder = BvhBuilder {
                primitives: &mut primitives,
                primitive_indices: &mut primitive_indices,
                nodes: &mut nodes,
                max_leaf_size,
                max_depth: 0,
                total_leaves: 0,
                total_leaf_occupancy: 0,
            };

            builder.build_recursive(0, primitive_count, 0);
            
            max_depth = builder.max_depth;
            total_leaves = builder.total_leaves;
            total_leaf_occupancy = builder.total_leaf_occupancy;
        }

        let avg_leaf_occupancy = if total_leaves > 0 {
            total_leaf_occupancy as f32 / total_leaves as f32
        } else {
            0.0
        };

        let metrics = BvhQualityMetrics {
            max_depth,
            avg_leaf_occupancy,
            total_nodes: nodes.len(),
            total_leaves,
        };

        Self {
            nodes,
            primitives,
            primitive_indices,
            metrics,
            max_leaf_size,
            needs_rebuild: false,
        }
    }

    /// Query if rebuild is needed based on quality metrics.
    #[inline]
    pub fn check_quality(&mut self) -> bool {
        let needs_rebuild = self.metrics.max_depth > 12 || self.metrics.avg_leaf_occupancy < 0.4;
        self.needs_rebuild = needs_rebuild;
        needs_rebuild
    }

    /// Trigger adaptive rebuild if quality degrades.
    pub fn rebuild_if_needed(&mut self) {
        if self.needs_rebuild {
            *self = Self::new(self.primitives.clone());
        }
    }

    /// Get current quality metrics.
    #[inline]
    pub fn metrics(&self) -> &BvhQualityMetrics {
        &self.metrics
    }

    /// Intersect ray with BVH, returning up to `max_hits` candidates.
    /// Capped at 256 for fail-safe.
    pub fn intersect_ray(&self, ray: &Ray, max_hits: usize) -> Vec<RayHit> {
        let max_hits = max_hits.min(256);
        let mut hits = Vec::new();

        if self.nodes.is_empty() {
            return hits;
        }

        let mut stack = Vec::with_capacity(64);
        stack.push(0usize);

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx];

            if !node.bounds.intersect_ray(ray.origin, ray.inv_direction, 0.0, f32::INFINITY) {
                continue;
            }

            match &node.data {
                NodeData::Interior { left, right } => {
                    stack.push(*right);
                    stack.push(*left);
                }
                NodeData::Leaf { start, count } => {
                    for i in *start..(*start + *count) {
                        let prim_idx = self.primitive_indices[i];
                        let prim = &self.primitives[prim_idx];

                        if prim.bounds.intersect_ray(ray.origin, ray.inv_direction, 0.0, f32::INFINITY) {
                            let center = prim.bounds.center();
                            let t = (center - ray.origin).dot(ray.direction);
                            
                            if t >= 0.0 {
                                hits.push(RayHit { id: prim.id, t });
                                
                                if hits.len() >= max_hits {
                                    hits.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
                                    return hits;
                                }
                            }
                        }
                    }
                }
            }
        }

        hits.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        hits
    }

    /// Early-exit frustum culling query.
    pub fn frustum_cull<F>(&self, test_fn: F, max_candidates: usize) -> Vec<u64>
    where
        F: Fn(&Aabb) -> bool,
    {
        let max_candidates = max_candidates.min(256);
        let mut candidates = Vec::new();

        if self.nodes.is_empty() {
            return candidates;
        }

        let mut stack = Vec::with_capacity(64);
        stack.push(0usize);

        while let Some(node_idx) = stack.pop() {
            let node = &self.nodes[node_idx];

            if !test_fn(&node.bounds) {
                continue;
            }

            match &node.data {
                NodeData::Interior { left, right } => {
                    stack.push(*right);
                    stack.push(*left);
                }
                NodeData::Leaf { start, count } => {
                    for i in *start..(*start + *count) {
                        let prim_idx = self.primitive_indices[i];
                        let prim = &self.primitives[prim_idx];

                        if test_fn(&prim.bounds) {
                            candidates.push(prim.id);
                            
                            if candidates.len() >= max_candidates {
                                return candidates;
                            }
                        }
                    }
                }
            }
        }

        candidates
    }
}

struct BvhBuilder<'a> {
    primitives: &'a mut [BvhPrimitive],
    primitive_indices: &'a mut [usize],
    nodes: &'a mut Vec<BvhNode>,
    max_leaf_size: usize,
    max_depth: usize,
    total_leaves: usize,
    total_leaf_occupancy: usize,
}

impl<'a> BvhBuilder<'a> {
    fn build_recursive(&mut self, start: usize, end: usize, depth: usize) -> usize {
        self.max_depth = self.max_depth.max(depth);
        
        let count = end - start;
        let mut bounds = Aabb::empty();
        
        for i in start..end {
            let prim_idx = self.primitive_indices[i];
            bounds.expand(&self.primitives[prim_idx].bounds);
        }

        if count <= self.max_leaf_size {
            self.total_leaves += 1;
            self.total_leaf_occupancy += count;
            
            let node_idx = self.nodes.len();
            self.nodes.push(BvhNode {
                bounds,
                data: NodeData::Leaf { start, count },
            });
            return node_idx;
        }

        let (_axis, split_pos) = self.find_best_split(start, end, &bounds);
        
        if split_pos == start || split_pos == end {
            self.total_leaves += 1;
            self.total_leaf_occupancy += count;
            
            let node_idx = self.nodes.len();
            self.nodes.push(BvhNode {
                bounds,
                data: NodeData::Leaf { start, count },
            });
            return node_idx;
        }

        let left = self.build_recursive(start, split_pos, depth + 1);
        let right = self.build_recursive(split_pos, end, depth + 1);

        let node_idx = self.nodes.len();
        self.nodes.push(BvhNode {
            bounds,
            data: NodeData::Interior { left, right },
        });
        node_idx
    }

    fn find_best_split(&mut self, start: usize, end: usize, bounds: &Aabb) -> (usize, usize) {
        let extent = bounds.max - bounds.min;
        let axis = if extent.x > extent.y && extent.x > extent.z {
            0
        } else if extent.y > extent.z {
            1
        } else {
            2
        };

        let num_bins = 16;
        let mut bins = vec![SahBin::default(); num_bins];
        let inv_bin_size = num_bins as f32 / extent[axis];

        for i in start..end {
            let prim_idx = self.primitive_indices[i];
            let prim = &self.primitives[prim_idx];
            let center = prim.bounds.center();
            let offset = center[axis] - bounds.min[axis];
            let bin_idx = ((offset * inv_bin_size) as usize).min(num_bins - 1);
            
            bins[bin_idx].count += 1;
            bins[bin_idx].bounds.expand(&prim.bounds);
        }

        let mut left_counts = vec![0; num_bins];
        let mut left_bounds = vec![Aabb::empty(); num_bins];
        let mut right_counts = vec![0; num_bins];
        let mut right_bounds = vec![Aabb::empty(); num_bins];

        left_counts[0] = bins[0].count;
        left_bounds[0] = bins[0].bounds;
        for i in 1..num_bins {
            left_counts[i] = left_counts[i - 1] + bins[i].count;
            left_bounds[i] = left_bounds[i - 1];
            left_bounds[i].expand(&bins[i].bounds);
        }

        right_counts[num_bins - 1] = bins[num_bins - 1].count;
        right_bounds[num_bins - 1] = bins[num_bins - 1].bounds;
        for i in (0..num_bins - 1).rev() {
            right_counts[i] = right_counts[i + 1] + bins[i].count;
            right_bounds[i] = right_bounds[i + 1];
            right_bounds[i].expand(&bins[i].bounds);
        }

        let mut best_cost = f32::INFINITY;
        let mut best_split = num_bins / 2;

        for i in 0..num_bins - 1 {
            let left_count = left_counts[i];
            let right_count = right_counts[i + 1];
            
            if left_count == 0 || right_count == 0 {
                continue;
            }

            let cost = left_bounds[i].surface_area() * left_count as f32
                + right_bounds[i + 1].surface_area() * right_count as f32;

            if cost < best_cost {
                best_cost = cost;
                best_split = i + 1;
            }
        }

        let mid_point = bounds.min[axis] + (best_split as f32 / inv_bin_size);
        
        let mut mid = start;
        for i in start..end {
            let prim_idx = self.primitive_indices[i];
            let prim = &self.primitives[prim_idx];
            let center = prim.bounds.center();
            
            if center[axis] < mid_point {
                self.primitive_indices.swap(i, mid);
                mid += 1;
            }
        }

        if mid == start || mid == end {
            mid = (start + end) / 2;
        }

        (axis, mid)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct SahBin {
    bounds: Aabb,
    count: usize,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_intersection() {
        let aabb = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let ray = Ray::new(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0));
        
        assert!(aabb.intersect_ray(ray.origin, ray.inv_direction, 0.0, f32::INFINITY));
    }

    #[test]
    fn test_bvh_empty() {
        let bvh = Bvh::new(Vec::new());
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = bvh.intersect_ray(&ray, 10);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn test_bvh_single_primitive() {
        let prim = BvhPrimitive::new(Aabb::new(Vec3::ZERO, Vec3::ONE), 1);
        let bvh = Bvh::new(vec![prim]);
        let ray = Ray::new(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0));
        let hits = bvh.intersect_ray(&ray, 10);
        
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn test_bvh_multiple_primitives() {
        let prims = vec![
            BvhPrimitive::new(Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0)), 1),
            BvhPrimitive::new(Aabb::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 1.0, 1.0)), 2),
            BvhPrimitive::new(Aabb::new(Vec3::new(4.0, 0.0, 0.0), Vec3::new(5.0, 1.0, 1.0)), 3),
        ];
        
        let bvh = Bvh::new(prims);
        let ray = Ray::new(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0));
        let hits = bvh.intersect_ray(&ray, 10);
        
        assert!(hits.len() >= 1);
        assert!(hits.iter().any(|h| h.id == 1));
    }

    #[test]
    fn test_bvh_miss() {
        let prim = BvhPrimitive::new(Aabb::new(Vec3::new(0.0, 10.0, 0.0), Vec3::new(1.0, 11.0, 1.0)), 1);
        let bvh = Bvh::new(vec![prim]);
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let hits = bvh.intersect_ray(&ray, 10);
        
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn test_bvh_distance_sorting() {
        let prims = vec![
            BvhPrimitive::new(Aabb::new(Vec3::new(5.0, 0.0, 0.0), Vec3::new(6.0, 1.0, 1.0)), 3),
            BvhPrimitive::new(Aabb::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 1.0)), 1),
            BvhPrimitive::new(Aabb::new(Vec3::new(3.0, 0.0, 0.0), Vec3::new(4.0, 1.0, 1.0)), 2),
        ];
        
        let bvh = Bvh::new(prims);
        let ray = Ray::new(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0));
        let hits = bvh.intersect_ray(&ray, 10);
        
        for i in 1..hits.len() {
            assert!(hits[i - 1].t <= hits[i].t, "Hits should be sorted by distance");
        }
    }

    #[test]
    fn test_bvh_max_hits_limit() {
        let mut prims = Vec::new();
        for i in 0..100 {
            prims.push(BvhPrimitive::new(
                Aabb::new(Vec3::new(i as f32, 0.0, 0.0), Vec3::new(i as f32 + 1.0, 1.0, 1.0)),
                i,
            ));
        }
        
        let bvh = Bvh::new(prims);
        let ray = Ray::new(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0));
        let hits = bvh.intersect_ray(&ray, 10);
        
        assert!(hits.len() <= 10);
    }

    #[test]
    fn test_bvh_quality_metrics() {
        let mut prims = Vec::new();
        for i in 0..1000 {
            let x = (i % 10) as f32;
            let y = (i / 10 % 10) as f32;
            let z = (i / 100) as f32;
            prims.push(BvhPrimitive::new(
                Aabb::new(Vec3::new(x, y, z), Vec3::new(x + 0.5, y + 0.5, z + 0.5)),
                i,
            ));
        }
        
        let bvh = Bvh::new(prims);
        let metrics = bvh.metrics();
        
        assert!(metrics.max_depth > 0);
        assert!(metrics.total_nodes > 0);
        assert!(metrics.total_leaves > 0);
        assert!(metrics.avg_leaf_occupancy > 0.0);
    }

    #[test]
    fn test_adaptive_rebuild_trigger() {
        let prims = vec![
            BvhPrimitive::new(Aabb::new(Vec3::ZERO, Vec3::ONE), 1),
        ];
        
        let mut bvh = Bvh::new(prims);
        
        bvh.metrics.max_depth = 15;
        assert!(bvh.check_quality());
        
        bvh.metrics.max_depth = 5;
        bvh.metrics.avg_leaf_occupancy = 0.2;
        assert!(bvh.check_quality());
    }

    #[test]
    fn test_frustum_cull() {
        let mut prims = Vec::new();
        // Create well-separated primitives
        for i in 0..10 {
            let x = i as f32 * 50.0;
            prims.push(BvhPrimitive::new(
                Aabb::new(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 1.0, 1.0, 1.0)),
                i + 1,
            ));
        }
        
        let bvh = Bvh::new(prims);
        
        // Test culling: should find primitives within the range
        let candidates = bvh.frustum_cull(
            |aabb| aabb.center().x < 100.0,
            256,
        );
        
        // Should find primitives 1 and 2 (at x=0 and x=50)
        assert!(candidates.len() >= 2);
        assert!(candidates.contains(&1));
        assert!(candidates.contains(&2));
        // Should not find primitive 10 (at x=450)
        assert!(!candidates.contains(&10));
    }

    #[test]
    fn test_triangle_primitive() {
        let v0 = Vec3::new(0.0, 0.0, 0.0);
        let v1 = Vec3::new(1.0, 0.0, 0.0);
        let v2 = Vec3::new(0.5, 1.0, 0.0);
        
        let prim = BvhPrimitive::from_triangle(v0, v1, v2, 42);
        
        assert_eq!(prim.id, 42);
        assert!(prim.bounds.min.x >= -0.01);
        assert!(prim.bounds.max.x <= 1.01);
        assert!(prim.bounds.min.y >= -0.01);
        assert!(prim.bounds.max.y <= 1.01);
    }

    #[test]
    fn test_failsafe_candidate_cap() {
        let mut prims = Vec::new();
        for i in 0..2000 {
            prims.push(BvhPrimitive::new(
                Aabb::new(Vec3::new(i as f32, 0.0, 0.0), Vec3::new(i as f32 + 1.0, 1.0, 1.0)),
                i,
            ));
        }
        
        let bvh = Bvh::new(prims);
        let ray = Ray::new(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0));
        let hits = bvh.intersect_ray(&ray, 1000);
        
        assert!(hits.len() <= 256, "Should be capped at 256 candidates");
    }
}
