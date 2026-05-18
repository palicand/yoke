use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use yoke_volume::VolumeProvider;
use yoke_volume::profile::ProfileName;

#[allow(dead_code)]
pub enum Target {
    VolumeName(ProfileName),
    LocalFile(PathBuf),
    Stdin,
}

#[allow(dead_code)]
impl Target {
    pub fn classify(raw: &str) -> Self {
        if raw == "-" {
            return Self::Stdin;
        }
        let p = PathBuf::from(raw);
        if p.exists() {
            return Self::LocalFile(p);
        }
        ProfileName::new(raw).map_or(Self::LocalFile(p), Self::VolumeName)
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
}
