//! Phrase registry: every clip key a voice pack can provide, with the prompt the
//! speaker reads and the completeness tier it counts toward.

use std::sync::OnceLock;

use serde::Serialize;

const REGISTRY: &str = include_str!("phrases.txt");

/// Optional pack clip that replaces the built-in radio chirp. Not a spoken phrase,
/// so it never counts toward tier completeness.
pub const RADIO_BEEP_KEY: &str = "radio_beep";

/// Completeness tier. A Spotter-complete pack covers flags and traffic; Engineer
/// adds the numbers and glue words behind lap times, gaps, and fuel counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    Spotter,
    Engineer,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Phrase {
    pub key: &'static str,
    pub prompt: &'static str,
    pub tier: Tier,
    pub category: &'static str,
}

/// All phrases in registry order (the order the recorder walks them).
pub fn phrases() -> &'static [Phrase] {
    static PHRASES: OnceLock<Vec<Phrase>> = OnceLock::new();
    PHRASES.get_or_init(|| parse(REGISTRY))
}

pub fn phrase(key: &str) -> Option<&'static Phrase> {
    phrases().iter().find(|p| p.key == key)
}

/// A key a pack may hold: a registry phrase or the radio beep override.
pub fn is_pack_key(key: &str) -> bool {
    key == RADIO_BEEP_KEY || phrase(key).is_some()
}

fn parse(raw: &'static str) -> Vec<Phrase> {
    let mut out = Vec::new();
    let mut section: Option<(Tier, &'static str)> = None;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let (tier, category) = header.split_once(' ').unwrap_or((header, ""));
            let tier = match tier {
                "spotter" => Tier::Spotter,
                "engineer" => Tier::Engineer,
                other => panic!("phrases.txt: unknown tier '{other}'"),
            };
            section = Some((tier, category.trim()));
            continue;
        }
        let (Some((tier, category)), Some((key, prompt))) = (section, line.split_once('=')) else {
            panic!("phrases.txt: line outside a section or without '=': {line}");
        };
        out.push(Phrase {
            key: key.trim(),
            prompt: prompt.trim(),
            tier,
            category,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn keys_are_unique_and_prompts_present() {
        let mut seen = HashSet::new();
        for p in phrases() {
            assert!(seen.insert(p.key), "duplicate key {}", p.key);
            assert!(!p.prompt.is_empty(), "{} has no prompt", p.key);
            assert!(!p.category.is_empty(), "{} has no category", p.key);
            assert!(
                p.key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{} is not a valid file stem",
                p.key
            );
        }
    }

    #[test]
    fn numbers_are_zero_to_twenty_plus_tens() {
        let numbers: Vec<_> = phrases()
            .iter()
            .filter(|p| p.category == "numbers")
            .map(|p| p.key)
            .collect();
        let mut expected: Vec<String> = (0..=20).map(|n| format!("n{n}")).collect();
        expected.extend((30..=90).step_by(10).map(|n| format!("n{n}")));
        assert_eq!(numbers, expected);
        assert!(numbers
            .iter()
            .all(|k| phrase(k).unwrap().tier == Tier::Engineer));
    }

    #[test]
    fn radio_beep_is_a_pack_key_but_not_a_phrase() {
        assert!(phrase(RADIO_BEEP_KEY).is_none());
        assert!(is_pack_key(RADIO_BEEP_KEY));
        assert!(is_pack_key("flag_green"));
        assert!(!is_pack_key("n29"));
    }

    #[test]
    fn spotter_lines_need_no_numbers() {
        let spotter = phrases().iter().filter(|p| p.tier == Tier::Spotter).count();
        assert!((30..=60).contains(&spotter), "{spotter} spotter phrases");
    }
}
