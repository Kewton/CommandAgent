//! Accepted artifact-check diagnostics as neutral report metadata (Issue #955).
//!
//! Design record:
//!
//! - Meaning. These values never decide anything: `VerificationReport::is_pass`,
//!   the status, the reasons, and every count ignore them. Present, empty,
//!   scrubbed, or omitted diagnostics are neither a failure nor a pass.
//! - Content. Every string here is already a *safe display string*: the
//!   producer (`planner::artifact_contract_checks::diagnostics`) scrubbed the
//!   whole decoded wire value, made control and bidi characters visible, and
//!   encoded `&`, `<`, `>`, and the backtick before anything was bounded. This
//!   module only holds, merges, and bounds them; it knows no domain field
//!   name, adapter, or profile.
//! - Ownership. A group is one receipt: its source, owner, adapter, request,
//!   and request ID stay attached to its items. Groups are never merged or
//!   deduplicated, so equal check/target names from two receipts stay two
//!   entries.
//! - Bounds. One unit holds at most [`MAX_DIAGNOSTICS`] items and
//!   [`MAX_BYTES`] serialized UTF-8 bytes, keys, ownership, omitted counts,
//!   and escapes included. Bounding drops whole items from the end and counts
//!   each dropped item as omitted; strings are never cut, so only well-formed
//!   data remains. Omitted counts only grow: a merge or a re-projection keeps
//!   every earlier omitted count.

use serde::Serialize;

/// Items one unit (an event field, a report, a repair section) may hold.
pub const MAX_DIAGNOSTICS: usize = 16;
/// Serialized UTF-8 bytes one unit may take.
pub const MAX_BYTES: usize = 8_192;

/// One accepted diagnostic of one failing or required non-passing row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ArtifactDiagnostic {
    pub check: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// The row status, one of the fixed protocol status words.
    pub row_status: &'static str,
    pub code: String,
    pub message: String,
}

/// The accepted diagnostics of one receipt, with its ownership.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ArtifactDiagnosticGroup {
    pub source: String,
    pub owner: String,
    pub adapter: String,
    pub request: String,
    pub request_id: Option<String>,
    pub items: Vec<ArtifactDiagnostic>,
    /// Items of this receipt that a bound left out.
    pub omitted: usize,
}

impl ArtifactDiagnosticGroup {
    /// Whether the group carries nothing at all (no item, nothing omitted).
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.omitted == 0
    }

    /// Serialized UTF-8 size of the group object.
    pub fn serialized_len(&self) -> usize {
        serde_json::to_string(self).map_or(usize::MAX, |text| text.len())
    }

    /// Drop items from the end until at most `max_items` remain and
    /// `overhead` plus the serialized group fits `max_bytes`.
    pub fn fit(&mut self, max_items: usize, max_bytes: usize, overhead: usize) {
        while !self.items.is_empty()
            && (self.items.len() > max_items
                || overhead.saturating_add(self.serialized_len()) > max_bytes)
        {
            self.items.pop();
            self.omitted += 1;
        }
    }
}

/// The accepted diagnostics a report carries: one group per receipt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ArtifactDiagnostics {
    pub groups: Vec<ArtifactDiagnosticGroup>,
    /// Items (with their groups) a report-level bound left out, in addition
    /// to each group's own `omitted`.
    pub omitted: usize,
}

impl ArtifactDiagnostics {
    /// Whether nothing is carried and nothing was omitted.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.omitted == 0
    }

    /// Items currently held across every group.
    pub fn item_count(&self) -> usize {
        self.groups.iter().map(|group| group.items.len()).sum()
    }

    /// Every omitted item: report-level plus each group's own count.
    pub fn total_omitted(&self) -> usize {
        self.omitted + self.groups.iter().map(|group| group.omitted).sum::<usize>()
    }

    /// Serialized UTF-8 size of the whole value.
    pub fn serialized_len(&self) -> usize {
        serde_json::to_string(self).map_or(usize::MAX, |text| text.len())
    }

    /// Add one receipt's group, then bound the report.
    pub fn push_group(&mut self, group: ArtifactDiagnosticGroup) {
        if group.is_empty() {
            return;
        }
        self.groups.push(group);
        self.fit(MAX_DIAGNOSTICS, MAX_BYTES, 0);
    }

    /// Append another report's groups, keeping their ownership and every
    /// omitted count, then bound the result.
    pub fn merge(&mut self, other: ArtifactDiagnostics) {
        self.omitted += other.omitted;
        self.groups
            .extend(other.groups.into_iter().filter(|group| !group.is_empty()));
        self.fit(MAX_DIAGNOSTICS, MAX_BYTES, 0);
    }

    /// Drop items from the end (and a group once it holds no item, adding
    /// its own omitted count here) until at most `max_items` remain and
    /// `overhead` plus the serialized value fits `max_bytes`.
    pub fn fit(&mut self, max_items: usize, max_bytes: usize, overhead: usize) {
        while self.item_count() > max_items
            || overhead.saturating_add(self.serialized_len()) > max_bytes
        {
            let Some(group) = self.groups.last_mut() else {
                return;
            };
            if group.items.pop().is_some() {
                group.omitted += 1;
            }
            if group.items.is_empty() {
                let group = self.groups.pop().unwrap_or_default();
                self.omitted += group.omitted;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactDiagnostic, ArtifactDiagnosticGroup, ArtifactDiagnostics, MAX_BYTES,
        MAX_DIAGNOSTICS,
    };

    fn item(check: &str, message: &str) -> ArtifactDiagnostic {
        ArtifactDiagnostic {
            check: check.to_string(),
            target: Some("t1".to_string()),
            row_status: "failed",
            code: "synthetic-code".to_string(),
            message: message.to_string(),
        }
    }

    fn group(
        adapter: &str,
        request_id: &str,
        items: Vec<ArtifactDiagnostic>,
    ) -> ArtifactDiagnosticGroup {
        ArtifactDiagnosticGroup {
            source: "draft_profile".to_string(),
            owner: "owner-a".to_string(),
            adapter: adapter.to_string(),
            request: "check/request.json".to_string(),
            request_id: Some(request_id.to_string()),
            items,
            omitted: 0,
        }
    }

    fn report(groups: Vec<ArtifactDiagnosticGroup>) -> ArtifactDiagnostics {
        let mut report = ArtifactDiagnostics::default();
        for group in groups {
            report.push_group(group);
        }
        report
    }

    #[test]
    fn equal_rows_from_two_receipts_stay_two_owned_entries() {
        let first = group("gate-a", "req-1", vec![item("rows/alpha", "first")]);
        let second = group("gate-b", "req-2", vec![item("rows/alpha", "first")]);
        let merged = report(vec![first.clone(), second.clone()]);
        assert_eq!(merged.groups, vec![first, second]);
        assert_eq!(merged.item_count(), 2);
        assert_eq!(merged.total_omitted(), 0);
    }

    #[test]
    fn an_empty_group_is_not_carried() {
        let merged = report(vec![group("gate-a", "req-1", Vec::new())]);
        assert!(merged.is_empty(), "{merged:?}");
    }

    #[test]
    fn the_item_bound_holds_before_at_and_after_sixteen() {
        for count in [MAX_DIAGNOSTICS - 1, MAX_DIAGNOSTICS, MAX_DIAGNOSTICS + 1] {
            let items = (0..count)
                .map(|index| item(&format!("rows/{index}"), "m"))
                .collect::<Vec<_>>();
            let merged = report(vec![group("gate-a", "req-1", items)]);
            assert_eq!(merged.item_count(), count.min(MAX_DIAGNOSTICS), "{count}");
            assert_eq!(
                merged.total_omitted(),
                count.saturating_sub(MAX_DIAGNOSTICS),
                "{count}"
            );
        }
    }

    /// One item whose message is sized so the whole serialized report is
    /// exactly `target` bytes.
    fn sized(target: usize) -> ArtifactDiagnostics {
        let mut value = report(vec![group("gate-a", "req-1", vec![item("rows/a", "")])]);
        let base = value.serialized_len();
        value.groups[0].items[0].message = "x".repeat(target - base);
        value
    }

    #[test]
    fn the_byte_bound_holds_before_at_and_after_the_limit() {
        for (target, kept) in [(MAX_BYTES - 1, 1), (MAX_BYTES, 1), (MAX_BYTES + 1, 0)] {
            let mut value = sized(target);
            assert_eq!(value.serialized_len(), target);
            value.fit(MAX_DIAGNOSTICS, MAX_BYTES, 0);
            assert_eq!(value.item_count(), kept, "{target}");
            assert_eq!(value.total_omitted(), 1 - kept, "{target}");
            assert!(value.serialized_len() <= MAX_BYTES, "{target}");
        }
    }

    #[test]
    fn escape_growth_is_measured_on_the_serialized_value() {
        // A quote or a backslash takes two serialized bytes.
        let mut value = sized(MAX_BYTES - 1);
        let message = &mut value.groups[0].items[0].message;
        message.pop();
        message.push('"');
        assert_eq!(value.serialized_len(), MAX_BYTES);
        value.fit(MAX_DIAGNOSTICS, MAX_BYTES, 0);
        assert_eq!(value.item_count(), 1);
        let mut value = sized(MAX_BYTES);
        let message = &mut value.groups[0].items[0].message;
        message.pop();
        message.push('\\');
        value.fit(MAX_DIAGNOSTICS, MAX_BYTES, 0);
        assert_eq!(value.item_count(), 0);
        assert_eq!(value.total_omitted(), 1);
    }

    #[test]
    fn overhead_counts_against_the_byte_bound() {
        let mut value = sized(MAX_BYTES - 10);
        value.fit(MAX_DIAGNOSTICS, MAX_BYTES, 10);
        assert_eq!(value.item_count(), 1);
        let mut value = sized(MAX_BYTES - 10);
        value.fit(MAX_DIAGNOSTICS, MAX_BYTES, 11);
        assert_eq!(value.item_count(), 0);
    }

    #[test]
    fn repeated_merges_and_reprojection_keep_every_omitted_count() {
        let mut first = group("gate-a", "req-1", vec![item("rows/a", "a")]);
        first.omitted = 3;
        let mut merged = report(vec![first]);
        merged.omitted = 2;
        let many = (0..MAX_DIAGNOSTICS)
            .map(|index| item(&format!("rows/b{index}"), "b"))
            .collect::<Vec<_>>();
        let mut other = ArtifactDiagnostics::default();
        other.push_group(group("gate-b", "req-2", many));
        other.omitted = 4;
        merged.merge(other);
        // 1 + 16 items, bound 16: one more omitted; 3 + 2 + 4 kept.
        assert_eq!(merged.item_count(), MAX_DIAGNOSTICS);
        assert_eq!(merged.total_omitted(), 3 + 2 + 4 + 1);
        let before = merged.total_omitted();
        merged.merge(ArtifactDiagnostics::default());
        merged.fit(MAX_DIAGNOSTICS, MAX_BYTES, 0);
        assert_eq!(merged.total_omitted(), before);
        // Re-projecting under a tighter bound only adds omitted items.
        merged.fit(4, MAX_BYTES, 0);
        assert_eq!(merged.item_count(), 4);
        assert_eq!(merged.total_omitted(), before + MAX_DIAGNOSTICS - 4);
        assert_eq!(merged.groups[0].adapter, "gate-a");
    }

    #[test]
    fn a_dropped_group_moves_its_omitted_count_to_the_report() {
        let mut first = group("gate-a", "req-1", vec![item("rows/a", "a")]);
        first.omitted = 5;
        let mut merged = report(vec![first]);
        merged.fit(0, MAX_BYTES, 0);
        assert!(merged.groups.is_empty());
        assert_eq!(merged.omitted, 6);
        assert_eq!(merged.total_omitted(), 6);
    }

    #[test]
    fn a_group_fit_drops_whole_items_only() {
        let mut value = group(
            "gate-a",
            "req-1",
            vec![item("rows/a", "aaaa"), item("rows/b", "bbbb")],
        );
        let full = value.serialized_len();
        value.fit(MAX_DIAGNOSTICS, full - 1, 0);
        assert_eq!(value.items, vec![item("rows/a", "aaaa")]);
        assert_eq!(value.omitted, 1);
    }
}
