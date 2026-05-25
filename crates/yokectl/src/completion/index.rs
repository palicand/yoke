use clap_complete::CompletionCandidate;
use yoke_index::cache::Cache;

#[derive(Clone)]
pub struct IndexEntryCompleter;

impl clap_complete::engine::ValueCandidates for IndexEntryCompleter {
    fn candidates(&self) -> Vec<CompletionCandidate> {
        let Some(cache) = Cache::from_project_dirs() else {
            return Vec::new();
        };
        cache
            .read_entries_sync()
            .ok()
            .into_iter()
            .flatten()
            .map(CompletionCandidate::new)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap_complete::engine::ValueCandidates;

    #[test]
    fn returns_without_panic_when_cache_absent() {
        let _ = IndexEntryCompleter.candidates();
    }
}
