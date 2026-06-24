//! Declarative effects extracted from `hvigorfile.ts` / `hvigorconfig.ts`.
//!
//! `BuildScriptEffect` enumerates the subset of hvigor script side-effects that
//! arkdown can represent declaratively after OXC static analysis.  Dynamic
//! effects (arbitrary JS execution, runtime plugin resolution) are not
//! representable and must be flagged as diagnostics.
//!
//! # Upstream correspondence
//!
//! | Variant | hvigor/SDK equivalent |
//! |---------|-----------------------|
//! | `HookRegistration` | `hvigorfile.ts` hook callback registration |
//! | `BuildOptionOverride` | `arkOptions.*` override from script |
//! | `CompilePluginFile` | `arkOptions.compilePluginFile` override |
//! | `CustomTask` | `hvigor.task()` or `Plugin.addTaskNode()` registration |
//! | `TaskDependency` | `TaskNode.dependsOn` edge declaration |

use arkdown_path::AbsPath;

// ---------------------------------------------------------------------------
// HookRegistrationEffect
// ---------------------------------------------------------------------------

/// A hook callback registration extracted from a build script.
///
/// # Upstream correspondence
///
/// Corresponds to a hook registered via `hvigorfile.ts`:
/// ```ts
/// hvigor.getProject().afterAllTasksExecuted(() => { ... })
/// ```
#[derive(Debug, Clone)]
pub struct HookRegistrationEffect {
    /// Hook point name (e.g., `"afterCompile"`, `"beforeBundling"`).
    pub hook_point: String,
    /// Module name this hook applies to, or `None` for project-level.
    pub module_name: Option<String>,
    /// The handler function name extracted from the script.
    pub handler_name: String,
}

// ---------------------------------------------------------------------------
// BuildOptionOverrideEffect
// ---------------------------------------------------------------------------

/// A build option override extracted from a build script.
///
/// # Upstream correspondence
///
/// Corresponds to `projectOhosConfigManager.getOverrides` output — the
/// flattened key-value pairs that override the base `BuildOption`.
#[derive(Debug, Clone)]
pub struct BuildOptionOverrideEffect {
    /// The build option path (e.g., `"arkOptions.obfuscation.enable"`).
    pub option_path: String,
    /// The override value (JSON-serialized).
    pub value: String,
}

// ---------------------------------------------------------------------------
// CustomTaskEffect
// ---------------------------------------------------------------------------

/// A custom task node registration extracted from a build script.
///
/// # Upstream correspondence
///
/// Corresponds to a task registered via `hvigor.task()` or
/// `Plugin.addTaskNode()` in a project/module `hvigorfile.ts`.
#[derive(Debug, Clone)]
pub struct CustomTaskEffect {
    /// The task name.
    pub task_name: String,
    /// Module name this task belongs to, or `None` for project-level.
    pub module_name: Option<String>,
    /// Handler function names for the task actions.
    pub action_names: Vec<String>,
    /// Names of tasks this task declares as dependencies.
    pub depends_on: Vec<String>,
}

// ---------------------------------------------------------------------------
// TaskDependencyEffect
// ---------------------------------------------------------------------------

/// A task dependency edge extracted from a build script.
///
/// # Upstream correspondence
///
/// Corresponds to `TaskNode.dependsOn` declarations in `hvigorfile.ts`.
#[derive(Debug, Clone)]
pub struct TaskDependencyEffect {
    /// Name of the task that must run first (dependency).
    pub from_task: String,
    /// Name of the task that depends on `from_task`.
    pub to_task: String,
}

// ---------------------------------------------------------------------------
// BuildScriptEffect
// ---------------------------------------------------------------------------

/// A single declarative effect extracted from a build script by OXC analysis.
///
/// Only effects that can be determined statically from the AST are
/// representable here.  Scripts that rely on runtime computation must produce
/// a `Diagnostic` instead of a `BuildScriptEffect`.
///
/// # Upstream correspondence
///
/// Represents the subset of hvigor script side-effects that arkdown can
/// handle without executing JavaScript.
#[derive(Debug, Clone)]
pub enum BuildScriptEffect {
    /// A hook callback registration.
    HookRegistration(HookRegistrationEffect),

    /// A build option key-value override.
    BuildOptionOverride(BuildOptionOverrideEffect),

    /// An `arkOptions.compilePluginFile` override pointing to a custom plugin.
    CompilePluginFile(AbsPath),

    /// A custom task node registration.
    CustomTask(CustomTaskEffect),

    /// An explicit task-to-task dependency edge.
    TaskDependency(TaskDependencyEffect),
}
