use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modifier {
    Normal,
    Toggle,
    DelayOn {
        ms: Option<u32>,
    },
    DelayOff {
        ms: Option<u32>,
    },
    GreaterThan {
        pct: Option<u8>,
        upper: Option<u8>,
    },
    LessThan {
        pct: Option<u8>,
    },
    Repeat {
        hz: Option<u32>,
        delay_ms: Option<u32>,
    },
    Pulse {
        ms: Option<u32>,
        count: Option<u32>,
    },
    Duty {
        ms: Option<u32>,
    },
    ForceOff {
        ms: Option<u32>,
    },
    DelayedLatch {
        ms: Option<u32>,
    },
    Tap {
        window_ms: Option<u32>,
        pulse_ms: Option<u32>,
    },
    IncrementValue {
        amount: Option<i32>,
        interval_ms: Option<u32>,
    },
    DecrementValue {
        amount: Option<i32>,
        interval_ms: Option<u32>,
    },
    Unknown {
        name: String,
        args: Vec<String>,
    },
}

impl Modifier {
    /// CSV keywords (the leading token of a modifier phrase) for every typed
    /// modifier, excluding `Unknown`. Keyword-only: the argument grammar is
    /// open, so this is not a closed set of full values.
    pub const KEYWORDS: &'static [&'static str] = &[
        "normal",
        "toggle",
        "delay_on",
        "delay_off",
        "greater_than",
        "less_than",
        "repeat",
        "pulse",
        "duty",
        "force_off",
        "delayed_latch",
        "tap",
        "increment_value",
        "decrement_value",
    ];

    /// The CSV keyword (leading token) for a typed modifier, or `None` for `Unknown`.
    /// Exhaustive by construction: a new `Modifier` variant cannot compile without an arm
    /// here, which is the single point that forces it to declare a keyword — and the
    /// `keyword_is_consistent_with_from_csv_for_every_listed_keyword` test ties that token
    /// back to [`Self::KEYWORDS`] and `from_csv`.
    #[must_use]
    pub const fn keyword(&self) -> Option<&'static str> {
        Some(match self {
            Self::Normal => "normal",
            Self::Toggle => "toggle",
            Self::DelayOn { .. } => "delay_on",
            Self::DelayOff { .. } => "delay_off",
            Self::GreaterThan { .. } => "greater_than",
            Self::LessThan { .. } => "less_than",
            Self::Repeat { .. } => "repeat",
            Self::Pulse { .. } => "pulse",
            Self::Duty { .. } => "duty",
            Self::ForceOff { .. } => "force_off",
            Self::DelayedLatch { .. } => "delayed_latch",
            Self::Tap { .. } => "tap",
            Self::IncrementValue { .. } => "increment_value",
            Self::DecrementValue { .. } => "decrement_value",
            Self::Unknown { .. } => return None,
        })
    }

    pub fn from_csv(s: &str) -> Option<Self> {
        fn unknown(name: &str, args: &[&str]) -> Modifier {
            Modifier::Unknown {
                name: name.to_owned(),
                args: args.iter().map(|s| (*s).to_owned()).collect(),
            }
        }

        // Returns Ok(None) if absent, Ok(Some(v)) if parsed, Err if present but malformed.
        fn parse_arg<T: std::str::FromStr>(args: &[&str], i: usize) -> Result<Option<T>, ()> {
            args.get(i)
                .map_or(Ok(None), |s| s.parse::<T>().map(Some).map_err(|_| ()))
        }

        let mut tokens = s.split_whitespace();
        let name = tokens.next()?;
        let args: Vec<&str> = tokens.collect();

        // Each typed arm guards on arity, then propagates `unknown(name, &args)` if any
        // declared arg fails to parse. This preserves original tokens on round-trip.
        Some(match name {
            "normal" if args.is_empty() => Self::Normal,
            "toggle" if args.is_empty() => Self::Toggle,
            "delay_on" if args.len() <= 1 => match parse_arg::<u32>(&args, 0) {
                Ok(ms) => Self::DelayOn { ms },
                Err(()) => unknown(name, &args),
            },
            "delay_off" if args.len() <= 1 => match parse_arg::<u32>(&args, 0) {
                Ok(ms) => Self::DelayOff { ms },
                Err(()) => unknown(name, &args),
            },
            "greater_than" if args.len() <= 2 => {
                match (parse_arg::<u8>(&args, 0), parse_arg::<u8>(&args, 1)) {
                    (Ok(pct), Ok(upper)) => Self::GreaterThan { pct, upper },
                    _ => unknown(name, &args),
                }
            }
            "less_than" if args.len() <= 1 => match parse_arg::<u8>(&args, 0) {
                Ok(pct) => Self::LessThan { pct },
                Err(()) => unknown(name, &args),
            },
            "repeat" if args.len() <= 2 => {
                match (parse_arg::<u32>(&args, 0), parse_arg::<u32>(&args, 1)) {
                    (Ok(hz), Ok(delay_ms)) => Self::Repeat { hz, delay_ms },
                    _ => unknown(name, &args),
                }
            }
            "pulse" if args.len() <= 2 => {
                match (parse_arg::<u32>(&args, 0), parse_arg::<u32>(&args, 1)) {
                    (Ok(ms), Ok(count)) => Self::Pulse { ms, count },
                    _ => unknown(name, &args),
                }
            }
            "duty" if args.len() <= 1 => match parse_arg::<u32>(&args, 0) {
                Ok(ms) => Self::Duty { ms },
                Err(()) => unknown(name, &args),
            },
            "force_off" if args.len() <= 1 => match parse_arg::<u32>(&args, 0) {
                Ok(ms) => Self::ForceOff { ms },
                Err(()) => unknown(name, &args),
            },
            "delayed_latch" if args.len() <= 1 => match parse_arg::<u32>(&args, 0) {
                Ok(ms) => Self::DelayedLatch { ms },
                Err(()) => unknown(name, &args),
            },
            "tap" if args.len() <= 2 => {
                match (parse_arg::<u32>(&args, 0), parse_arg::<u32>(&args, 1)) {
                    (Ok(window_ms), Ok(pulse_ms)) => Self::Tap {
                        window_ms,
                        pulse_ms,
                    },
                    _ => unknown(name, &args),
                }
            }
            "increment_value" if args.len() <= 2 => {
                match (parse_arg::<i32>(&args, 0), parse_arg::<u32>(&args, 1)) {
                    (Ok(amount), Ok(interval_ms)) => Self::IncrementValue {
                        amount,
                        interval_ms,
                    },
                    _ => unknown(name, &args),
                }
            }
            "decrement_value" if args.len() <= 2 => {
                match (parse_arg::<i32>(&args, 0), parse_arg::<u32>(&args, 1)) {
                    (Ok(amount), Ok(interval_ms)) => Self::DecrementValue {
                        amount,
                        interval_ms,
                    },
                    _ => unknown(name, &args),
                }
            }
            _ => unknown(name, &args),
        })
    }

    pub fn to_csv(&self) -> String {
        fn render(name: &str, args: &[Option<String>]) -> String {
            let mut out = name.to_owned();
            let last_some = args.iter().rposition(Option::is_some);
            if let Some(last) = last_some {
                for arg in &args[..=last] {
                    out.push(' ');
                    out.push_str(arg.as_deref().unwrap_or(""));
                }
            }
            out
        }
        let opt_str = |o: &Option<u32>| o.map(|v| v.to_string());
        let opt_i = |o: &Option<i32>| o.map(|v| v.to_string());
        let opt_u8s = |o: &Option<u8>| o.map(|v| v.to_string());

        match self {
            Self::Normal => "normal".into(),
            Self::Toggle => "toggle".into(),
            Self::DelayOn { ms } => render("delay_on", &[opt_str(ms)]),
            Self::DelayOff { ms } => render("delay_off", &[opt_str(ms)]),
            Self::GreaterThan { pct, upper } => {
                render("greater_than", &[opt_u8s(pct), opt_u8s(upper)])
            }
            Self::LessThan { pct } => render("less_than", &[opt_u8s(pct)]),
            Self::Repeat { hz, delay_ms } => render("repeat", &[opt_str(hz), opt_str(delay_ms)]),
            Self::Pulse { ms, count } => render("pulse", &[opt_str(ms), opt_str(count)]),
            Self::Duty { ms } => render("duty", &[opt_str(ms)]),
            Self::ForceOff { ms } => render("force_off", &[opt_str(ms)]),
            Self::DelayedLatch { ms } => render("delayed_latch", &[opt_str(ms)]),
            Self::Tap {
                window_ms,
                pulse_ms,
            } => render("tap", &[opt_str(window_ms), opt_str(pulse_ms)]),
            Self::IncrementValue {
                amount,
                interval_ms,
            } => render("increment_value", &[opt_i(amount), opt_str(interval_ms)]),
            Self::DecrementValue {
                amount,
                interval_ms,
            } => render("decrement_value", &[opt_i(amount), opt_str(interval_ms)]),
            Self::Unknown { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let mut out = name.clone();
                    for a in args {
                        out.push(' ');
                        out.push_str(a);
                    }
                    out
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_round_trips() {
        let m = Modifier::from_csv("normal").unwrap();
        assert_eq!(m, Modifier::Normal);
        assert_eq!(m.to_csv(), "normal");
    }

    #[test]
    fn toggle_round_trips() {
        assert_eq!(Modifier::from_csv("toggle"), Some(Modifier::Toggle));
    }

    #[test]
    fn delay_on_with_arg() {
        let m = Modifier::from_csv("delay_on 1000").unwrap();
        assert_eq!(m, Modifier::DelayOn { ms: Some(1000) });
        assert_eq!(m.to_csv(), "delay_on 1000");
    }

    #[test]
    fn delay_on_without_arg() {
        let m = Modifier::from_csv("delay_on").unwrap();
        assert_eq!(m, Modifier::DelayOn { ms: None });
        assert_eq!(m.to_csv(), "delay_on");
    }

    #[test]
    fn greater_than_with_upper_bound() {
        let m = Modifier::from_csv("greater_than 30 70").unwrap();
        assert_eq!(
            m,
            Modifier::GreaterThan {
                pct: Some(30),
                upper: Some(70)
            }
        );
        assert_eq!(m.to_csv(), "greater_than 30 70");
    }

    #[test]
    fn unknown_modifier_round_trips() {
        let m = Modifier::from_csv("future_modifier 42 7").unwrap();
        assert_eq!(
            m,
            Modifier::Unknown {
                name: "future_modifier".into(),
                args: vec!["42".into(), "7".into()],
            }
        );
        assert_eq!(m.to_csv(), "future_modifier 42 7");
    }

    #[test]
    fn delay_on_with_garbage_arg_round_trips() {
        let m = Modifier::from_csv("delay_on abc").unwrap();
        assert_eq!(
            m,
            Modifier::Unknown {
                name: "delay_on".into(),
                args: vec!["abc".into()],
            }
        );
        assert_eq!(m.to_csv(), "delay_on abc");
    }

    #[test]
    fn delay_on_with_extra_arg_round_trips() {
        let m = Modifier::from_csv("delay_on 1000 extra").unwrap();
        assert_eq!(
            m,
            Modifier::Unknown {
                name: "delay_on".into(),
                args: vec!["1000".into(), "extra".into()],
            }
        );
        assert_eq!(m.to_csv(), "delay_on 1000 extra");
    }

    #[test]
    fn normal_rejects_extra_args() {
        let m = Modifier::from_csv("normal junk").unwrap();
        assert_eq!(
            m,
            Modifier::Unknown {
                name: "normal".into(),
                args: vec!["junk".into()],
            }
        );
        assert_eq!(m.to_csv(), "normal junk");
    }

    #[test]
    fn greater_than_with_garbage_pct_round_trips() {
        let m = Modifier::from_csv("greater_than 999 70").unwrap();
        // 999 doesn't fit in u8, so falls through to Unknown.
        assert_eq!(
            m,
            Modifier::Unknown {
                name: "greater_than".into(),
                args: vec!["999".into(), "70".into()],
            }
        );
        assert_eq!(m.to_csv(), "greater_than 999 70");
    }

    // source: https://quadstick.s3.amazonaws.com/documents/user_manual/um/dropdown_list_for_ouput_functions.htm
    #[test]
    fn every_documented_modifier_parses() {
        for name in [
            "normal",
            "toggle",
            "repeat",
            "pulse",
            "duty",
            "greater_than",
            "less_than",
            "force_off",
            "delayed_latch",
            "delay_off",
            "delay_on",
            "tap",
            "increment_value",
            "decrement_value",
        ] {
            let m = Modifier::from_csv(name).unwrap_or_else(|| panic!("could not parse: {name}"));
            assert!(
                !matches!(m, Modifier::Unknown { .. }),
                "{name} fell through to Unknown"
            );
        }
    }

    #[test]
    fn keywords_lists_every_typed_modifier() {
        // Every keyword must parse to a typed (non-Unknown) variant, and the
        // list must cover all 14 documented modifiers.
        // 14 = the documented modifier set (source URL in every_documented_modifier_parses).
        assert_eq!(Modifier::KEYWORDS.len(), 14);
        for kw in Modifier::KEYWORDS {
            let m = Modifier::from_csv(kw).unwrap_or_else(|| panic!("could not parse {kw}"));
            assert!(
                !matches!(m, Modifier::Unknown { .. }),
                "{kw} fell through to Unknown"
            );
        }
    }

    #[test]
    fn keyword_is_consistent_with_from_csv_for_every_listed_keyword() {
        // `keyword()` is the single per-variant source of the leading token. Every entry
        // in KEYWORDS must parse to a typed variant whose `keyword()` returns that same
        // token, so KEYWORDS / from_csv / keyword() can never silently disagree.
        for kw in Modifier::KEYWORDS {
            let m = Modifier::from_csv(kw).unwrap_or_else(|| panic!("could not parse {kw}"));
            assert_eq!(m.keyword(), Some(*kw), "keyword() disagrees for {kw}");
        }
    }

    #[test]
    fn unknown_modifier_has_no_keyword() {
        let m = Modifier::from_csv("future_modifier 1").unwrap();
        assert!(matches!(m, Modifier::Unknown { .. }));
        assert_eq!(m.keyword(), None);
    }
}
