//! Triangulating a simple polygon given by its corner positions in 3-space: [`triangulate`].
//!
//! A navigation face is a polygon of up to a few dozen corners, not always convex (about 5 % of Sekiro's faces and 2 % of Elden
//! Ring's are not, seen from above) and not always planar (corners lie up to 1.4 m off the face's mean plane), so a fan
//! from one corner is wrong for some of them. The polygon is projected along its Newell normal's largest axis and cut
//! into triangles by ear clipping in that plane.
//!
//! This file uses only the standard library, so it can be read and run on its own.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

/// The corner `i` of `corners`, the origin for an index outside it (never asked: every index is a position in the
/// ring being clipped).
fn at(corners: &[[f64; 3]], i: usize) -> [f64; 3] {
    corners.get(i).copied().unwrap_or([0.0; 3])
}

/// The polygon's Newell vector: its normal scaled by twice its area, for a polygon whose corners run counter-clockwise
/// seen from the normal's side.
pub(crate) fn newell(corners: &[[f64; 3]]) -> [f64; 3] {
    let mut normal = [0.0f64; 3];
    for (i, &p) in corners.iter().enumerate() {
        let q = at(
            corners,
            if i.saturating_add(1) == corners.len() {
                0
            } else {
                i.saturating_add(1)
            },
        );
        normal[0] += (p[1] - q[1]) * (p[2] + q[2]);
        normal[1] += (p[2] - q[2]) * (p[0] + q[0]);
        normal[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    normal
}

/// The projection plane of a polygon whose Newell vector is `normal`: the two axes left when the largest component of
/// the normal is dropped (`u`, `v`, in cyclic order) and a sign, which is -1 when that component is negative. A
/// polygon that turns the way its normal says then has positive area in the plane (u, v) once its `u` is multiplied by
/// the sign.
pub(crate) fn plane(normal: [f64; 3]) -> (usize, usize, f64) {
    let [x, y, z] = normal.map(f64::abs);
    let (axis, u, v) = if x >= y && x >= z {
        (0, 1, 2)
    } else if y >= z {
        (1, 2, 0)
    } else {
        (2, 0, 1)
    };
    let component = normal.get(axis).copied().unwrap_or(0.0);
    (u, v, if component >= 0.0 { 1.0 } else { -1.0 })
}

/// Triangulate the polygon with these `corners` in order: the triangles as triples of indices into `corners`, and
/// whether ear clipping finished. A polygon of fewer than three corners gives none.
///
/// Every corner that turns the way the polygon does (or is straight) and has no other corner of the ring inside its
/// triangle is cut off in turn; a cut of zero area (a straight corner) is dropped, so its triangle is not returned.
/// Corners at the same position as one of the triangle's own do not block it, since a face may touch itself at a
/// repeated vertex. If no ear is left (a polygon that is not simple, or one with no area) the rest is fanned from its
/// first corner and the second result is `false`.
///
/// On the 279,053 faces of Sekiro's navmeshes and the 6,317,094 of Elden Ring's, ear clipping finishes on all but two
/// (zero-area faces of Elden Ring), no triangle turns against its polygon in the projection plane, and the projected
/// triangle areas add up to the polygon's within 1e-9 of it.
pub fn triangulate(corners: &[[f64; 3]]) -> (Vec<[usize; 3]>, bool) {
    let count = corners.len();
    if count < 3 {
        return (Vec::new(), true);
    }
    let (u, v, sign) = plane(newell(corners));
    // twice the signed area of the triangle a, b, c in the projection plane
    let turn = |a: usize, b: usize, c: usize| -> f64 {
        let (a, b, c) = (at(corners, a), at(corners, b), at(corners, c));
        let side = |p: [f64; 3], q: [f64; 3], r: [f64; 3]| {
            (q[u] - p[u]) * (r[v] - q[v]) - (q[v] - p[v]) * (r[u] - q[u])
        };
        sign * side(a, b, c)
    };
    // twice the signed area of the triangle a, b, p: positive when p is on the inner side of the directed edge a-b
    let inside = |a: usize, b: usize, p: [f64; 3]| -> f64 {
        let (a, b) = (at(corners, a), at(corners, b));
        sign * ((b[u] - a[u]) * (p[v] - a[v]) - (b[v] - a[v]) * (p[u] - a[u]))
    };
    let mut ring: Vec<usize> = (0..count).collect();
    let mut triangles = Vec::with_capacity(count.saturating_sub(2));
    while ring.len() > 3 {
        let len = ring.len();
        let mut clipped = None;
        for i in 0..len {
            let previous = ring.get(if i == 0 {
                len.saturating_sub(1)
            } else {
                i.saturating_sub(1)
            });
            let current = ring.get(i);
            let next = ring.get(if i.saturating_add(1) == len {
                0
            } else {
                i.saturating_add(1)
            });
            let (Some(&a), Some(&b), Some(&c)) = (previous, current, next) else {
                continue;
            };
            let area = turn(a, b, c);
            if area < 0.0 {
                continue;
            }
            let blocked = ring.iter().any(|&other| {
                if other == a || other == b || other == c {
                    return false;
                }
                let position = at(corners, other);
                if [a, b, c].iter().any(|&own| at(corners, own) == position) {
                    return false;
                }
                inside(a, b, position) >= 0.0
                    && inside(b, c, position) >= 0.0
                    && inside(c, a, position) >= 0.0
            });
            if !blocked {
                clipped = Some((i, [a, b, c], area));
                break;
            }
        }
        let Some((i, triangle, area)) = clipped else {
            for pair in ring.get(1..).unwrap_or_default().windows(2) {
                if let (Some(&first), Some(&second), Some(&origin)) =
                    (pair.first(), pair.get(1), ring.first())
                {
                    triangles.push([origin, first, second]);
                }
            }
            return (triangles, false);
        };
        if area > 0.0 {
            triangles.push(triangle);
        }
        ring.remove(i);
    }
    if let &[a, b, c] = ring.as_slice()
        && turn(a, b, c) != 0.0
    {
        triangles.push([a, b, c]);
    }
    (triangles, true)
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test fixtures built by hand"
)]
mod tests {
    use super::*;

    /// Twice the projected area of a triangle list over the polygon's own plane.
    fn area(corners: &[[f64; 3]], triangles: &[[usize; 3]]) -> f64 {
        let (u, v, sign) = plane(newell(corners));
        triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| corners[i]);
                sign * ((b[u] - a[u]) * (c[v] - a[v]) - (b[v] - a[v]) * (c[u] - a[u]))
            })
            .sum()
    }

    #[test]
    fn a_square_is_two_triangles() {
        let square = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]];
        let (triangles, clean) = triangulate(&square);
        assert!(clean);
        assert_eq!(triangles.len(), 2);
        assert!((area(&square, &triangles) - area(&square, &[[0, 1, 2], [0, 2, 3]])).abs() < 1e-12);
    }

    /// The signed turn of each triangle (twice its area in the polygon's projection plane).
    fn turns(corners: &[[f64; 3]], triangles: &[[usize; 3]]) -> Vec<f64> {
        let (u, v, sign) = plane(newell(corners));
        triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| corners[i]);
                sign * ((b[u] - a[u]) * (c[v] - a[v]) - (b[v] - a[v]) * (c[u] - a[u]))
            })
            .collect()
    }

    #[test]
    fn a_u_shape_is_cut_around_its_notch_where_a_fan_turns_back() {
        // counter-clockwise seen from +y (x right, z away from the viewer upward on the page): a 4 by 4 square with a
        // notch 2 wide and 3 deep cut from its far side
        let u = [
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 0.0, -4.0],
            [3.0, 0.0, -4.0],
            [3.0, 0.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 0.0, -4.0],
            [0.0, 0.0, -4.0],
        ];
        let (triangles, clean) = triangulate(&u);
        assert!(clean);
        assert_eq!(triangles.len(), 6);
        // the U covers 16 - 6 = 10 square units: twice the area is 20, and no triangle turns back
        let cut = turns(&u, &triangles);
        assert!(cut.iter().all(|&t| t > 0.0), "{cut:?}");
        assert!((cut.iter().sum::<f64>() - 20.0).abs() < 1e-12);
        // a fan from corner 0 has the same signed total but one triangle (0, 3, 4) that turns against the polygon
        let fan: Vec<[usize; 3]> = (1..7).map(|k| [0, k, k + 1]).collect();
        let fanned = turns(&u, &fan);
        assert!((fanned.iter().sum::<f64>() - 20.0).abs() < 1e-12);
        assert!(fanned.iter().any(|&t| t < 0.0), "{fanned:?}");
    }

    #[test]
    fn a_straight_corner_adds_no_triangle() {
        let line = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 0.0, 2.0],
            [0.0, 0.0, 2.0],
        ];
        let (triangles, clean) = triangulate(&line);
        assert!(clean);
        assert_eq!(triangles.len(), 2);
        assert!((area(&line, &triangles).abs() - 8.0).abs() < 1e-12);
    }

    #[test]
    fn fewer_than_three_corners_and_a_polygon_without_area_give_nothing_wrong() {
        assert_eq!(triangulate(&[[0.0; 3], [1.0; 3]]), (Vec::new(), true));
        let flat = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        let (triangles, _) = triangulate(&flat);
        assert!(triangles.iter().all(|t| t.iter().all(|&i| i < flat.len())));
    }

    #[test]
    fn a_polygon_wound_the_other_way_triangulates_the_same() {
        let square = [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 0.0, 0.0]];
        let (triangles, clean) = triangulate(&square);
        assert!(clean);
        assert_eq!(triangles.len(), 2);
    }
}
