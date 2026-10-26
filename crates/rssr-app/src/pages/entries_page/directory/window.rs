//! Variable-height article windows. Geometry is supplied by the host; no DOM or I/O.
use std::ops::Range;

const ESTIMATED_HEIGHT: f64 = 56.0;
const OVERSCAN: f64 = 240.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Part {
    Space { start: usize, height: f64 },
    Row(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct View {
    pub parts: Vec<Part>,
}

pub(super) struct Window {
    heights: Vec<f64>,
    offsets: Vec<f64>,
    width: f64,
    gap: f64,
    top: f64,
    viewport: f64,
    focused: Option<usize>,
}

impl Window {
    pub fn new(count: usize) -> Self {
        let mut value = Self {
            heights: vec![ESTIMATED_HEIGHT; count],
            offsets: Vec::new(),
            width: 0.0,
            gap: 0.0,
            top: 0.0,
            viewport: 0.0,
            focused: None,
        };
        value.reindex();
        value
    }

    fn reindex(&mut self) {
        self.offsets.clear();
        self.offsets.push(0.0);
        for height in &self.heights {
            self.offsets.push(self.offsets.last().unwrap() + height + self.gap);
        }
    }

    fn row_at(&self, y: f64) -> usize {
        self.offsets
            .partition_point(|offset| *offset <= y)
            .saturating_sub(1)
            .min(self.heights.len())
    }

    /// Preserve the first visible logical row when measured heights change above it.
    /// Bad host measurements are ignored rather than poisoning all future offsets.
    pub fn measure(
        &mut self,
        top: f64,
        viewport: f64,
        width: f64,
        gap: f64,
        rows: &[(usize, f64)],
        focused: Option<usize>,
    ) -> f64 {
        if ![top, viewport, width, gap].iter().all(|n| n.is_finite())
            || viewport <= 0.0
            || width <= 0.0
            || gap < 0.0
        {
            return 0.0;
        }
        let anchor = self.row_at(top.max(0.0));
        let old_offset = self.offsets[anchor];
        let mut changed = self.gap != gap;
        if self.width > 0.0 && (self.width - width).abs() > 0.5 {
            // Estimates are not a fixed row-height contract. Re-measure after reflow.
            self.heights.fill(ESTIMATED_HEIGHT);
            changed = true;
        }
        self.width = width;
        self.gap = gap;
        for &(index, height) in rows {
            if height.is_finite()
                && (1.0..=10_000.0).contains(&height)
                && let Some(old) = self.heights.get_mut(index)
            {
                changed |= *old != height;
                *old = height;
            }
        }
        if changed {
            self.reindex();
        }
        let delta = if top > 0.0 && anchor < self.heights.len() {
            self.offsets[anchor] - old_offset
        } else {
            0.0
        };
        self.top = top + delta;
        self.viewport = viewport;
        self.focused = focused.filter(|index| *index < self.heights.len());
        delta
    }

    pub fn reveal(&mut self, index: usize, focus: bool) -> Option<f64> {
        let &height = self.heights.get(index)?;
        if focus {
            self.focused = Some(index);
        }
        let offset = self.offsets[index];
        // A follow target can arrive before the first host measurement.
        self.viewport = self.viewport.max(1.0);
        if offset < self.top || offset + height > self.top + self.viewport {
            self.top = offset;
        }
        Some(self.top)
    }

    pub fn view(&self) -> View {
        let count = self.heights.len();
        let end = self
            .offsets
            .partition_point(|offset| *offset < self.top + self.viewport + OVERSCAN)
            .min(count);
        let start = self.row_at((self.top - OVERSCAN).max(0.0)).min(end);
        let visible = if self.viewport > 0.0 { start..end } else { 0..0 };
        let mut rows: Vec<usize> = visible.collect();
        if let Some(index) = self.focused
            && let Err(position) = rows.binary_search(&index)
        {
            rows.insert(position, index);
        }
        let mut parts = Vec::with_capacity(rows.len() + 3);
        let mut next = 0;
        for index in rows {
            self.spacer(next..index, &mut parts);
            parts.push(Part::Row(index));
            next = index + 1;
        }
        self.spacer(next..count, &mut parts);
        View { parts }
    }

    fn spacer(&self, range: Range<usize>, parts: &mut Vec<Part>) {
        if !range.is_empty() {
            parts.push(Part::Space {
                start: range.start,
                height: (self.offsets[range.end] - self.offsets[range.start] - self.gap).max(0.0),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows(window: &Window) -> Vec<usize> {
        window
            .view()
            .parts
            .into_iter()
            .filter_map(|part| if let Part::Row(i) = part { Some(i) } else { None })
            .collect()
    }
    #[test]
    fn mounts_a_bounded_window_and_keeps_an_offscreen_focus_without_mounting_the_gap() {
        let mut window = Window::new(600);
        window.measure(0.0, 600.0, 180.0, 0.0, &[], None);
        assert!(rows(&window).len() < 20);
        window.measure(28_000.0, 600.0, 180.0, 0.0, &[], Some(0));
        let visible = rows(&window);
        assert_eq!(visible[0], 0);
        assert!(visible[1] > 490 && visible.len() < 25);
        assert!(
            window.view().parts.iter().any(|part| matches!(part, Part::Space { start: 1, .. }))
        );
    }
    #[test]
    fn variable_heights_keep_the_visible_anchor_and_total_spacing() {
        let mut window = Window::new(5);
        let delta =
            window.measure(112.0, 80.0, 180.0, 4.0, &[(0, 80.0), (1, 40.0), (2, 60.0)], None);
        assert_eq!(delta, 16.0); // 80 + 4 + 40 + 4 - (56 + 56)
        assert_eq!(window.row_at(112.0 + delta), 2);
        let parts = window.view().parts;
        let total: f64 = parts
            .iter()
            .map(|part| match part {
                Part::Space { height, .. } => *height,
                Part::Row(index) => window.heights[*index],
            })
            .sum::<f64>()
            + (parts.len() - 1) as f64 * window.gap;
        assert_eq!(total, 80.0 + 40.0 + 60.0 + 56.0 + 56.0 + 16.0);
    }
    #[test]
    fn reveal_reaches_unmounted_last_row_and_resize_invalidates_measurements() {
        let mut window = Window::new(600);
        window.measure(0.0, 600.0, 200.0, 0.0, &[(0, 100.0)], None);
        assert_eq!(window.reveal(599, true), Some(599.0 * 56.0 + 44.0));
        assert!(rows(&window).contains(&599));
        window.measure(0.0, 600.0, 120.0, 0.0, &[(0, 140.0)], None);
        assert_eq!(window.heights[0], 140.0);
        assert_eq!(window.heights[1], ESTIMATED_HEIGHT);
        assert_eq!(window.reveal(600, false), None);
    }
    #[test]
    fn reveal_before_first_measurement_mounts_the_target() {
        let mut window = Window::new(600);
        assert!(window.reveal(599, false).is_some());
        assert!(rows(&window).contains(&599));
        assert!(rows(&window).len() < 10);
    }
    #[test]
    fn invalid_geometry_does_not_break_subsequent_input_and_empty_lists_are_safe() {
        let mut window = Window::new(0);
        assert_eq!(window.measure(0.0, 500.0, 180.0, 0.0, &[], None), 0.0);
        assert!(window.view().parts.is_empty());
        let mut window = Window::new(600);
        window.measure(f64::NAN, 500.0, 180.0, 0.0, &[], None);
        window.measure(0.0, 500.0, 180.0, 0.0, &[(0, f64::INFINITY), (1, -1.0)], None);
        assert!(rows(&window).len() < 20);
        assert_eq!(window.heights[0], ESTIMATED_HEIGHT);
    }
}
