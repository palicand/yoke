use crate::error::VolumeError;
use crate::profile::{ProfileEntry, ProfileName};
use rand::RngExt;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime};

const STALE_TMP_THRESHOLD: Duration = Duration::from_mins(1);

pub fn list_profiles(root: &Path) -> Result<Vec<ProfileEntry>, VolumeError> {
    let now = SystemTime::now();
    let mut entries = Vec::new();
    for dent in fs::read_dir(root)? {
        let dent = dent?;
        let fname = dent.file_name();
        let fname_str = fname.to_string_lossy();
        if fname_str.starts_with('.') {
            continue;
        }
        let meta = dent.metadata()?;
        if !meta.is_file() {
            continue;
        }
        // Accept any `.csv` file as a profile first. Genuine write_profile
        // temp files end in `.tmp.<hex>`, not `.csv`, so this can't swallow
        // a stale temp — and a contrived `foo.csv.tmp.bar.csv` should be
        // kept rather than swept.
        if fname_str.to_ascii_lowercase().ends_with(".csv") {
            let name = ProfileName::new(&fname_str)?;
            let kind = name.kind();
            entries.push(ProfileEntry {
                name,
                kind,
                byte_len: meta.len(),
                modified: meta.modified().unwrap_or(now),
            });
            continue;
        }
        if fname_str.contains(".csv.tmp.") {
            let modified = meta.modified().unwrap_or(now);
            let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
            if age >= STALE_TMP_THRESHOLD {
                tracing::warn!(file = %fname_str, "sweeping stale .tmp file");
                let _ = fs::remove_file(dent.path());
            }
        }
    }
    entries.sort_by(|a, b| a.name.as_filename().cmp(b.name.as_filename()));
    Ok(entries)
}

pub fn read_profile(root: &Path, name: &ProfileName) -> Result<Vec<u8>, VolumeError> {
    Ok(fs::read(root.join(name.as_filename()))?)
}

pub fn write_profile(root: &Path, name: &ProfileName, bytes: &[u8]) -> Result<(), VolumeError> {
    let final_path = root.join(name.as_filename());
    let suffix: u32 = rand::rng().random();
    let tmp_path = root.join(format!("{}.tmp.{:08x}", name.as_filename(), suffix));
    let result: Result<(), std::io::Error> = (|| {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    if let Err(e) = fs::rename(&tmp_path, &final_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}

pub fn delete_profile(root: &Path, name: &ProfileName) -> Result<(), VolumeError> {
    Ok(fs::remove_file(root.join(name.as_filename()))?)
}

pub fn rename_profile(
    root: &Path,
    from: &ProfileName,
    to: &ProfileName,
) -> Result<(), VolumeError> {
    Ok(fs::rename(
        root.join(from.as_filename()),
        root.join(to.as_filename()),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ProfileKind;
    use tempfile::tempdir;

    fn pname(s: &str) -> ProfileName {
        ProfileName::new(s).unwrap()
    }

    #[test]
    fn write_then_read_returns_same_bytes() {
        let dir = tempdir().unwrap();
        let bytes = b"hello,csv\n,,\n";
        write_profile(dir.path(), &pname("test"), bytes).unwrap();
        let read = read_profile(dir.path(), &pname("test")).unwrap();
        assert_eq!(&read[..], bytes);
    }

    #[test]
    fn write_leaves_no_tmp_files_on_success() {
        let dir = tempdir().unwrap();
        write_profile(dir.path(), &pname("test"), b"x").unwrap();
        let stragglers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(stragglers.is_empty(), "leftover tmp files: {stragglers:?}");
    }

    #[test]
    fn list_excludes_hidden_and_non_csv() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.csv"), "").unwrap();
        fs::write(dir.path().join("b.csv"), "").unwrap();
        fs::write(dir.path().join(".DS_Store"), "").unwrap();
        fs::write(dir.path().join("notes.txt"), "").unwrap();
        let names: Vec<_> = list_profiles(dir.path())
            .unwrap()
            .into_iter()
            .map(|p| p.name.as_filename().to_string())
            .collect();
        assert_eq!(names, vec!["a.csv".to_string(), "b.csv".to_string()]);
    }

    #[test]
    fn list_classifies_kind() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("default.csv"), "").unwrap();
        fs::write(dir.path().join("prefs.csv"), "").unwrap();
        fs::write(dir.path().join("destiny.csv"), "").unwrap();
        let entries = list_profiles(dir.path()).unwrap();
        let kinds: std::collections::HashMap<_, _> = entries
            .iter()
            .map(|e| (e.name.as_filename().to_string(), e.kind))
            .collect();
        assert_eq!(kinds["default.csv"], ProfileKind::Default);
        assert_eq!(kinds["prefs.csv"], ProfileKind::Prefs);
        assert_eq!(kinds["destiny.csv"], ProfileKind::Game);
    }

    #[test]
    fn list_sweeps_stale_tmp_files() {
        let dir = tempdir().unwrap();
        let stale = dir.path().join("orphan.csv.tmp.12345");
        fs::write(&stale, "leftover").unwrap();
        let old = SystemTime::now() - Duration::from_mins(2);
        let times = fs::FileTimes::new().set_modified(old).set_accessed(old);
        fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_times(times)
            .unwrap();
        list_profiles(dir.path()).unwrap();
        assert!(!stale.exists(), "stale .tmp should have been swept");
    }

    #[test]
    fn list_keeps_csv_files_with_tmp_in_middle_of_name() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("legit.csv"), "").unwrap();
        // Pathological name with both ".csv.tmp." substring and a .csv suffix.
        fs::write(dir.path().join("foo.csv.tmp.bar.csv"), "").unwrap();
        let stale = dir.path().join("orphan.csv.tmp.deadbeef");
        fs::write(&stale, "leftover").unwrap();
        let old = SystemTime::now() - Duration::from_mins(2);
        let times = fs::FileTimes::new().set_modified(old).set_accessed(old);
        fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_times(times)
            .unwrap();
        let names: Vec<_> = list_profiles(dir.path())
            .unwrap()
            .into_iter()
            .map(|p| p.name.as_filename().to_string())
            .collect();
        assert_eq!(
            names,
            vec!["foo.csv.tmp.bar.csv".to_string(), "legit.csv".to_string()]
        );
        assert!(!stale.exists(), "stale .tmp should still be swept");
    }

    #[test]
    fn list_leaves_fresh_tmp_files_alone() {
        let dir = tempdir().unwrap();
        let fresh = dir.path().join("in_flight.csv.tmp.67890");
        fs::write(&fresh, "ongoing write").unwrap();
        list_profiles(dir.path()).unwrap();
        assert!(fresh.exists(), "fresh .tmp should not be swept");
    }

    #[test]
    fn delete_removes_file() {
        let dir = tempdir().unwrap();
        write_profile(dir.path(), &pname("x"), b"data").unwrap();
        delete_profile(dir.path(), &pname("x")).unwrap();
        assert!(read_profile(dir.path(), &pname("x")).is_err());
    }

    #[test]
    fn rename_moves_file() {
        let dir = tempdir().unwrap();
        write_profile(dir.path(), &pname("a"), b"hi").unwrap();
        rename_profile(dir.path(), &pname("a"), &pname("b")).unwrap();
        assert!(read_profile(dir.path(), &pname("a")).is_err());
        assert_eq!(read_profile(dir.path(), &pname("b")).unwrap(), b"hi");
    }
}
