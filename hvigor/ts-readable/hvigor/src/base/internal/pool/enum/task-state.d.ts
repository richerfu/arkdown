/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/enum/task-state.d.ts
 */
export declare enum TaskState {
    WAITING = "waiting",
    RUNNING = "running",
    END = "end",
    REJECT = "reject",
    ERROR = "error"
}
export declare const transitionMap: Map<TaskState, Set<TaskState>>;
