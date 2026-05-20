use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use yoke_volume::VolumeProvider;
use yoke_volume::profile::ProfileName;

pub enum Target {
    VolumeName(ProfileName),
    LocalFile(PathBuf),
    Stdin,
}

impl Target {
    pub fn classify(raw: &str) -> Self {
        if raw == "-" {
            return Self::Stdin;
        }
        // Path separators force the LocalFile interpretation even if the basename is a valid
        // ProfileName — otherwise `./foo.csv` would silently be looked up on the volume.
        if raw.contains('/') || raw.contains('\\') {
            return Self::LocalFile(PathBuf::from(raw));
        }
        ProfileName::new(raw).map_or_else(|_| Self::LocalFile(PathBuf::from(raw)), Self::VolumeName)
    }

    pub fn read_bytes(&self, provider: &dyn VolumeProvider) -> Result<Vec<u8>> {
        match self {
            Self::VolumeName(n) => provider.read_profile(n).context("read from volume"),
            Self::LocalFile(p) => std::fs::read(p).with_context(|| format!("read {}", p.display())),
            Self::Stdin => {
                let mut buf = Vec::new();
                std::io::stdin()
                    .read_to_end(&mut buf)
                    .context("read stdin")?;
                Ok(buf)
            }
        }
    }

    pub fn write_bytes(&self, provider: &dyn VolumeProvider, bytes: &[u8]) -> Result<()> {
        match self {
            Self::VolumeName(n) => provider.write_profile(n, bytes).context("write to volume"),
            Self::LocalFile(p) => {
                std::fs::write(p, bytes).with_context(|| format!("write {}", p.display()))
            }
            Self::Stdin => anyhow::bail!("cannot write to stdin"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn dash_classifies_as_stdin() {
        assert!(matches!(Target::classify("-"), Target::Stdin));
    }

    #[test]
    fn existing_file_classifies_as_local() {
        let f = NamedTempFile::new().unwrap();
        assert!(matches!(
            Target::classify(f.path().to_str().unwrap()),
            Target::LocalFile(_)
        ));
    }

    #[test]
    fn valid_name_classifies_as_volume() {
        assert!(matches!(Target::classify("default"), Target::VolumeName(_)));
    }

    #[test]
    fn path_with_separator_classifies_as_local_regardless_of_existence() {
        assert!(matches!(
            Target::classify("./does-not-exist.csv"),
            Target::LocalFile(_)
        ));
    }
}
