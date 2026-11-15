//! Squarified treemap layout (Bruls, Huizing and van Wijk).
//!
//! The layout is pure geometry so it can be tested without a graphics context.
//! Items are laid out in rows along the shorter side of the remaining
//! rectangle; a row is closed as soon as adding another item would make the
//! worst aspect ratio worse.

use freespace_core::tree::NodeId;

/// Axis-aligned rectangle in screen space.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// Creates a rectangle.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Area in square pixels.
    #[must_use]
    pub fn area(self) -> f32 {
        self.width * self.height
    }

    /// Right edge.
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Returns `true` when the point lies inside the rectangle.
    #[must_use]
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Shrinks every edge by `amount`, never below zero.
    #[must_use]
    pub fn inset(self, amount: f32) -> Self {
        let amount = amount.max(0.0).min(self.width / 2.0).min(self.height / 2.0);

        Self::new(
            self.x + amount,
            self.y + amount,
            (self.width - amount * 2.0).max(0.0),
            (self.height - amount * 2.0).max(0.0),
        )
    }
}

/// One laid-out node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreemapItem {
    /// Node represented by the rectangle.
    pub node: NodeId,
    /// Screen-space rectangle.
    pub rect: Rect,
}

/// Lays out `items` inside `bounds`.
///
/// Items are ordered by size internally; non-positive and non-finite sizes are
/// ignored. Areas are proportional to sizes.
#[must_use]
pub fn squarify(items: &[(NodeId, f64)], bounds: Rect) -> Vec<TreemapItem> {
    let mut usable: Vec<(NodeId, f64)> = items
        .iter()
        .copied()
        .filter(|(_, size)| size.is_finite() && *size > 0.0)
        .collect();
    usable.sort_by(|left, right| right.1.total_cmp(&left.1));
    if usable.is_empty() || bounds.area() <= f32::EPSILON {
        return Vec::new();
    }

    let total: f64 = usable.iter().map(|(_, size)| *size).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let scale = f64::from(bounds.area()) / total;
    let areas: Vec<f64> = usable.iter().map(|(_, size)| size * scale).collect();

    let mut remaining = bounds;
    let mut start = 0usize;
    let mut end = 0usize;
    let mut result = Vec::with_capacity(usable.len());

    while end < usable.len() {
        let side = f64::from(remaining.width.min(remaining.height));
        if side <= f64::EPSILON {
            break;
        }

        if start == end {
            end += 1;
            continue;
        }

        let current = worst(&areas[start..end], side);
        let extended = worst(&areas[start..end + 1], side);
        if extended <= current {
            end += 1;
        } else {
            layout_row(
                &usable[start..end],
                &areas[start..end],
                &mut remaining,
                &mut result,
            );
            start = end;
        }
    }

    if start < end {
        layout_row(
            &usable[start..end],
            &areas[start..end],
            &mut remaining,
            &mut result,
        );
    }

    result
}

fn worst(areas: &[f64], side: f64) -> f64 {
    let sum: f64 = areas.iter().sum();
    if sum <= f64::EPSILON || side <= f64::EPSILON {
        return f64::INFINITY;
    }
    let side2 = side * side;
    let sum2 = sum * sum;

    areas
        .iter()
        .map(|area| {
            let ratio = side2 * area / sum2;
            let inverse = sum2 / (side2 * area);
            ratio.max(inverse)
        })
        .fold(0.0, f64::max)
}

fn layout_row(
    row: &[(NodeId, f64)],
    areas: &[f64],
    remaining: &mut Rect,
    result: &mut Vec<TreemapItem>,
) {
    let sum: f64 = areas.iter().sum();
    let side = f64::from(remaining.width.min(remaining.height));
    if sum <= f64::EPSILON || side <= f64::EPSILON {
        return;
    }
    let thickness = (sum / side) as f32;

    if remaining.width >= remaining.height {
        let mut y = remaining.y;
        for ((node, _), area) in row.iter().zip(areas) {
            let height = (*area / f64::from(thickness)) as f32;
            let height = height.min(remaining.bottom() - y).max(0.0);
            result.push(TreemapItem {
                node: *node,
                rect: Rect::new(remaining.x, y, thickness, height),
            });
            y += height;
        }
        remaining.x += thickness;
        remaining.width = (remaining.width - thickness).max(0.0);
    } else {
        let mut x = remaining.x;
        for ((node, _), area) in row.iter().zip(areas) {
            let width = (*area / f64::from(thickness)) as f32;
            let width = width.min(remaining.right() - x).max(0.0);
            result.push(TreemapItem {
                node: *node,
                rect: Rect::new(x, remaining.y, width, thickness),
            });
            x += width;
        }
        remaining.y += thickness;
        remaining.height = (remaining.height - thickness).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{Rect, squarify};

    fn bounds() -> Rect {
        Rect::new(0.0, 0.0, 400.0, 300.0)
    }

    #[test]
    fn keeps_areas_proportional() {
        let sizes = [(1, 500.0), (2, 250.0), (3, 150.0), (4, 100.0)];
        let items = squarify(&sizes, bounds());
        let total = bounds().area();

        assert_eq!(items.len(), 4);
        for (node, size) in sizes {
            let item = items
                .iter()
                .find(|item| item.node == node)
                .expect("every node is laid out");
            let expected = total * (size as f32) / 1000.0;
            let actual = item.rect.area();
            assert!(
                (actual - expected).abs() <= expected * 0.02,
                "node {node}: expected {expected}, got {actual}"
            );
        }
    }

    #[test]
    fn keeps_rectangles_inside_the_bounds() {
        let sizes: Vec<(u32, f64)> = (0..40).map(|index| (index, f64::from(index + 1))).collect();
        let bounds = bounds();
        let items = squarify(&sizes, bounds);

        for item in &items {
            assert!(item.rect.width >= 0.0);
            assert!(item.rect.height >= 0.0);
            assert!(item.rect.x >= bounds.x - 0.01);
            assert!(item.rect.y >= bounds.y - 0.01);
            assert!(item.rect.right() <= bounds.right() + 0.01);
            assert!(item.rect.bottom() <= bounds.bottom() + 0.01);
        }
    }

    #[test]
    fn orders_items_by_size() {
        let items = squarify(&[(7, 10.0), (8, 100.0), (9, 50.0)], bounds());

        assert_eq!(items[0].node, 8, "largest item gets the first row");
    }

    #[test]
    fn ignores_invalid_sizes() {
        assert!(squarify(&[], bounds()).is_empty());
        assert!(squarify(&[(1, 0.0), (2, -5.0), (3, f64::NAN)], bounds()).is_empty());
        assert!(squarify(&[(1, 1.0)], Rect::new(0.0, 0.0, 0.0, 10.0)).is_empty());
    }

    #[test]
    fn hits_points_inside_rectangles() {
        let rect = Rect::new(10.0, 20.0, 30.0, 40.0);

        assert!(rect.contains(10.0, 20.0));
        assert!(rect.contains(25.0, 40.0));
        assert!(!rect.contains(9.0, 20.0));
        assert!(!rect.contains(40.1, 20.0));
        assert_eq!(rect.inset(5.0), Rect::new(15.0, 25.0, 20.0, 30.0));
    }
}
