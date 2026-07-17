//! Notify adapter for Vault changes. Indexing belongs to `IndexService`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{debug, info, warn};

use crate::application::index_service::IndexService;
#[cfg(test)]
use crate::application::ports::RepositoryHandle;
#[cfg(test)]
use crate::domain::entity::Weights;
use crate::infrastructure::vault_adapter::is_sync_conflict_path;
#[cfg(test)]
use crate::infrastructure::vault_adapter::VaultAdapter;

const DEBOUNCE_MS: u64 = 500;
const HIDDEN_DIRS: &[&str] = &[".obsidian", ".compass", ".git"];

pub struct FileWatcher {
    vault: PathBuf,
    indexer: Arc<IndexService>,
    watcher: Option<RecommendedWatcher>,
}

impl FileWatcher {
    pub fn new(vault: PathBuf, indexer: Arc<IndexService>) -> Self {
        Self {
            vault,
            indexer,
            watcher: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_millis(100)),
        )?;
        watcher.watch(&self.vault, RecursiveMode::Recursive)?;
        info!(vault = %self.vault.display(), "FileWatcher started");
        self.watcher = Some(watcher);

        let vault = self.vault.clone();
        let indexer = Arc::clone(&self.indexer);
        tokio::spawn(async move {
            let mut paths = HashSet::new();
            let mut debounce_timer: Option<tokio::time::Instant> = None;
            let mut rebuild_required = false;
            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        let (event_paths, event_requires_rebuild) = collect_event_paths(
                            &event.paths,
                            matches!(&event.kind, EventKind::Remove(_)),
                        );
                        paths.extend(event_paths);
                        rebuild_required |= event_requires_rebuild;
                        if !paths.is_empty() || rebuild_required {
                            debounce_timer = Some(tokio::time::Instant::now());
                        }
                    }
                    _ = async {
                        if let Some(timer) = debounce_timer {
                            let elapsed = timer.elapsed();
                            if elapsed < Duration::from_millis(DEBOUNCE_MS) {
                                tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS) - elapsed).await;
                            }
                        } else {
                            std::future::pending::<()>().await;
                        }
                    }, if debounce_timer.is_some() => {
                        if rebuild_required {
                            match indexer.rebuild().await {
                                Ok(stats) => info!(indexed = stats.indexed, skipped = stats.skipped, duplicates = stats.duplicates, "rebuilt after sync conflict"),
                                Err(error) => warn!(%error, "sync-conflict rebuild failed"),
                            }
                        } else {
                            for path in &paths {
                                let relative = relative_path(&vault, path);
                                match indexer.process_changed_path(&relative).await {
                                    Ok(()) => debug!(path = %path.display(), "processed"),
                                    Err(error) => warn!(path = %path.display(), %error, "process failed"),
                                }
                            }
                        }
                        paths.clear();
                        rebuild_required = false;
                        debounce_timer = None;
                    }
                }
            }
        });
        Ok(())
    }
}

/// Transitional test helper retained for existing behavior tests. It delegates
/// every indexing decision to the application service.
#[cfg(test)]
pub(crate) async fn process_single_file(
    vault: &Path,
    repository: &RepositoryHandle,
    weights: &Weights,
    path: &Path,
) -> Result<()> {
    let adapter = Arc::new(VaultAdapter::new(vault.to_path_buf()));
    let indexer = IndexService::new(adapter, Arc::clone(repository), *weights);
    indexer
        .process_changed_path(&relative_path(vault, path))
        .await
}

fn relative_path(vault: &Path, path: &Path) -> String {
    path.strip_prefix(vault)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_hidden_path(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        ancestor
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| HIDDEN_DIRS.contains(&name))
    })
}

fn collect_event_paths(event_paths: &[PathBuf], is_remove: bool) -> (HashSet<PathBuf>, bool) {
    let mut paths = HashSet::new();
    let mut rebuild_required = false;
    for path in event_paths {
        if is_hidden_path(path) {
            continue;
        }
        if is_sync_conflict_path(path) {
            rebuild_required = true;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("md")
            || (is_remove && !path.exists())
        {
            paths.insert(path.clone());
        }
    }
    (paths, rebuild_required)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_paths_are_filtered() {
        assert!(is_hidden_path(&PathBuf::from("/vault/.obsidian/config.md")));
        assert!(!is_hidden_path(&PathBuf::from("/vault/knowledge/test.md")));
    }

    #[test]
    fn relative_paths_use_forward_slashes() {
        let vault = PathBuf::from("/vault");
        assert_eq!(
            relative_path(&vault, &vault.join("knowledge/test.md")),
            "knowledge/test.md"
        );
    }

    #[test]
    fn sync_conflict_event_requests_rebuild_and_retains_removed_primary_path() {
        let removed_primary = PathBuf::from("/vault/Knowledge/note.md");
        let conflict =
            PathBuf::from("/vault/Knowledge/note.sync-conflict-20260712-120000-DEVICE.md");

        let (paths, rebuild_required) =
            collect_event_paths(&[removed_primary.clone(), conflict], true);

        assert!(rebuild_required);
        assert!(paths.contains(&removed_primary));
    }
}
