use std::collections::VecDeque;

use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::geometry::{Point, point_in_polygon_with_clearance, regular_polygon, segment_distance};

#[derive(Clone, Debug)]
pub struct MazeConfig {
    pub seed: u64,
    pub polygon_sides: usize,
    pub radius: f32,
    pub unit: f32,
    pub max_length_units: usize,
    pub failure_limit: usize,
}

impl Default for MazeConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            polygon_sides: 6,
            radius: 360.0,
            unit: 14.0,
            max_length_units: 10,
            failure_limit: 1_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Maze {
    pub boundary: Vec<Point>,
    pub nodes: Vec<Point>,
    pub edges: Vec<(usize, usize)>,
    pub unit: f32,
    pub start: usize,
    pub end: usize,
    pub solution_units: usize,
    pub attempts: usize,
}

impl Maze {
    pub fn segments(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.edges
            .iter()
            .map(|&(a, b)| (self.nodes[a], self.nodes[b]))
    }
}

pub fn generate(config: &MazeConfig) -> Maze {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let boundary = regular_polygon(config.polygon_sides, config.radius);
    let origin = random_interior_point(&mut rng, &boundary, config.radius, config.unit);
    let mut nodes = vec![origin];
    let mut edges = Vec::new();
    let mut accepted_segments: Vec<(Point, Point)> = Vec::new();
    let mut failures = 0;
    let mut attempts = 0;

    while failures < config.failure_limit {
        attempts += 1;
        let from_index = rng.random_range(0..nodes.len());
        let from = nodes[from_index];
        let angle_step = rng.random_range(0..24);
        let angle = (angle_step as f32 * 15.0).to_radians();
        let direction = Point::new(angle.cos(), angle.sin());
        let length_units = rng.random_range(1..=config.max_length_units);
        let to = from + direction * (config.unit * length_units as f32);

        if !line_is_clear(
            from,
            to,
            direction,
            &boundary,
            &accepted_segments,
            config.unit,
        ) {
            failures += 1;
            continue;
        }

        let mut previous = from_index;
        for step in 1..=length_units {
            let point = from + direction * (config.unit * step as f32);
            let next = nodes.len();
            nodes.push(point);
            edges.push((previous, next));
            previous = next;
        }
        accepted_segments.push((from, to));
        failures = 0;
    }

    let (start, end, solution_units) = graph_diameter(nodes.len(), &edges);

    Maze {
        boundary,
        nodes,
        edges,
        unit: config.unit,
        start,
        end,
        solution_units,
        attempts,
    }
}

fn random_interior_point(rng: &mut StdRng, boundary: &[Point], radius: f32, unit: f32) -> Point {
    // Rejection sampling in the polygon's bounding square is uniform over the
    // usable polygon interior. Keep a small margin so the first branch has room.
    for _ in 0..10_000 {
        let point = Point::new(
            rng.random_range(-radius..=radius),
            rng.random_range(-radius..=radius),
        );
        if point_in_polygon_with_clearance(point, boundary, unit) {
            return point;
        }
    }
    Point::ZERO
}

fn line_is_clear(
    from: Point,
    to: Point,
    direction: Point,
    boundary: &[Point],
    segments: &[(Point, Point)],
    unit: f32,
) -> bool {
    if !point_in_polygon_with_clearance(from, boundary, unit * 0.5)
        || !point_in_polygon_with_clearance(to, boundary, unit * 0.5)
    {
        return false;
    }

    // A branch necessarily touches the path it grows from. Exempt its first unit
    // only when testing a segment that actually contains `from`; unrelated
    // segments must clear the entire candidate line.
    let trimmed_from = from + direction * unit;
    segments.iter().all(|&(a, b)| {
        let starts_on_segment = segment_distance(from, from, a, b) < 1e-3;
        let clearance_start = if starts_on_segment {
            trimmed_from
        } else {
            from
        };
        segment_distance(clearance_start, to, a, b) + 1e-3 >= unit
    })
}

fn graph_diameter(node_count: usize, edges: &[(usize, usize)]) -> (usize, usize, usize) {
    let mut adjacency = vec![Vec::new(); node_count];
    for &(a, b) in edges {
        adjacency[a].push(b);
        adjacency[b].push(a);
    }

    // Every generated line adds fresh nodes and attaches them at exactly one
    // existing node, so the graph is a tree. Two BFS traversals therefore find
    // its exact diameter in O(V + E): any node -> farthest A -> farthest B.
    let (start, _) = farthest_node(0, &adjacency);
    let (end, distance) = farthest_node(start, &adjacency);
    (start, end, distance)
}

fn farthest_node(origin: usize, adjacency: &[Vec<usize>]) -> (usize, usize) {
    let node_count = adjacency.len();
    let mut distance = vec![usize::MAX; node_count];
    let mut queue = VecDeque::from([origin]);
    distance[origin] = 0;
    while let Some(node) = queue.pop_front() {
        for &next in &adjacency[node] {
            if distance[next] == usize::MAX {
                distance[next] = distance[node] + 1;
                queue.push_back(next);
            }
        }
    }
    distance
        .into_iter()
        .enumerate()
        .filter(|(_, d)| *d != usize::MAX)
        .max_by_key(|&(_, d)| d)
        .unwrap_or((origin, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_is_deterministic() {
        let config = MazeConfig {
            failure_limit: 100,
            ..Default::default()
        };
        let first = generate(&config);
        let second = generate(&config);
        assert_eq!(first.nodes, second.nodes);
        assert_eq!(first.edges, second.edges);
        assert_eq!((first.start, first.end), (second.start, second.end));
    }

    #[test]
    fn endpoints_are_valid() {
        let maze = generate(&MazeConfig {
            failure_limit: 50,
            ..Default::default()
        });
        assert!(maze.start < maze.nodes.len());
        assert!(maze.end < maze.nodes.len());
        assert_ne!(maze.start, maze.end);
        assert!(maze.solution_units > 0);
    }

    #[test]
    fn origin_is_seeded_random_interior_point() {
        let config = MazeConfig {
            failure_limit: 10,
            ..Default::default()
        };
        let maze = generate(&config);
        assert_ne!(maze.nodes[0], Point::ZERO);
        assert!(point_in_polygon_with_clearance(
            maze.nodes[0],
            &maze.boundary,
            config.unit
        ));
    }

    #[test]
    fn tree_diameter_is_exact() {
        // 0-1-2-3 with a short branch 1-4: diameter is 3 edges (0 to 3 or 4 to 3).
        let edges = [(0, 1), (1, 2), (2, 3), (1, 4)];
        let (start, end, distance) = graph_diameter(5, &edges);
        assert_eq!(distance, 3);
        assert!((start == 3 && matches!(end, 0 | 4)) || (end == 3 && matches!(start, 0 | 4)));
    }

    #[test]
    fn unrelated_segments_do_not_receive_branch_clearance_exemption() {
        let boundary = regular_polygon(4, 100.0);
        let unrelated = [(Point::new(2.0, -20.0), Point::new(2.0, 20.0))];
        assert!(!line_is_clear(
            Point::ZERO,
            Point::new(20.0, 0.0),
            Point::new(1.0, 0.0),
            &boundary,
            &unrelated,
            10.0,
        ));
    }
}
