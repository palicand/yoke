use yoke_volume::state::VidPid;

/// Extract VID:PID from a `PnP` hardware or device-instance ID such as
/// `USB\VID_16D0&PID_092B&REV_0100` or `USB\VID_0951&PID_1666\<serial>`.
///
/// Requires exactly 4 hex digits after each marker, per the `PnP` ID format.
#[must_use]
pub fn vid_pid_from_pnp_id(id: &str) -> Option<VidPid> {
    let upper = id.to_ascii_uppercase();
    let vendor = hex4_after(&upper, "VID_")?;
    let product = hex4_after(&upper, "PID_")?;
    Some(VidPid { vendor, product })
}

fn hex4_after(haystack: &str, marker: &str) -> Option<u16> {
    let at = haystack.find(marker)? + marker.len();
    let digits = haystack.get(at..at + 4)?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u16::from_str_radix(digits, 16).ok()
}

/// Decode UTF-16 up to the first NUL (or the whole slice if none).
#[must_use]
pub fn utf16_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Split a REG_MULTI_SZ-style double-NUL-terminated UTF-16 list.
#[must_use]
pub fn split_multi_sz(buf: &[u16]) -> Vec<String> {
    buf.split(|&c| c == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// NUL-terminated UTF-16 for passing as PCWSTR.
#[must_use]
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_volume::state::VidPid;

    #[test]
    fn parses_usb_hardware_id() {
        assert_eq!(
            vid_pid_from_pnp_id(r"USB\VID_16D0&PID_092B&REV_0100"),
            Some(VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            })
        );
    }

    #[test]
    fn parses_instance_id_with_serial_suffix() {
        assert_eq!(
            vid_pid_from_pnp_id(r"USB\VID_0951&PID_1666\0019E06B9C85F961976B0A5C"),
            Some(VidPid {
                vendor: 0x0951,
                product: 0x1666,
            })
        );
    }

    #[test]
    fn parse_is_case_insensitive() {
        assert_eq!(
            vid_pid_from_pnp_id(r"usb\vid_16d0&pid_092b"),
            Some(VidPid {
                vendor: 0x16D0,
                product: 0x092B,
            })
        );
    }

    #[test]
    fn rejects_ids_without_vid_pid() {
        assert_eq!(vid_pid_from_pnp_id(r"SWD\PRINTENUM\PrintQueues"), None);
        assert_eq!(vid_pid_from_pnp_id(r"USB\VID_16D0"), None);
        assert_eq!(vid_pid_from_pnp_id(r"USB\VID_16D0&PID_09"), None);
        assert_eq!(vid_pid_from_pnp_id(""), None);
    }

    #[test]
    fn utf16_stops_at_nul() {
        let buf: Vec<u16> = "E:\\\0garbage".encode_utf16().collect();
        assert_eq!(utf16_to_string(&buf), "E:\\");
    }

    #[test]
    fn utf16_without_nul_takes_all() {
        let buf: Vec<u16> = "QUADSTICK".encode_utf16().collect();
        assert_eq!(utf16_to_string(&buf), "QUADSTICK");
    }

    #[test]
    fn multi_sz_splits_entries() {
        let buf: Vec<u16> = "abc\0de\0\0".encode_utf16().collect();
        assert_eq!(
            split_multi_sz(&buf),
            vec!["abc".to_string(), "de".to_string()]
        );
    }

    #[test]
    fn multi_sz_empty_list() {
        let buf: Vec<u16> = "\0\0".encode_utf16().collect();
        assert!(split_multi_sz(&buf).is_empty());
        assert!(split_multi_sz(&[]).is_empty());
    }

    #[test]
    fn to_wide_appends_nul() {
        let w = to_wide("E:");
        assert_eq!(w, vec![b'E'.into(), b':'.into(), 0u16]);
    }
}
