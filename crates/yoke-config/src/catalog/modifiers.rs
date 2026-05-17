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
    pub fn from_csv(s: &str) -> Option<Self> {
        fn parse_opt<T: std::str::FromStr>(a: Option<&str>) -> Option<T> {
            a.and_then(|s| s.parse().ok())
        }

        let mut tokens = s.split_whitespace();
        let name = tokens.next()?;

        Some(match name {
            "normal" => Self::Normal,
            "toggle" => Self::Toggle,
            "delay_on" => Self::DelayOn {
                ms: parse_opt(tokens.next()),
            },
            "delay_off" => Self::DelayOff {
                ms: parse_opt(tokens.next()),
            },
            "greater_than" => Self::GreaterThan {
                pct: parse_opt(tokens.next()),
                upper: parse_opt(tokens.next()),
            },
            "less_than" => Self::LessThan {
                pct: parse_opt(tokens.next()),
            },
            "repeat" => Self::Repeat {
                hz: parse_opt(tokens.next()),
                delay_ms: parse_opt(tokens.next()),
            },
            "pulse" => Self::Pulse {
                ms: parse_opt(tokens.next()),
                count: parse_opt(tokens.next()),
            },
            "duty" => Self::Duty {
                ms: parse_opt(tokens.next()),
            },
            "force_off" => Self::ForceOff {
                ms: parse_opt(tokens.next()),
            },
            "delayed_latch" => Self::DelayedLatch {
                ms: parse_opt(tokens.next()),
            },
            "tap" => Self::Tap {
                window_ms: parse_opt(tokens.next()),
                pulse_ms: parse_opt(tokens.next()),
            },
            "increment_value" => Self::IncrementValue {
                amount: parse_opt(tokens.next()),
                interval_ms: parse_opt(tokens.next()),
            },
            "decrement_value" => Self::DecrementValue {
                amount: parse_opt(tokens.next()),
                interval_ms: parse_opt(tokens.next()),
            },
            _ => Self::Unknown {
                name: name.to_owned(),
                args: tokens.map(str::to_owned).collect(),
            },
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
}
