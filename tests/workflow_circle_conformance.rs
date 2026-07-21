use commandagent::workflow::runner::{EdgeEvidence, circle_adjudication, edge_earned};
use commandagent::workflow::schema::{Route, Verdict};

fn route() -> Route {
    Route {
        from: "i".into(),
        on: Verdict::Full,
        when: None,
        to: "f".into(),
        carry: vec![],
    }
}
fn evidence() -> EdgeEvidence {
    EdgeEvidence {
        verdict: Verdict::Full,
        evidence: true,
        adjudicated: true,
        epoch: 2,
        previous_epoch: 1,
        carry_present: true,
    }
}

#[test]
fn earned_edge_positive() {
    assert!(edge_earned(&route(), "i_to_f", &evidence()).is_ok());
}
#[test]
fn label_only_rejected() {
    assert!(
        edge_earned(
            &route(),
            "i_to_f",
            &EdgeEvidence {
                evidence: false,
                ..evidence()
            }
        )
        .is_err()
    );
}
#[test]
fn lineage_or_carry_rejected() {
    assert!(
        edge_earned(
            &route(),
            "i_to_f",
            &EdgeEvidence {
                carry_present: false,
                ..evidence()
            }
        )
        .is_err()
    );
}
#[test]
fn epoch_reversal_rejected() {
    assert!(
        edge_earned(
            &route(),
            "i_to_f",
            &EdgeEvidence {
                epoch: 1,
                ..evidence()
            }
        )
        .is_err()
    );
}
#[test]
fn fix_full_does_not_close_circle() {
    assert_eq!(circle_adjudication(false, None).0, "circle_failed");
}
#[test]
fn verify_origin_closes_circle() {
    assert_eq!(circle_adjudication(true, None).0, "circle_full");
}
