/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/interface/task-details-interface.d.ts
 */
export interface TaskDetails {
    readonly name: string;
    readonly group?: string;
    readonly description?: string;
    readonly sourcePluginId?: string;
    readonly isEnabled?: boolean;
    [propName: string]: unknown;
}
