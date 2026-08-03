use anyhow::{Context, bail};

const PRODUCT_TOKEN: &str = "CommandAgentFetch";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsDecision {
    pub allowed: bool,
    pub rule_group: String,
    pub crawl_delay_ms: u64,
    pub matched_rule: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    allow: bool,
    path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Group {
    agents: Vec<String>,
    rules: Vec<Rule>,
    crawl_delay_ms: Option<u64>,
}

pub fn decide(status: u16, body: &[u8], path: &str) -> anyhow::Result<RobotsDecision> {
    match status {
        404 | 410 => {
            return Ok(RobotsDecision {
                allowed: true,
                rule_group: "no_published_rules".to_string(),
                crawl_delay_ms: 0,
                matched_rule: None,
            });
        }
        401 | 403 => bail!("robots_denied:http_status={status}"),
        200 => {}
        _ => bail!("robots_unavailable:http_status={status}"),
    }
    let text = std::str::from_utf8(body).context("robots_parse_error:non_utf8")?;
    let groups = parse_groups(text)?;
    let product_agent = PRODUCT_TOKEN.to_ascii_lowercase();
    let exact = groups
        .iter()
        .filter(|group| {
            group
                .agents
                .iter()
                .any(|agent| agent.eq_ignore_ascii_case(&product_agent))
        })
        .collect::<Vec<_>>();
    let selected = if exact.is_empty() {
        groups
            .iter()
            .filter(|group| group.agents.iter().any(|agent| agent == "*"))
            .collect::<Vec<_>>()
    } else {
        exact
    };
    if selected.is_empty() {
        return Ok(RobotsDecision {
            allowed: true,
            rule_group: "unmatched".to_string(),
            crawl_delay_ms: 0,
            matched_rule: None,
        });
    }
    let mut best: Option<&Rule> = None;
    for rule in selected.iter().flat_map(|group| &group.rules) {
        if rule.path.is_empty() || !path.starts_with(&rule.path) {
            continue;
        }
        let replace = best.is_none_or(|current| {
            rule.path.len() > current.path.len()
                || (rule.path.len() == current.path.len() && rule.allow && !current.allow)
        });
        if replace {
            best = Some(rule);
        }
    }
    let crawl_delay_ms = selected
        .iter()
        .filter_map(|group| group.crawl_delay_ms)
        .max()
        .unwrap_or(0);
    Ok(RobotsDecision {
        allowed: best.is_none_or(|rule| rule.allow),
        rule_group: if selected.iter().any(|group| {
            group
                .agents
                .iter()
                .any(|agent| agent.eq_ignore_ascii_case(&product_agent))
        }) {
            PRODUCT_TOKEN.to_string()
        } else {
            "*".to_string()
        },
        crawl_delay_ms,
        matched_rule: best.map(|rule| {
            format!(
                "{}:{}",
                if rule.allow { "allow" } else { "disallow" },
                rule.path
            )
        }),
    })
}

fn parse_groups(text: &str) -> anyhow::Result<Vec<Group>> {
    let mut groups = Vec::new();
    let mut current = Group::default();
    let mut saw_directive = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            if !current.agents.is_empty() {
                groups.push(std::mem::take(&mut current));
                saw_directive = false;
            }
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .context("robots_parse_error:directive_missing_colon")?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        match name.as_str() {
            "user-agent" => {
                if value.is_empty() {
                    bail!("robots_parse_error:empty_user_agent");
                }
                if saw_directive && !current.agents.is_empty() {
                    groups.push(std::mem::take(&mut current));
                    saw_directive = false;
                }
                current.agents.push(value.to_ascii_lowercase());
            }
            "allow" | "disallow" => {
                if current.agents.is_empty() {
                    bail!("robots_parse_error:rule_before_user_agent");
                }
                saw_directive = true;
                if !value.is_empty() {
                    if !value.starts_with('/') {
                        bail!("robots_parse_error:rule_path_not_absolute");
                    }
                    current.rules.push(Rule {
                        allow: name == "allow",
                        path: value.to_string(),
                    });
                }
            }
            "crawl-delay" => {
                if current.agents.is_empty() || value.is_empty() {
                    bail!("robots_parse_error:invalid_crawl_delay");
                }
                saw_directive = true;
                let seconds = value
                    .parse::<u64>()
                    .context("robots_parse_error:invalid_crawl_delay")?;
                current.crawl_delay_ms = Some(
                    seconds
                        .checked_mul(1_000)
                        .context("robots_parse_error:crawl_delay_overflow")?,
                );
            }
            _ => {}
        }
    }
    if !current.agents.is_empty() {
        groups.push(current);
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_group_wins_and_longest_allow_breaks_a_tie() {
        let body = format!(
            "User-agent: *\nDisallow: /\n\nUser-agent: {PRODUCT_TOKEN}\nDisallow: /events\nAllow: /events/public\nCrawl-delay: 2\n"
        );
        let public = decide(200, body.as_bytes(), "/events/public/a").unwrap();
        assert!(public.allowed);
        assert_eq!(public.crawl_delay_ms, 2_000);
        let private = decide(200, body.as_bytes(), "/events/private").unwrap();
        assert!(!private.allowed);
    }

    #[test]
    fn fixed_status_matrix_fails_closed() {
        assert!(decide(404, b"", "/").unwrap().allowed);
        assert!(decide(410, b"", "/").unwrap().allowed);
        assert!(decide(401, b"", "/").is_err());
        assert!(decide(403, b"", "/").is_err());
        assert!(decide(500, b"", "/").is_err());
        assert!(decide(200, b"bad line", "/").is_err());
    }
}
