//! The JSON examples of the user guide page `profile-tools.md` (Issue #1003), read from both
//! language pages and decoded by the code that reads such files, so an example cannot drift
//! from the format. Hashes in the descriptor example are placeholders: only the decode is checked.

use crate::structured_budget::policy::RunBudgetPolicy;
use crate::structured_budget::policy_v2::{RunBudgetPolicyV2, names_protocol_v2};
use crate::tools::profile_tools::descriptor::Descriptor;

const PAGES: [(&str, &str); 2] = [
    ("en", include_str!("../docs/guide/en/profile-tools.md")),
    ("ja", include_str!("../docs/guide/ja/profile-tools.md")),
];

/// The budget ID both budget examples declare.
const EXAMPLE_BUDGET_ID: &str = "wallet-example";

/// The body of the fenced block whose opening line is exactly `` ```json <name> ``.
fn example(page: &str, name: &str) -> String {
    let opening = format!("```json {name}");
    let mut lines = page.lines();
    assert!(
        lines.any(|line| line == opening),
        "the page has no `{opening}` block"
    );
    let body = lines
        .take_while(|line| *line != "```")
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!body.trim().is_empty(), "`{opening}` block is empty");
    body
}

#[test]
fn the_descriptor_example_decodes_as_a_v1_descriptor() {
    for (language, page) in PAGES {
        let body = example(page, "descriptor-example");
        let descriptor: Descriptor = serde_json::from_str(&body)
            .unwrap_or_else(|error| panic!("{language}: descriptor example: {error}"));
        assert_eq!(
            descriptor.protocol, "profile-tool.descriptor/v1",
            "{language}"
        );
    }
}

#[test]
fn the_v1_budget_example_is_routed_to_and_accepted_by_the_v1_decoder() {
    for (language, page) in PAGES {
        let body = example(page, "run-budget-v1-example");
        assert!(
            !names_protocol_v2(body.as_bytes()),
            "{language}: the v1 example must not name v2"
        );
        let policy = RunBudgetPolicy::decode(body.as_bytes(), EXAMPLE_BUDGET_ID)
            .unwrap_or_else(|refusal| panic!("{language}: v1 example refused: {refusal:?}"));
        assert_eq!(policy.budget_id, EXAMPLE_BUDGET_ID, "{language}");
    }
}

#[test]
fn the_v2_budget_example_is_routed_to_and_accepted_by_the_v2_decoder() {
    for (language, page) in PAGES {
        let body = example(page, "run-budget-v2-example");
        assert!(
            names_protocol_v2(body.as_bytes()),
            "{language}: the v2 example must name v2"
        );
        let policy = RunBudgetPolicyV2::decode(body.as_bytes(), EXAMPLE_BUDGET_ID)
            .unwrap_or_else(|refusal| panic!("{language}: v2 example refused: {refusal:?}"));
        assert_eq!(policy.budget_id, EXAMPLE_BUDGET_ID, "{language}");
    }
}

#[test]
fn the_v2_example_names_the_endpoint_the_direct_client_posts_to() {
    // The direct client's base URL and path are private literals in `providers/openai.rs`
    // (`OPENAI_BASE_URL` and `"/v1/responses"`); no `pub(crate)` constant holds the whole
    // endpoint, so the literal is compared here.
    for (language, page) in PAGES {
        let value: serde_json::Value =
            serde_json::from_str(&example(page, "run-budget-v2-example")).unwrap();
        assert_eq!(
            value["route"]["endpoint"], "https://api.openai.com/v1/responses",
            "{language}"
        );
    }
}

#[test]
fn the_two_language_pages_carry_the_same_examples() {
    for name in [
        "descriptor-example",
        "run-budget-v1-example",
        "run-budget-v2-example",
    ] {
        let en: serde_json::Value = serde_json::from_str(&example(PAGES[0].1, name)).unwrap();
        let ja: serde_json::Value = serde_json::from_str(&example(PAGES[1].1, name)).unwrap();
        assert_eq!(en, ja, "{name}");
    }
}
