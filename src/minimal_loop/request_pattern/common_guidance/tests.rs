use std::ffi::OsStr;
use std::path::Path;

use super::{COMMON_HEADING, MAX_BYTES, SYSTEM_GUIDANCE_ENV, compose, parse};
use crate::minimal_loop::prompt::{
    PromptGuidance, ToolPromptMode, build_guided_request_messages, build_request_messages,
};
use crate::minimal_loop::request_pattern::RequestPattern;
use crate::state::ConversationMessage;
use crate::tools::registry::ToolRegistry;

const REQUEST: &str = "Please tidy the greeting module.";
const PROFILE: &str = "Profile block for the test.";
const COMMON: &str = "Keep each reply short.\nName the files you changed.";
const MARKER: &str = "marker-925-must-not-be-echoed";

fn messages(guidance: PromptGuidance<'_>, mode: ToolPromptMode) -> Vec<ConversationMessage> {
    build_guided_request_messages(
        &[ConversationMessage::user(REQUEST.to_string())],
        ToolRegistry::default().specs(),
        Path::new("/tmp/work"),
        Some("feedback kept"),
        guidance,
        mode,
    )
}

fn checked(value: &str) -> Option<String> {
    parse(Some(OsStr::new(value))).unwrap()
}

fn refusal(value: &OsStr) -> String {
    format!("{:#}", parse(Some(value)).unwrap_err())
}

#[test]
fn unset_and_empty_values_build_the_unchanged_request() {
    assert_eq!(parse(None).unwrap(), None);
    assert_eq!(checked(""), None);
    for mode in [ToolPromptMode::Native, ToolPromptMode::XmlFallback] {
        for profile in [None, Some(PROFILE)] {
            let plain = build_request_messages(
                &[ConversationMessage::user(REQUEST.to_string())],
                ToolRegistry::default().specs(),
                Path::new("/tmp/work"),
                Some("feedback kept"),
                profile,
                mode,
            );
            for value in [parse(None).unwrap(), checked("")] {
                let request = compose(None, value.as_deref());
                assert_eq!(request, None);
                let guided = messages(
                    PromptGuidance {
                        profile,
                        request: request.as_deref(),
                    },
                    mode,
                );
                assert_eq!(guided, plain);
            }
        }
    }
    for pattern in RequestPattern::ALL {
        assert_eq!(
            compose(Some(pattern.guidance()), checked("").as_deref()).as_deref(),
            Some(pattern.guidance())
        );
    }
}

#[test]
fn a_valid_value_is_kept_verbatim() {
    let value = "  First line.\n\tSecond line.  \n";
    assert_eq!(checked(value).as_deref(), Some(value));
    assert_eq!(checked(COMMON).as_deref(), Some(COMMON));
}

#[test]
fn the_size_limit_counts_utf8_bytes() {
    assert_eq!(MAX_BYTES, 8192);
    let ascii = "a".repeat(MAX_BYTES);
    assert_eq!(checked(&ascii).as_deref(), Some(ascii.as_str()));
    let over = "a".repeat(MAX_BYTES + 1);
    assert!(parse(Some(OsStr::new(&over))).is_err());

    // "あ" is three bytes: 2730 of them plus two ASCII bytes is exactly 8 KiB.
    let multibyte = format!("{}ab", "あ".repeat(2730));
    assert_eq!(multibyte.len(), MAX_BYTES);
    assert_eq!(checked(&multibyte).as_deref(), Some(multibyte.as_str()));
    for over in [format!("{}abc", "あ".repeat(2730)), "あ".repeat(2731)] {
        assert_eq!(over.len(), MAX_BYTES + 1);
        assert!(over.chars().count() < MAX_BYTES);
        assert_eq!(
            refusal(OsStr::new(&over)),
            format!("{SYSTEM_GUIDANCE_ENV} is longer than 8 KiB")
        );
    }
}

#[test]
fn whitespace_only_values_are_refused() {
    for value in [" ", "\n", " \t\r\n ", "\u{3000}"] {
        assert_eq!(
            refusal(OsStr::new(value)),
            format!("{SYSTEM_GUIDANCE_ENV} contains only whitespace")
        );
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_values_are_refused_without_the_value() {
    use std::os::unix::ffi::OsStrExt;

    let mut bytes = MARKER.as_bytes().to_vec();
    bytes.push(0xff);
    let message = refusal(OsStr::from_bytes(&bytes));
    assert_eq!(message, format!("{SYSTEM_GUIDANCE_ENV} is not valid UTF-8"));
    assert!(!message.contains(MARKER));
}

#[test]
fn refusals_name_the_variable_and_never_the_value() {
    let long = format!("{MARKER} ").repeat(MAX_BYTES / MARKER.len());
    assert!(long.len() > MAX_BYTES);
    let message = refusal(OsStr::new(&long));
    assert!(message.starts_with(SYSTEM_GUIDANCE_ENV), "{message}");
    assert!(!message.contains(MARKER), "{message}");
    assert!(message.len() < 64, "{message}");
}

#[test]
fn common_guidance_alone_is_the_request_block() {
    let request = compose(None, checked(COMMON).as_deref()).unwrap();
    assert_eq!(request, COMMON);
    for mode in [ToolPromptMode::Native, ToolPromptMode::XmlFallback] {
        let sent = messages(
            PromptGuidance {
                profile: Some(PROFILE),
                request: Some(&request),
            },
            mode,
        );
        let base = messages(
            PromptGuidance {
                profile: Some(PROFILE),
                request: None,
            },
            mode,
        );
        let system = &sent[0].content;
        assert_eq!(system.matches(COMMON).count(), 1);
        assert_eq!(system.matches("Request guidance:").count(), 1);
        assert!(!system.contains(COMMON_HEADING.trim()));
        let block = format!("\n\nRequest guidance:\n{COMMON}");
        assert_eq!(system.replacen(&block, "", 1), base[0].content);
        assert!(system.find("Profile guidance:").unwrap() < system.find(COMMON).unwrap());
        if mode == ToolPromptMode::XmlFallback {
            assert!(
                system.find(COMMON).unwrap()
                    < system.find("Native tool calls are unavailable").unwrap()
            );
        }
        assert_eq!(sent[1..], base[1..]);
        assert_eq!(sent[1].content, REQUEST);
    }
}

#[test]
fn an_adopted_pattern_keeps_its_block_and_the_common_guidance_follows_once() {
    for pattern in RequestPattern::ALL {
        let request = compose(Some(pattern.guidance()), checked(COMMON).as_deref()).unwrap();
        assert_eq!(
            request,
            format!("{}\n\nCommon guidance:\n{COMMON}", pattern.guidance())
        );
        for mode in [ToolPromptMode::Native, ToolPromptMode::XmlFallback] {
            let sent = messages(
                PromptGuidance {
                    profile: Some(PROFILE),
                    request: Some(&request),
                },
                mode,
            );
            let pattern_only = messages(
                PromptGuidance {
                    profile: Some(PROFILE),
                    request: Some(pattern.guidance()),
                },
                mode,
            );
            let system = &sent[0].content;
            assert_eq!(system.matches(pattern.guidance()).count(), 1);
            assert_eq!(system.matches(COMMON).count(), 1);
            assert_eq!(system.matches("Request guidance:").count(), 1);
            assert_eq!(system.matches("Common guidance:").count(), 1);
            assert!(
                system.find(pattern.guidance()).unwrap() < system.find("Common guidance:").unwrap()
            );
            let common_block = format!("{COMMON_HEADING}{COMMON}");
            assert_eq!(
                system.replacen(&common_block, "", 1),
                pattern_only[0].content
            );
            assert_eq!(sent[1..], pattern_only[1..]);
        }
    }
}
