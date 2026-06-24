/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/worker-id-helper.d.ts
 */
export declare function getModuleIndex(modulePath: string, isWidget?: boolean): number;
export declare function getWorkerIdWithModule(modulePath: string, isWidget?: boolean): number | undefined;
export declare function setWorkerIdWithModule(modulePath: string, workerId: number, isWidget?: boolean): void;
