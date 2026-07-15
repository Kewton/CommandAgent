#[cfg(test)]
mod cases {
    use std::collections::BTreeMap;

    use super::super::*;

    fn bind_to_values(
        report_path: &str,
        text: &str,
        values: &BTreeMap<String, f64>,
    ) -> Vec<ClaimBinding> {
        bind_report_claims_to_candidates(report_path, text, &candidates::from_values(values))
    }

    #[test]
    fn binds_grouped_percent_and_unit_claims_by_printed_precision() {
        let values = BTreeMap::from([
            ("total".to_string(), 1234.567),
            ("rate_percent".to_string(), 40.0),
            ("count".to_string(), 3.2),
        ]);

        let claims = bind_to_values(
            "output/report.md",
            "Total 1,234.57 USD; rate 40%; count 3 件.",
            &values,
        );

        assert_eq!(claims.len(), 3);
        assert!(claims.iter().all(|claim| claim.ok), "{claims:?}");
        assert_eq!(claims[0].matched_key.as_deref(), Some("total"));
        assert_eq!(claims[0].unit.as_deref(), Some("USD"));
        assert!(claims[1].percent);
        assert_eq!(claims[2].matched_key.as_deref(), Some("count"));
    }

    #[test]
    fn percent_is_percentage_points_not_an_ambiguous_fraction() {
        let values = BTreeMap::from([("fraction".to_string(), 0.4)]);
        let claims = bind_to_values("output/report.md", "Rate 40%", &values);
        assert!(!claims[0].ok);
    }

    #[test]
    fn html_markup_script_and_style_numbers_are_not_claims() {
        let values = BTreeMap::from([("total".to_string(), 12.5)]);
        let html = r#"<html><style>.x{width:99px}</style><script>const x=88</script><h1>Total <b>12.5</b></h1></html>"#;
        let claims = bind_to_values("output/report.html", html, &values);
        assert_eq!(claims.len(), 1);
        assert!(claims[0].ok);
    }

    #[test]
    fn markdown_link_destinations_are_not_numeric_claims() {
        let values = BTreeMap::from([("total".to_string(), 7.0)]);
        let claims = bind_to_values(
            "output/report.md",
            "[source](https://example.test/2026/07) Total 7",
            &values,
        );
        assert_eq!(claims.len(), 1);
        assert!(claims[0].ok);
    }
}
