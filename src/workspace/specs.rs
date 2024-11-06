use color_eyre::eyre::{Context, Result};
use dashmap::DashMap;
use lsp_types::Uri;
use notify::{Event, EventKind};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::{
    collections::HashMap,
    fs::{self, read_dir},
    path::{Path, PathBuf},
};
use tracing::instrument;

fn is_a_validator<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    path.is_file()
        && path
            .file_name()
            .map(|name| name.to_string_lossy().ends_with(".hl7v.toml"))
            .unwrap_or(false)
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct WorkspaceSpec {
    /// Name of the custom spec
    pub name: String,

    /// Custom segments
    pub segments: Vec<SegmentSpec>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct SegmentSpec {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub fields: HashMap<usize, FieldSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct FieldSpec {
    pub description: Option<String>,
    pub datatype: Option<String>,
    pub required: Option<bool>,
    pub allowed_values: Option<Vec<(String, String)>>,
}

impl WorkspaceSpec {
    #[instrument(level = "debug")]
    pub fn load_spec<P: AsRef<Path> + std::fmt::Debug>(path: P) -> Result<Self> {
        let spec = toml::from_str(&fs::read_to_string(path).wrap_err("Failed to read file")?)
            .wrap_err("Failed to parse TOML")?;
        tracing::trace!(?spec, "Loaded spec");

        Ok(spec)
    }
}

#[derive(Debug)]
pub struct WorkspaceSpecs {
    pub specs: DashMap<PathBuf, WorkspaceSpec>,
}

impl WorkspaceSpecs {
    #[instrument(level = "debug", skip(workspace_folders))]
    pub fn new<I, P>(workspace_folders: I) -> Result<Self>
    where
        I: Iterator<Item = P>,
        P: AsRef<Path> + std::fmt::Debug,
    {
        let specs = DashMap::new();

        for folder in workspace_folders {
            let folder = folder.as_ref();
            let folder_span = tracing::debug_span!("folder", folder = ?folder);
            let _folder_guard = folder_span.enter();

            tracing::debug!(?folder, "Reading directory for custom validator scripts");
            for entry in read_dir(folder)
                .wrap_err_with(|| format!("Failed to read directory: {folder:?}"))?
            {
                let entry_span = tracing::debug_span!("entry");
                let _entry_guard = entry_span.enter();

                let entry = entry.wrap_err("Failed to read directory entry")?;
                let path = entry.path();

                if is_a_validator(&path) {
                    match WorkspaceSpec::load_spec(&path) {
                        Ok(spec) => {
                            tracing::debug!(?path, "Custom validator script found");
                            tracing::trace!(?spec, "Loaded spec");
                            specs.insert(path.clone(), spec);
                        }
                        Err(e) => {
                            tracing::error!(?e, ?path, "Failed to load spec");
                        }
                    }
                }
            }
        }

        Ok(WorkspaceSpecs { specs })
    }

    #[instrument(level = "debug", skip(self))]
    pub fn update(&self, event: Event) -> Result<bool> {
        let mut changed = false;
        let Event { paths, kind, .. } = event;
        match kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in paths.iter() {
                    if is_a_validator(path) {
                        tracing::debug!(?path, "Custom validator script created/modified");
                        match WorkspaceSpec::load_spec(path) {
                            Ok(spec) => {
                                self.specs.insert(path.clone(), spec);
                                changed = true;
                            }
                            Err(e) => {
                                tracing::error!(?e, ?path, "Failed to load custom spec");
                            }
                        }
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in paths.iter() {
                    if self.specs.contains_key(path) {
                        tracing::debug!(?path, "Custom validator script removed");
                        self.specs.remove(path);
                        changed = true;
                    }
                }
            }
            _ => {}
        }

        Ok(changed)
    }

    fn spec_applies_to_uri(spec_path: &Path, uri: &Uri) -> bool {
        let path = PathBuf::from(uri.path().as_str());
        let spec_path = spec_path.canonicalize().ok();
        spec_path
            .and_then(|spec_path| spec_path.parent().map(|p| p.to_path_buf()))
            .filter(|p| path.starts_with(p))
            .is_some()
    }

    // TODO: rewrite this without cloning
    // pub fn specs_for_uri(&self, uri: &Uri) -> Vec<WorkspaceSpec> {
    //     (&self.specs)
    //         .into_iter()
    //         .filter_map(|x| {
    //             let (path, spec) = x.pair();
    //             if WorkspaceSpecs::spec_applies_to_uri(path, uri) {
    //                 Some(spec.clone())
    //             } else {
    //                 None
    //             }
    //         })
    //         .collect()
    // }

    pub fn describe_field(&self, uri: &Uri, segment: &str, field: usize) -> String {
        (&self.specs)
            .into_iter()
            .filter_map(|x| {
                let (path, spec) = x.pair();
                if !WorkspaceSpecs::spec_applies_to_uri(path, uri) {
                    return None;
                }

                spec.segments
                    .iter()
                    .find(|s| s.name == segment)
                    .and_then(|s| s.fields.get(&field))
                    .map(|f| {
                        let description = f.description.clone();
                        let datatype = f.datatype.as_ref().map(|d| format!("({d})"));
                        let required = match f.required {
                            Some(true) => Some("[*required*]".to_string()),
                            Some(false) => Some("[*optional*]".to_string()),
                            None => None,
                        };
                        let table_values = f
                            .allowed_values
                            .as_ref()
                            .map(|v| {
                                v.iter()
                                    .map(|(k, v)| format!("        `{k}` ({v})"))
                                    .collect::<Vec<String>>()
                                    .join("\n")
                            })
                            .unwrap_or_default();
                        let table_values = if table_values.is_empty() {
                            None
                        } else {
                            Some(format!("\n      Table values:\n{table_values}"))
                        };

                        if description.is_none()
                            && datatype.is_none()
                            && required.is_none()
                            && table_values.is_none()
                        {
                            return None;
                        }
                        Some(format!(
                            "\n    {spec_name}:\n      {desc}",
                            spec_name = spec.name,
                            desc = [description, datatype, required, table_values]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<String>>()
                                .join(" ")
                        ))
                    })
            })
            .flatten()
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn table_values(&self, uri: &Uri, segment: &str, field: usize) -> Vec<(String, String)> {
        (&self.specs)
            .into_iter()
            .filter_map(|x| {
                let (path, spec) = x.pair();
                if !WorkspaceSpecs::spec_applies_to_uri(path, uri) {
                    return None;
                }

                spec.segments
                    .iter()
                    .find(|s| s.name == segment)
                    .and_then(|s| s.fields.get(&field))
                    .map(|f| f.allowed_values.clone())
            })
            .flatten()
            .next() // TODO: merge allowed values? or only pick one? or group by validation?
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_can_roundtrip_with_toml() {
        let my_spec = WorkspaceSpec {
            name: "My Custom Spec".to_string(),
            segments: vec![
                SegmentSpec {
                    name: "MSH".to_string(),
                    description: Some("Message Header".to_string()),
                    required: Some(true),
                    fields: [(
                        1,
                        FieldSpec {
                            description: Some("Field Separator".to_string()),
                            datatype: Some("ST".to_string()),
                            required: Some(true),
                            allowed_values: Some(vec![
                                ("|".to_string(), "Pipe".to_string()),
                                ("^".to_string(), "Caret".to_string()),
                                ("~".to_string(), "Tilde".to_string()),
                            ]),
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
                SegmentSpec {
                    name: "PID".to_string(),
                    description: Some("Patient Identification".to_string()),
                    required: None,
                    fields: [(
                        3,
                        FieldSpec {
                            description: Some("Patient Identifier List".to_string()),
                            datatype: Some("CX".to_string()),
                            required: Some(true),
                            allowed_values: None,
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
            ],
        };

        let toml_spec = toml::to_string(&my_spec).expect("Can serialize spec");
        eprintln!("{}", toml_spec);
        let roundtripped_spec: WorkspaceSpec =
            toml::from_str(&toml_spec).expect("Can deserialize spec");
        assert_eq!(my_spec, roundtripped_spec);
    }

    #[test]
    fn the_sample_spec_can_be_loaded() {
        WorkspaceSpec::load_spec("sample.hl7v.toml").expect("Can load sample spec");
    }
}
