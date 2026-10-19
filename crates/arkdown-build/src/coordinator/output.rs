use std::path::PathBuf;

/// Typed output of a complete build run.
#[derive(Default)]
pub struct BuildOutput {
    /// Primary packages consumed by parent APP assembly and device-test stages.
    pub artifact_paths: Vec<PathBuf>,
    /// Other deliverables published by the stages in this build, including unsigned packages.
    pub(crate) additional_artifacts: Vec<PathBuf>,
}

impl BuildOutput {
    pub(crate) fn new(artifact_paths: Vec<PathBuf>) -> Self {
        Self {
            artifact_paths,
            additional_artifacts: Vec::new(),
        }
    }

    pub(crate) fn with_additional_artifacts(mut self, paths: Vec<PathBuf>) -> Self {
        self.additional_artifacts = paths;
        self
    }

    pub(crate) fn extend(&mut self, output: Self) {
        self.artifact_paths.extend(output.artifact_paths);
        self.additional_artifacts
            .extend(output.additional_artifacts);
    }

    /// Deterministic public result; only includes outputs registered by their producing stage.
    pub fn all_artifact_paths(&self) -> Vec<PathBuf> {
        let mut paths = self
            .artifact_paths
            .iter()
            .chain(&self.additional_artifacts)
            .cloned()
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        paths
    }
}
