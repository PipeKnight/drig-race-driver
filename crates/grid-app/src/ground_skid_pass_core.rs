//! Pure singleton-head and terminal-gate checks for ground-skid early pass.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub owned: bool,
    pub len: usize,
}
/// None is a normal absent/empty/unvisited skid, never force it visible.
pub(crate) fn singleton_head(rows: &[Entry]) -> Result<Option<usize>, &'static str> {
    if rows.iter().filter(|r| r.owned).count() > 1 {
        return Err("multiple owned phase items");
    }
    let mut found = None;
    let mut i = 0usize;
    while i < rows.len() {
        let n = rows[i].len;
        if n == 0 {
            i += 1;
            continue;
        }
        let end = i
            .checked_add(n)
            .filter(|end| *end <= rows.len())
            .ok_or("invalid batch span")?;
        if rows[i..end].iter().any(|r| r.owned) {
            if !rows[i].owned || n != 1 {
                return Err("owned draw not isolated singleton head");
            }
            found = Some(i);
        }
        i = end;
    }
    Ok(found)
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Gate<T> {
    pub ticket: Option<T>,
    pub attempted: bool,
    pub running: bool,
    pub success: bool,
    pub fatal: Option<&'static str>,
}
impl<T> Default for Gate<T> {
    fn default() -> Self {
        Self {
            ticket: None,
            attempted: false,
            running: false,
            success: false,
            fatal: None,
        }
    }
}
impl<T: Copy + Eq> Gate<T> {
    pub fn fail(&mut self, why: &'static str) {
        self.fatal.get_or_insert(why);
    }
    pub fn arm(&mut self, ticket: T) -> Result<(), &'static str> {
        if self.fatal.is_some() || self.ticket.is_some() {
            return Err("gate not clear");
        }
        self.ticket = Some(ticket);
        self.attempted = false;
        self.running = false;
        self.success = false;
        Ok(())
    }
    pub fn begin(&mut self, ticket: T) -> bool {
        if self.fatal.is_some() || self.ticket != Some(ticket) || self.attempted {
            return false;
        }
        self.attempted = true;
        self.running = true;
        true
    }
    /// Called ONLY after nested DrawMaterial terminal success. Never returns Failure
    /// after GPU draw emission. Even a duplicate marker retains success to avoid double draw.
    pub fn terminal(&mut self, ticket: T) {
        if self.ticket != Some(ticket) || !self.running {
            self.fail("unexpected terminal marker");
            return;
        }
        if self.success {
            self.fail("duplicate terminal marker");
        }
        self.success = true;
    }
    pub fn end(&mut self, ticket: T) {
        if self.ticket != Some(ticket) || !self.running {
            self.fail("unexpected replay end");
            return;
        }
        self.running = false;
    }
    pub fn skip_ordinary(&self, ticket: T) -> bool {
        self.ticket == Some(ticket) && self.success
    }
    pub fn restored(&mut self, ticket: T) -> Result<(), &'static str> {
        if self.ticket != Some(ticket) || self.running {
            return Err("restore scope/running differs");
        }
        self.ticket = None;
        self.attempted = false;
        self.success = false;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn e(owned: bool, len: usize) -> Entry {
        Entry { owned, len }
    }
    #[test]
    fn empty_and_unvisited_owned_are_normal() {
        assert_eq!(singleton_head(&[]), Ok(None));
        assert_eq!(singleton_head(&[e(true, 0), e(false, 1)]), Ok(None));
    }
    #[test]
    fn ready_singleton_is_exact_head() {
        assert_eq!(
            singleton_head(&[e(false, 2), e(false, 1), e(true, 1)]),
            Ok(Some(2))
        );
    }
    #[test]
    fn foreign_head_covering_owned_is_refused() {
        assert!(singleton_head(&[e(false, 2), e(true, 1)]).is_err());
    }
    #[test]
    fn merged_owned_head_is_refused() {
        assert!(singleton_head(&[e(true, 2), e(false, 1)]).is_err());
    }
    #[test]
    fn duplicate_owner_and_overflow_fail() {
        assert!(singleton_head(&[e(true, 1), e(true, 1)]).is_err());
        assert!(singleton_head(&[e(false, usize::MAX)]).is_err());
    }
    #[test]
    fn prepare_ready_does_not_skip() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(!g.skip_ordinary(7));
    }
    #[test]
    fn node_not_run_preserves_ordinary() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(!g.skip_ordinary(7));
        g.restored(7).unwrap();
        assert_eq!(g.ticket, None);
    }
    #[test]
    fn upstream_skip_ok_without_marker_preserves_ordinary() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(g.begin(7));
        g.end(7);
        assert!(!g.skip_ordinary(7));
    }
    #[test]
    fn terminal_success_skips_only_matching_scope() {
        let mut g = Gate::default();
        g.arm((1, 2, 3)).unwrap();
        assert!(g.begin((1, 2, 3)));
        g.terminal((1, 2, 3));
        g.end((1, 2, 3));
        assert!(g.skip_ordinary((1, 2, 3)));
        assert!(!g.skip_ordinary((2, 2, 3)));
        assert!(!g.skip_ordinary((1, 4, 3)));
    }
    #[test]
    fn no_double_replay_attempt() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(g.begin(7));
        g.terminal(7);
        g.end(7);
        assert!(!g.begin(7));
        assert!(g.skip_ordinary(7));
    }
    #[test]
    fn late_failure_never_clears_success() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(g.begin(7));
        g.terminal(7);
        g.fail("late failure");
        g.end(7);
        assert!(g.skip_ordinary(7));
        assert!(g.fatal.is_some());
    }
    #[test]
    fn wrong_terminal_cannot_mask_other_item() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(g.begin(7));
        g.terminal(8);
        g.end(7);
        assert!(!g.skip_ordinary(7));
        assert!(g.fatal.is_some());
    }
    #[test]
    fn restore_scope_failure_is_atomic() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(g.restored(8).is_err());
        assert_eq!(g.ticket, Some(7));
    }
    #[test]
    fn reset_next_frame_cannot_reuse_success() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(g.begin(7));
        g.terminal(7);
        g.end(7);
        g.restored(7).unwrap();
        g.arm(8).unwrap();
        assert!(!g.skip_ordinary(7));
        assert!(!g.skip_ordinary(8));
    }
    #[test]
    fn second_view_is_independent() {
        let mut g = Gate::default();
        g.arm((5, 100)).unwrap();
        assert!(g.begin((5, 100)));
        g.terminal((5, 100));
        g.end((5, 100));
        assert!(!g.skip_ordinary((5, 101)));
    }
    #[test]
    fn pending_or_failed_gate_cannot_rearm() {
        let mut g = Gate::default();
        g.arm(7).unwrap();
        assert!(g.arm(8).is_err());
        g.restored(7).unwrap();
        g.fail("failure");
        assert!(g.arm(8).is_err());
    }
}
