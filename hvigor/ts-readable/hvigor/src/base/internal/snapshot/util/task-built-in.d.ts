/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/task-built-in.d.ts
 */
export declare namespace TaskBuiltIn {
    enum Execution {
        COMMAND = "BUILTIN_TASK_COMMAND",
        TOOLCHAIN = "BUILTIN_TASK_TOOLCHAIN",
        ENV = "BUILTIN_TASK_ENV"
    }
    const snapshotSerializationReplacer: (key: string, value: any) => any;
    const snapshotSerializationReceiver: (key: string, jsonEntry: any) => any;
}
