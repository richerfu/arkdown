/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/index.d.ts
 */
export { replacer } from './src/common/util/replacer';
export { hvigor } from './src/base/external/core/hvigor';
export { BuildResult } from './src/base/external/models/build-result';
export { getNode } from './src/base/external/core/hvigor-core';
export { hvigorConfig } from './src/base/external/core/hvigor-config';
export { FileUtil } from './src/common/util/hvigor-file-util';
export { TaskInput, TaskOutput } from './src/base/external/api/hvigor-api';
export type { HvigorPlugin, HvigorNode, HvigorTask, Task } from './src/base/external/api/hvigor-api';
/** 对外提供hvigor自定义任务可以使用线程池的能力 */
export { submitWorker, WorkerOption } from './src/base/external/worker/worker';
export { Priority } from './src/base/internal/pool/enum/priority';
export { TCB } from './src/base/internal/pool/model/tcb';
