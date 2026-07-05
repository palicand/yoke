use crate::error::VolumeError;
use crate::state::{HORI_PS4_VID_PID, QUADSTICK_VID_PIDS, VidPid};
use std::collections::HashSet;

pub const TEST_VIDPIDS_ENV: &str = "YOKE_TEST_VIDPIDS";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClass {
    QuadStick(VidPid),
    HoriPs4,
    Other,
}

/// Classifies USB VID:PIDs into `QuadStick` / Hori / Other.
///
/// The `QuadStick` set is the built-in list optionally extended via `YOKE_TEST_VIDPIDS`,
/// which lets a plain USB stick stand in for a `QuadStick` during VM testing.
#[derive(Clone, Debug)]
pub struct DeviceClassifier {
    quadstick: HashSet<VidPid>,
}

impl DeviceClassifier {
    /// Reads `YOKE_TEST_VIDPIDS` once. A malformed value is a hard error
    /// (refuse-on-ambiguity) rather than a silently ignored override.
    pub fn from_env() -> Result<Self, VolumeError> {
        match std::env::var(TEST_VIDPIDS_ENV) {
            Ok(raw) => {
                let extra = parse_vidpid_list(&raw).map_err(VolumeError::BackendInit)?;
                tracing::warn!(
                    value = %raw,
                    "{TEST_VIDPIDS_ENV} active: extending QuadStick VID:PID set for testing"
                );
                Ok(Self::with_extra(&extra))
            }
            Err(_) => Ok(Self::with_extra(&[])),
        }
    }

    #[must_use]
    pub fn with_extra(extra: &[VidPid]) -> Self {
        let mut quadstick: HashSet<VidPid> = QUADSTICK_VID_PIDS.iter().copied().collect();
        quadstick.extend(extra.iter().copied());
        Self { quadstick }
    }

    #[must_use]
    pub fn classify(&self, vid_pid: VidPid) -> DeviceClass {
        if self.quadstick.contains(&vid_pid) {
            DeviceClass::QuadStick(vid_pid)
        } else if vid_pid == HORI_PS4_VID_PID {
            DeviceClass::HoriPs4
        } else {
            DeviceClass::Other
        }
    }
}

fn parse_vidpid_list(raw: &str) -> Result<Vec<VidPid>, String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            VidPid::parse(s).ok_or_else(|| {
                format!(
                    "invalid VID:PID entry {s:?} in {TEST_VIDPIDS_ENV} (expected hex vvvv:pppp)"
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{HORI_PS4_VID_PID, VidPid};

    const PRIMARY: VidPid = VidPid {
        vendor: 0x16D0,
        product: 0x092B,
    };
    const STICK: VidPid = VidPid {
        vendor: 0x0951,
        product: 0x1666,
    };

    #[test]
    fn classify_builtin_quadstick() {
        let c = DeviceClassifier::with_extra(&[]);
        assert_eq!(c.classify(PRIMARY), DeviceClass::QuadStick(PRIMARY));
    }

    #[test]
    fn classify_hori() {
        let c = DeviceClassifier::with_extra(&[]);
        assert_eq!(c.classify(HORI_PS4_VID_PID), DeviceClass::HoriPs4);
    }

    #[test]
    fn classify_unknown_is_other() {
        let c = DeviceClassifier::with_extra(&[]);
        assert_eq!(c.classify(STICK), DeviceClass::Other);
    }

    #[test]
    fn extra_vidpids_classify_as_quadstick() {
        let c = DeviceClassifier::with_extra(&[STICK]);
        assert_eq!(c.classify(STICK), DeviceClass::QuadStick(STICK));
        assert_eq!(c.classify(PRIMARY), DeviceClass::QuadStick(PRIMARY));
    }

    #[test]
    fn parse_list_accepts_commas_and_whitespace() {
        assert_eq!(
            parse_vidpid_list("0951:1666, 16d0:092b").unwrap(),
            vec![
                STICK,
                VidPid {
                    vendor: 0x16D0,
                    product: 0x092B
                }
            ]
        );
    }

    #[test]
    fn parse_list_rejects_malformed_entries() {
        let err = parse_vidpid_list("0951:1666,bogus").unwrap_err();
        assert!(
            err.contains("bogus"),
            "error should name the bad entry: {err}"
        );
    }
}
