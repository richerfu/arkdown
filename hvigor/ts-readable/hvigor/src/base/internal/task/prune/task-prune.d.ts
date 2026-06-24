/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/prune/task-prune.d.ts
 */
import { HvigorCoreNode } from '../../../external/core/hvigor-core-node';
import { DefaultTask } from '../../../external/task/default-task';
/**
 * hvigor prune命令 任务
 *
 * @since 2023/6/20
 */
export declare class PruneTask extends DefaultTask {
    constructor(hvigorNode: HvigorCoreNode);
    registryAction(): Function;
}
