const INPUT_SEPARATOR: &str = " --input ";

pub(super) fn parts(command: &str) -> Option<(&str, Option<&str>)> {
    let body = command.trim().strip_prefix(super::CATALOG_CHECK_PREFIX)?;
    let (id, input) = body
        .split_once(INPUT_SEPARATOR)
        .map_or((body, None), |(id, input)| (id, Some(input)));
    if input.is_some_and(str::is_empty) {
        return None;
    }
    Some((id, input))
}

pub(crate) fn catalog_check_command_with_input(id: &str, input: &str) -> String {
    format!(
        "{}{id}{INPUT_SEPARATOR}{input}",
        super::CATALOG_CHECK_PREFIX
    )
}
