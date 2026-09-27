use serde_json::Value;

/// Reads an integer measurement from a JSON field without letting a
/// non-finite or out-of-range float saturate to `i64::MIN`/`i64::MAX`.
/// Unusable numbers are reported as missing, matching the absent-field
/// default at the call site.
pub(super) fn finite_i64_field(value: &Value, name: &str) -> Option<i64> {
    let value = value.get(name)?;
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    let number = value.as_f64()?;
    if !number.is_finite() || number < i64::MIN as f64 || number >= 9_223_372_036_854_775_808.0 {
        return None;
    }
    Some(number.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::super::surface_fit_overflows;
    use super::*;
    use serde_json::json;

    #[test]
    fn integral_and_fractional_measurements_are_accepted() {
        assert_eq!(
            finite_i64_field(&json!({"overflow_top_px": 12}), "overflow_top_px"),
            Some(12)
        );
        assert_eq!(
            finite_i64_field(&json!({"overflow_top_px": 12.6}), "overflow_top_px"),
            Some(13)
        );
        assert_eq!(
            finite_i64_field(&json!({"overflow_top_px": i64::MAX}), "overflow_top_px"),
            Some(i64::MAX)
        );
    }

    #[test]
    fn out_of_range_and_non_numeric_measurements_are_rejected() {
        assert_eq!(
            finite_i64_field(&json!({"overflow_top_px": 1e30}), "overflow_top_px"),
            None
        );
        assert_eq!(
            finite_i64_field(&json!({"overflow_top_px": -1e30}), "overflow_top_px"),
            None
        );
        assert_eq!(
            finite_i64_field(
                &json!({"overflow_top_px": i64::MAX as f64}),
                "overflow_top_px"
            ),
            None
        );
        assert_eq!(
            finite_i64_field(&json!({"overflow_top_px": "12"}), "overflow_top_px"),
            None
        );
        assert_eq!(finite_i64_field(&json!({}), "overflow_top_px"), None);
    }

    #[test]
    fn invalid_surface_fit_overflow_is_treated_as_missing() {
        let overflows = surface_fit_overflows(&json!({
            "overflow_top_px": 1e30,
            "overflow_left_px": 4.2
        }));
        assert_eq!(overflows["top"], 0);
        assert_eq!(overflows["left"], 4);
    }
}
