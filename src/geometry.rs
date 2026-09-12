use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn distance(self, other: Self) -> f32 {
        (self - other).length_squared().sqrt()
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Point {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

pub fn regular_polygon(sides: usize, radius: f32) -> Vec<Point> {
    (0..sides)
        .map(|i| {
            let angle =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * i as f32 / sides as f32;
            Point::new(radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

pub fn point_in_polygon_with_clearance(point: Point, polygon: &[Point], clearance: f32) -> bool {
    // The regular polygon is counter-clockwise in mathematical coordinates.
    polygon.iter().enumerate().all(|(i, &a)| {
        let b = polygon[(i + 1) % polygon.len()];
        let edge = b - a;
        let inward_cross = edge.x * (point.y - a.y) - edge.y * (point.x - a.x);
        inward_cross / edge.length_squared().sqrt() >= clearance
    })
}

pub fn segment_distance(a: Point, b: Point, c: Point, d: Point) -> f32 {
    if segments_intersect(a, b, c, d) {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

pub fn closest_point_on_segment(p: Point, a: Point, b: Point) -> Point {
    let ab = b - a;
    if ab.length_squared() < 1e-12 {
        return a;
    }
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    a + ab * t
}

fn point_segment_distance(p: Point, a: Point, b: Point) -> f32 {
    p.distance(closest_point_on_segment(p, a, b))
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    fn cross(a: Point, b: Point) -> f32 {
        a.x * b.y - a.y * b.x
    }
    let r = b - a;
    let s = d - c;
    let denominator = cross(r, s);
    if denominator.abs() < 1e-6 {
        return false;
    }
    let t = cross(c - a, s) / denominator;
    let u = cross(c - a, r) / denominator;
    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_clearance_works() {
        let polygon = regular_polygon(4, 100.0);
        assert!(point_in_polygon_with_clearance(Point::ZERO, &polygon, 10.0));
        assert!(!point_in_polygon_with_clearance(
            Point::new(0.0, -99.0),
            &polygon,
            10.0
        ));
    }

    #[test]
    fn crossing_segments_have_zero_distance() {
        let a = Point::new(-1.0, 0.0);
        let b = Point::new(1.0, 0.0);
        let c = Point::new(0.0, -1.0);
        let d = Point::new(0.0, 1.0);
        assert_eq!(segment_distance(a, b, c, d), 0.0);
    }

    #[test]
    fn zero_length_segment_distance_is_finite() {
        let point = Point::new(3.0, 4.0);
        assert_eq!(
            segment_distance(Point::ZERO, Point::ZERO, point, point),
            5.0
        );
    }
}
