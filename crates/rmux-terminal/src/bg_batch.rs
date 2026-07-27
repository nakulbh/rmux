//! Background run batching: consecutive same-color cells → one rect.

use egui::{Color32, Pos2, Rect, Vec2};

/// One horizontal span of identical background color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BgSpan {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
    pub color: Color32,
}

impl BgSpan {
    pub(crate) fn paint(self, painter: &egui::Painter, pane: Rect, cell_w: f32, cell_h: f32) {
        let cols = self.end_col.saturating_sub(self.start_col);
        if cols == 0 {
            return;
        }
        let r = Rect::from_min_size(
            Pos2::new(
                pane.left() + self.start_col as f32 * cell_w,
                pane.top() + self.row as f32 * cell_h,
            ),
            Vec2::new(cols as f32 * cell_w, cell_h),
        );
        painter.rect_filled(r, 0.0, self.color);
    }
}

/// Accumulates horizontal background runs; `None` color breaks the span.
#[derive(Default)]
pub(crate) struct BgSpanAccumulator {
    current: Option<BgSpan>,
}

impl BgSpanAccumulator {
    /// Note a cell. `color = None` means "no custom bg" (break any open span).
    ///
    /// Returns a finished span ready to paint, if any.
    pub(crate) fn push(
        &mut self,
        row: u16,
        col: u16,
        width: u16,
        color: Option<Color32>,
    ) -> Option<BgSpan> {
        let Some(color) = color else {
            return self.take();
        };
        if color.a() == 0 {
            // Transparent custom bg: break span (do not extend across).
            return self.take();
        }

        match self.current.as_mut() {
            Some(s) if s.row == row && s.end_col == col && s.color == color => {
                s.end_col = col + width;
                None
            }
            _ => {
                let finished = self.current.take();
                self.current = Some(BgSpan { row, start_col: col, end_col: col + width, color });
                finished
            }
        }
    }

    /// End of row / end of grid: flush remaining span.
    pub(crate) fn take(&mut self) -> Option<BgSpan> {
        self.current.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extends_same_color_run() {
        let mut acc = BgSpanAccumulator::default();
        assert!(acc.push(0, 0, 1, Some(Color32::RED)).is_none());
        assert!(acc.push(0, 1, 1, Some(Color32::RED)).is_none());
        let s = acc.take().expect("span");
        assert_eq!(s.start_col, 0);
        assert_eq!(s.end_col, 2);
        assert_eq!(s.color, Color32::RED);
    }

    #[test]
    fn color_change_flushes_previous() {
        let mut acc = BgSpanAccumulator::default();
        assert!(acc.push(0, 0, 1, Some(Color32::RED)).is_none());
        let prev = acc.push(0, 1, 1, Some(Color32::BLUE)).expect("flush red");
        assert_eq!(prev.color, Color32::RED);
        assert_eq!(prev.end_col, 1);
        let blue = acc.take().expect("blue");
        assert_eq!(blue.color, Color32::BLUE);
        assert_eq!(blue.start_col, 1);
    }

    #[test]
    fn none_breaks_span() {
        let mut acc = BgSpanAccumulator::default();
        acc.push(0, 0, 2, Some(Color32::RED));
        let prev = acc.push(0, 2, 1, None).expect("break");
        assert_eq!(prev.end_col, 2);
        assert!(acc.take().is_none());
    }

    #[test]
    fn zero_alpha_breaks_without_extending() {
        let mut acc = BgSpanAccumulator::default();
        acc.push(0, 0, 1, Some(Color32::RED));
        let transparent = Color32::from_rgba_unmultiplied(1, 2, 3, 0);
        let prev = acc.push(0, 1, 1, Some(transparent)).expect("break");
        assert_eq!(prev.end_col, 1);
        // Next red starts fresh — must not bridge across transparent.
        acc.push(0, 2, 1, Some(Color32::RED));
        let s = acc.take().expect("new red");
        assert_eq!(s.start_col, 2);
        assert_eq!(s.end_col, 3);
    }

    #[test]
    fn wide_cell_advances_end_col() {
        let mut acc = BgSpanAccumulator::default();
        acc.push(0, 0, 2, Some(Color32::GREEN));
        let s = acc.take().expect("wide");
        assert_eq!(s.end_col, 2);
    }
}
