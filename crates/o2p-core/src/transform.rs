use crate::Point;

/// Affine transform in top-left, x-right, y-down point coordinates.
/// `x' = a*x + c*y + e`, `y' = b*x + d*y + f`.
/// No format or backend units appear in this representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translation(offset: Point) -> Self {
        Self {
            e: offset.x,
            f: offset.y,
            ..Self::IDENTITY
        }
    }

    /// Clockwise degrees in the y-down coordinate system. Reflections occur
    /// about the local axes BEFORE rotation, keeping their axes element-local.
    pub fn rotation_and_flips(degrees: f32, horizontal: bool, vertical: bool) -> Self {
        let angle = f64::from(degrees).rem_euclid(360.0);
        // Exact quarter turns avoid spurious offsets on degenerate lines.
        let (s, c) = match angle {
            0.0 => (0.0, 1.0),
            90.0 => (1.0, 0.0),
            180.0 => (0.0, -1.0),
            270.0 => (-1.0, 0.0),
            _ => angle.to_radians().sin_cos(),
        };
        let sx = if horizontal { -1.0 } else { 1.0 };
        let sy = if vertical { -1.0 } else { 1.0 };
        Self {
            a: (c * sx) as f32,
            b: (s * sx) as f32,
            c: (-s * sy) as f32,
            d: (c * sy) as f32,
            ..Self::IDENTITY
        }
    }

    /// Conjugate a linear transform by translation to an element's center:
    /// `T(center) * self * T(-center)`. Already-positioned drawing coordinates
    /// retain their original bounds; the center is fixed by the transform.
    pub fn around(self, center: Point) -> Self {
        let x = f64::from(center.x);
        let y = f64::from(center.y);
        Self {
            e: (x - f64::from(self.a) * x - f64::from(self.c) * y + f64::from(self.e)) as f32,
            f: (y - f64::from(self.b) * x - f64::from(self.d) * y + f64::from(self.f)) as f32,
            ..self
        }
    }

    pub fn apply(self, point: Point) -> Point {
        let x = f64::from(point.x);
        let y = f64::from(point.y);
        Point {
            x: (f64::from(self.a) * x + f64::from(self.c) * y + f64::from(self.e)) as f32,
            y: (f64::from(self.b) * x + f64::from(self.d) * y + f64::from(self.f)) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_and_translation_use_top_left_coordinates() {
        let center = Point { x: 30.0, y: 50.0 };
        let t = Transform::rotation_and_flips(90.0, false, false).around(center);
        assert_eq!(t.apply(center), center);
        assert_eq!(
            t.apply(Point { x: 10.0, y: 40.0 }),
            Point { x: 40.0, y: 30.0 }
        );
        assert_eq!(
            Transform::translation(Point { x: 3.0, y: -4.0 }).apply(center),
            Point { x: 33.0, y: 46.0 }
        );
    }

    #[test]
    fn flips_precede_rotation_and_keep_element_center_fixed() {
        let center = Point { x: 30.0, y: 50.0 };
        let p = Point { x: 10.0, y: 40.0 };
        for (h, v, degrees, expected) in [
            (true, false, 0.0, Point { x: 50.0, y: 40.0 }),
            (false, true, 0.0, Point { x: 10.0, y: 60.0 }),
            (true, true, 0.0, Point { x: 50.0, y: 60.0 }),
            (true, false, 90.0, Point { x: 40.0, y: 70.0 }),
            (false, true, 90.0, Point { x: 20.0, y: 30.0 }),
            (true, true, 90.0, Point { x: 20.0, y: 70.0 }),
        ] {
            let t = Transform::rotation_and_flips(degrees, h, v).around(center);
            assert_eq!(t.apply(center), center);
            assert_eq!(t.apply(p), expected);
        }
    }

    #[test]
    fn diagonal_and_negative_rotation_are_clockwise_and_normalized() {
        let p = Point { x: 2.0, y: 0.0 };
        let p45 = Transform::rotation_and_flips(45.0, false, false).apply(p);
        assert!((p45.x - 2.0_f32.sqrt()).abs() < 1e-6);
        assert!((p45.y - 2.0_f32.sqrt()).abs() < 1e-6);
        assert_eq!(
            Transform::rotation_and_flips(-90.0, false, false).apply(p),
            Point { x: 0.0, y: -2.0 }
        );
    }
}
