use aurora_console::BulkSummary;

const NOTHING_TO_DO_DIALOG: &str = "Nothing to do";
const NOTHING_TO_DO_STATUS: &str = "Bulk covers: nothing to do";
const STATUS_PREFIX: &str = "Bulk covers: ";

/// One count of a finished run with its wording in the finished dialog and in the status line.
struct OutcomeCount {
    count: usize,
    dialog_label: &'static str,
    status_label: &'static str,
}

/// The finished dialog's body: one line per outcome that happened, in a fixed order.
pub(super) fn finished_body(summary: &BulkSummary) -> String {
    let lines: Vec<String> = outcome_counts(summary)
        .iter()
        .filter(|outcome| outcome.count > 0)
        .map(|outcome| format!("{}: {}", outcome.dialog_label, outcome.count))
        .collect();
    if lines.is_empty() {
        return NOTHING_TO_DO_DIALOG.to_owned();
    }
    lines.join("\n")
}

/// The status line after a run: the same outcomes, comma-separated.
pub(super) fn status_text(summary: &BulkSummary) -> String {
    let parts: Vec<String> = outcome_counts(summary)
        .iter()
        .filter(|outcome| outcome.count > 0)
        .map(|outcome| format!("{} {}", outcome.count, outcome.status_label))
        .collect();
    if parts.is_empty() {
        return NOTHING_TO_DO_STATUS.to_owned();
    }
    format!("{STATUS_PREFIX}{}", parts.join(", "))
}

fn outcome_counts(summary: &BulkSummary) -> [OutcomeCount; 6] {
    [
        OutcomeCount {
            count: summary.uploaded,
            dialog_label: "Saved to the console",
            status_label: "saved",
        },
        OutcomeCount {
            count: summary.already_had_cover,
            dialog_label: "Already had a cover",
            status_label: "already had a cover",
        },
        OutcomeCount {
            count: summary.unreadable,
            dialog_label: "Cover on the console could not be read",
            status_label: "cover on the console unreadable",
        },
        OutcomeCount {
            count: summary.no_cover_online,
            dialog_label: "No cover online",
            status_label: "no cover online",
        },
        OutcomeCount {
            count: summary.failed,
            dialog_label: "Failed",
            status_label: "failed",
        },
        OutcomeCount {
            count: summary.not_handled,
            dialog_label: "Not handled",
            status_label: "not handled",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_summary() -> BulkSummary {
        BulkSummary {
            uploaded: 1,
            already_had_cover: 2,
            unreadable: 3,
            no_cover_online: 4,
            failed: 5,
            not_handled: 6,
        }
    }

    #[test]
    fn finished_body_lists_every_outcome_in_order() {
        assert_eq!(
            finished_body(&full_summary()),
            "Saved to the console: 1\n\
             Already had a cover: 2\n\
             Cover on the console could not be read: 3\n\
             No cover online: 4\n\
             Failed: 5\n\
             Not handled: 6"
        );
    }

    #[test]
    fn finished_body_leaves_out_zero_counts() {
        let summary = BulkSummary {
            uploaded: 1,
            no_cover_online: 1,
            ..BulkSummary::default()
        };
        assert_eq!(
            finished_body(&summary),
            "Saved to the console: 1\nNo cover online: 1"
        );
    }

    #[test]
    fn finished_body_of_an_empty_run_says_nothing_to_do() {
        assert_eq!(finished_body(&BulkSummary::default()), "Nothing to do");
    }

    #[test]
    fn status_text_lists_every_outcome_in_order() {
        assert_eq!(
            status_text(&full_summary()),
            "Bulk covers: 1 saved, 2 already had a cover, 3 cover on the console unreadable, 4 no cover online, 5 failed, 6 not handled"
        );
    }

    #[test]
    fn status_text_leaves_out_zero_counts() {
        let summary = BulkSummary {
            uploaded: 1,
            already_had_cover: 1,
            no_cover_online: 1,
            failed: 1,
            ..BulkSummary::default()
        };
        assert_eq!(
            status_text(&summary),
            "Bulk covers: 1 saved, 1 already had a cover, 1 no cover online, 1 failed"
        );
    }

    #[test]
    fn status_text_of_an_empty_run_says_nothing_to_do() {
        assert_eq!(
            status_text(&BulkSummary::default()),
            "Bulk covers: nothing to do"
        );
    }
}
