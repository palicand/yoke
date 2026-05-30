use clap_complete::CompletionCandidate;
use yoke_config::catalog::{Channel, Input, Modifier, Output, PreferenceSpec, SubProfileMode};

#[derive(Clone, Copy)]
pub enum CatalogKind {
    Input,
    Output,
    Preference,
    Mode,
    Channel,
    Modifier,
}

#[derive(Clone, Copy)]
pub struct CatalogValueCompleter(pub CatalogKind);

impl clap_complete::engine::ValueCandidates for CatalogValueCompleter {
    fn candidates(&self) -> Vec<CompletionCandidate> {
        let names: Vec<String> = match self.0 {
            CatalogKind::Input => Input::all_csv_names().collect(),
            CatalogKind::Output => Output::all_csv_names().collect(),
            CatalogKind::Preference => PreferenceSpec::ALL
                .iter()
                .map(|s| s.id.to_string())
                .collect(),
            CatalogKind::Mode => SubProfileMode::KNOWN
                .iter()
                .map(SubProfileMode::canonical_csv)
                .collect(),
            CatalogKind::Channel => Channel::ALL
                .iter()
                .map(|c| c.canonical_csv().to_string())
                .collect(),
            CatalogKind::Modifier => Modifier::KEYWORDS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        };
        names.into_iter().map(CompletionCandidate::new).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap_complete::engine::ValueCandidates;

    #[test]
    fn channel_candidates_non_empty() {
        let c = CatalogValueCompleter(CatalogKind::Channel);
        assert!(!c.candidates().is_empty());
    }

    #[test]
    fn preference_candidates_non_empty() {
        let c = CatalogValueCompleter(CatalogKind::Preference);
        assert!(!c.candidates().is_empty());
    }

    #[test]
    fn modifier_candidates_non_empty() {
        let c = CatalogValueCompleter(CatalogKind::Modifier);
        assert!(!c.candidates().is_empty());
    }
}
