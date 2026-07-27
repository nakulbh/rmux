//! Coalesced terminal size: layout can change every frame; PTY/grid reflow is throttled.

/// Tracks layout-desired vs applied PTY/grid size with throttle + settle.
///
/// While the desired size keeps changing, apply at most every
/// [`Self::THROTTLE_SECS`]. Once it settles for [`Self::SETTLE_SECS`], apply
/// so the shell catches the final geometry.
#[derive(Debug, Clone, Copy)]
pub struct CoalescedSize {
    applied: (u16, u16),
    desired: (u16, u16),
    desired_changed_at: f64,
    last_applied_at: f64,
}

impl CoalescedSize {
    /// While size is still moving, reflow at most this often.
    pub const THROTTLE_SECS: f64 = 0.05;
    /// After desired size stops changing, apply within this window.
    pub const SETTLE_SECS: f64 = 0.04;

    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            applied: (cols.max(1), rows.max(1)),
            desired: (cols.max(1), rows.max(1)),
            desired_changed_at: 0.0,
            last_applied_at: 0.0,
        }
    }

    pub fn applied(&self) -> (u16, u16) {
        self.applied
    }

    pub fn desired(&self) -> (u16, u16) {
        self.desired
    }

    pub fn cols(&self) -> u16 {
        self.applied.0
    }

    pub fn rows(&self) -> u16 {
        self.applied.1
    }

    pub fn desired_cols(&self) -> u16 {
        self.desired.0
    }

    pub fn desired_rows(&self) -> u16 {
        self.desired.1
    }

    /// True when layout size differs from the live PTY/grid size.
    pub fn is_pending(&self) -> bool {
        self.desired != self.applied
    }

    /// Update layout target. Returns `true` when desired size changed.
    pub fn set_desired(&mut self, cols: u16, rows: u16, now: f64) -> bool {
        let next = (cols.max(1), rows.max(1));
        if next == self.desired {
            return false;
        }
        self.desired = next;
        self.desired_changed_at = now;
        true
    }

    /// Force applied and desired to the same size (immediate resize).
    pub fn force(&mut self, cols: u16, rows: u16, now: f64) {
        let next = (cols.max(1), rows.max(1));
        self.applied = next;
        self.desired = next;
        self.desired_changed_at = now;
        self.last_applied_at = now;
    }

    /// If a reflow should run now, update applied and return the new size.
    pub fn poll_apply(&mut self, now: f64) -> Option<(u16, u16)> {
        if !self.is_pending() {
            return None;
        }

        let first = self.last_applied_at == 0.0;
        let settled = (now - self.desired_changed_at) >= Self::SETTLE_SECS;
        let throttled = (now - self.last_applied_at) >= Self::THROTTLE_SECS;
        if !(first || settled || throttled) {
            return None;
        }

        self.applied = self.desired;
        self.last_applied_at = now;
        Some(self.applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_desired_detects_change() {
        let mut s = CoalescedSize::new(80, 24);
        assert!(!s.set_desired(80, 24, 1.0));
        assert!(s.set_desired(100, 24, 1.0));
        assert_eq!(s.desired(), (100, 24));
        assert_eq!(s.applied(), (80, 24));
        assert!(s.is_pending());
    }

    #[test]
    fn first_pending_applies_immediately() {
        let mut s = CoalescedSize::new(80, 24);
        s.set_desired(90, 30, 1.0);
        // last_applied_at is 0 → first apply path
        assert_eq!(s.poll_apply(1.0), Some((90, 30)));
        assert!(!s.is_pending());
    }

    #[test]
    fn throttle_blocks_until_interval() {
        let mut s = CoalescedSize::new(80, 24);
        s.set_desired(90, 24, 1.0);
        assert_eq!(s.poll_apply(1.0), Some((90, 24)));

        s.set_desired(100, 24, 1.01);
        // Only 10ms since apply, not settled (desired just changed)
        assert_eq!(s.poll_apply(1.02), None);

        // Throttle window elapsed
        assert_eq!(s.poll_apply(1.0 + CoalescedSize::THROTTLE_SECS), Some((100, 24)));
    }

    #[test]
    fn settle_applies_after_desired_stops() {
        let mut s = CoalescedSize::new(80, 24);
        s.set_desired(90, 24, 10.0);
        assert_eq!(s.poll_apply(10.0), Some((90, 24)));

        // Change again immediately after apply so throttle still blocks.
        s.set_desired(100, 24, 10.01);
        assert_eq!(s.poll_apply(10.03), None); // < throttle and < settle
        // Past settle (and throttle) with margin for f64 noise.
        assert_eq!(s.poll_apply(10.06), Some((100, 24)));
    }

    #[test]
    fn force_clears_pending() {
        let mut s = CoalescedSize::new(80, 24);
        s.set_desired(120, 40, 1.0);
        s.force(120, 40, 1.5);
        assert!(!s.is_pending());
        assert_eq!(s.poll_apply(2.0), None);
    }
}
