use color_eyre::eyre::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use lsp_types::WorkspaceFolder;
use notify::{Event, RecommendedWatcher, Watcher};
use specs::WorkspaceSpecs;
use std::{path::PathBuf, sync::Arc, thread::JoinHandle};
use tracing::instrument;

pub mod specs;

pub struct Workspace {
    pub _folders: Vec<PathBuf>,
    _watcher: RecommendedWatcher,
    pub specs: Arc<WorkspaceSpecs>,
    _watch_handle: JoinHandle<()>,
    pub _custom_spec_changes: Receiver<()>,
}

impl Workspace {
    #[instrument(level = "debug")]
    pub fn new(workspace_folders: Vec<WorkspaceFolder>) -> Result<Self> {
        let folders: Vec<PathBuf> = workspace_folders
            .into_iter()
            .map(|folder| PathBuf::from(folder.uri.path().as_str()))
            .filter(|path| path.exists() && path.is_dir())
            .collect();

        let (tx, rx) = crossbeam_channel::unbounded();
        let mut watcher =
            notify::recommended_watcher(tx).wrap_err("Failed to create file system watcher")?;

        tracing::debug!(?folders, "Watching workspace folders recursively");
        for folder in folders.iter() {
            watcher
                .watch(folder.as_path(), notify::RecursiveMode::Recursive)
                .wrap_err_with(|| format!("Failed to watch directory: {folder:?}"))?;
        }

        let specs =
            Arc::new(WorkspaceSpecs::new(folders.iter()).wrap_err("Failed to load custom specs")?);
        tracing::debug!(?specs, "Loaded specs");
        let (tx_specs, custom_spec_changes) = crossbeam_channel::unbounded();
        let watch_handle = Workspace::watch(rx, specs.clone(), tx_specs);

        let workspace = Workspace {
            _folders: folders,
            _watcher: watcher,
            specs,
            _watch_handle: watch_handle,
            _custom_spec_changes: custom_spec_changes,
        };

        Ok(workspace)
    }

    fn watch(
        rx: Receiver<Result<Event, notify::Error>>,
        specs: Arc<WorkspaceSpecs>,
        tx_specs: Sender<()>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            for event in rx {
                match event {
                    Ok(event) => match specs.update(event) {
                        Ok(changed) => {
                            if changed {
                                tracing::info!("Specs updated");
                                if let Err(e) = tx_specs.send(()) {
                                    tracing::error!(?e, "Failed to send update notification");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(?e, "Failed to update specs");
                        }
                    },
                    Err(e) => {
                        tracing::error!(?e, "Failed to receive event");
                    }
                }
            }
        })
    }
}
