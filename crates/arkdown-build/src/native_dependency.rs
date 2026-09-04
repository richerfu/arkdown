use std::sync::Arc;

use arkdown_core::{ArkdownConfig, ModuleConfig};
use arkdown_native::NativeCmakeLocalDependency;
use arkdown_session::ModuleCompileContext;

/// One local module whose native outputs are required by a dependent module.
pub struct NativeDependencyStageInput {
    pub module: ModuleConfig,
    pub module_context: ModuleCompileContext,
    pub config: Arc<ArkdownConfig>,
    pub target_name: String,
    pub local_dependencies: Arc<[NativeCmakeLocalDependency]>,
}

/// Dependency-first native module waves for one build invocation.
///
/// Waves remain explicit because CMake projects can share generated
/// dependency packages. Parallel execution requires a disjoint-output proof.
#[derive(Default)]
pub struct NativeDependencyStagePlan {
    pub(crate) waves: Vec<Vec<NativeDependencyStageInput>>,
}

impl NativeDependencyStagePlan {
    pub fn new(waves: Vec<Vec<NativeDependencyStageInput>>) -> Self {
        Self { waves }
    }

    pub fn is_empty(&self) -> bool {
        self.waves.is_empty()
    }

    pub fn waves(&self) -> &[Vec<NativeDependencyStageInput>] {
        &self.waves
    }
}
