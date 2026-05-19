use strsim::levenshtein;

const MAX_DISTANCE: usize = 2;
const MAX_SUGGESTIONS: usize = 5;

pub fn suggestions<'a, I>(input: &str, candidates: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut scored: Vec<(usize, &str)> = candidates
        .into_iter()
        .map(|c| (levenshtein(input, c), c))
        .filter(|(d, _)| *d <= MAX_DISTANCE)
        .collect();
    scored.sort_by_key(|(d, _)| *d);
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, c)| c.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_closest_first() {
        let s = suggestions("kb_eter", ["kb_enter", "kb_escape", "kb_a"]);
        assert_eq!(s, vec!["kb_enter".to_string()]);
    }

    #[test]
    fn returns_empty_when_nothing_within_distance() {
        let s = suggestions("xyz", ["kb_enter", "kb_a"]);
        assert!(s.is_empty());
    }

    #[test]
    fn caps_at_five_and_preserves_source_order_on_ties() {
        let candidates: Vec<String> = (0..10).map(|i| format!("foo{i}")).collect();
        let refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
        let s = suggestions("foo", refs);
        assert_eq!(
            s,
            vec![
                "foo0".to_string(),
                "foo1".to_string(),
                "foo2".to_string(),
                "foo3".to_string(),
                "foo4".to_string(),
            ]
        );
    }
}
