//! Segment endpoint rendering, segment labels, and the study
//! scope rule. Every reader that reasons per segment scope uses
//! the definitions in this module, so the rule exists once.

use crate::db::Segment;

/// Render one endpoint as the program states it. A bar anchor
/// renders as `b. <value>`; a mark anchor renders as its value,
/// which names the mark (a letter, a number, a double letter).
/// A non-zero offset appends the signed distance in bars.
pub fn render_endpoint(kind: &str, value: &str, offset: i64) -> String {
    let base = match kind {
        "start" => "inizio".to_string(),
        "end" => "fine".to_string(),
        "bar" => format!("b. {value}"),
        _ => value.to_string(),
    };
    match offset {
        0 => base,
        n if n > 0 => format!("{base} +{n} b."),
        n => format!("{base} {n} b."),
    }
}

/// The display name of a segment: its alias when it has one,
/// else its rendered range. A pure bar-to-bar range renders in
/// the compact `bb. <start>-<end>` form.
pub fn segment_label(seg: &Segment) -> String {
    if !seg.alias.is_empty() {
        return seg.alias.clone();
    }
    if seg.start_kind == "bar"
        && seg.end_kind == "bar"
        && seg.start_offset == 0
        && seg.end_offset == 0
    {
        return format!("bb. {}\u{2013}{}", seg.start_value, seg.end_value);
    }
    let start = render_endpoint(&seg.start_kind, &seg.start_value, seg.start_offset);
    let end = render_endpoint(&seg.end_kind, &seg.end_value, seg.end_offset);
    format!("{start}\u{2013}{end}")
}

/// Whether a session counts toward a scope. An empty scope is
/// the whole piece: every session of the piece counts. A
/// non-empty scope is a segment set: a whole-piece session
/// (segment `None`) counts, and a segment session counts only
/// when its segment belongs to the set. Credit flows down from
/// the piece and up from a segment, never sideways.
pub fn counts(session_segment: Option<i64>, scope: &[i64]) -> bool {
    if scope.is_empty() {
        return true;
    }
    match session_segment {
        None => true,
        Some(id) => scope.contains(&id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(alias: &str, sk: &str, sv: &str, so: i64, ek: &str, ev: &str, eo: i64) -> Segment {
        Segment {
            alias: alias.into(),
            start_kind: sk.into(),
            start_value: sv.into(),
            start_offset: so,
            end_kind: ek.into(),
            end_value: ev.into(),
            end_offset: eo,
            ..Default::default()
        }
    }

    #[test]
    fn endpoint_renders_each_anchor_kind() {
        assert_eq!(render_endpoint("start", "", 0), "inizio");
        assert_eq!(render_endpoint("end", "", 0), "fine");
        assert_eq!(render_endpoint("bar", "123", 0), "b. 123");
        assert_eq!(render_endpoint("mark", "RR", 0), "RR");
        assert_eq!(render_endpoint("mark", "97", 0), "97");
    }

    #[test]
    fn endpoint_renders_signed_offsets() {
        assert_eq!(render_endpoint("mark", "RR", 17), "RR +17 b.");
        assert_eq!(render_endpoint("mark", "97", -2), "97 -2 b.");
        assert_eq!(render_endpoint("bar", "295", 0), "b. 295");
    }

    #[test]
    fn label_prefers_the_alias() {
        let s = seg("Cadenza", "bar", "100", 0, "bar", "130", 0);
        assert_eq!(segment_label(&s), "Cadenza");
    }

    #[test]
    fn label_renders_a_compact_bar_range() {
        let s = seg("", "bar", "1", 0, "bar", "123", 0);
        assert_eq!(segment_label(&s), "bb. 1\u{2013}123");
    }

    #[test]
    fn label_renders_mixed_anchors_and_open_ends() {
        let s = seg("", "start", "", 0, "mark", "E", 0);
        assert_eq!(segment_label(&s), "inizio\u{2013}E");
        let s = seg("", "mark", "H", 1, "end", "", 0);
        assert_eq!(segment_label(&s), "H +1 b.\u{2013}fine");
    }

    #[test]
    fn counts_follows_the_scope_rule() {
        // Whole-piece scope: everything counts.
        assert!(counts(None, &[]));
        assert!(counts(Some(7), &[]));
        // Segment scope: whole-piece sessions count.
        assert!(counts(None, &[7, 9]));
        // Segment scope: a member segment counts.
        assert!(counts(Some(9), &[7, 9]));
        // Segment scope: another segment never counts.
        assert!(!counts(Some(8), &[7, 9]));
    }
}
