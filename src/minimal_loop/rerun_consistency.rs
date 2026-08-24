pub(crate) fn reproduced<T: PartialEq>(baseline: &T, rerun: &T) -> bool {
    baseline == rerun
}
