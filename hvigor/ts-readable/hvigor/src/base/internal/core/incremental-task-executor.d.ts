/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/core/incremental-task-executor.d.ts
 */
import { CoreTaskImpl } from '../../external/task/core-task-impl';
/**
 * 声明为Incremental,并实现了相关增量方法的task的执行流程
 *
 * @param {CoreTaskImpl} coreTask
 */
export declare function doIncrementalTask(coreTask: CoreTaskImpl): Promise<void>;
