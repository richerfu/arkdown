/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/legacy/legacy-app-plugin.d.ts
 */
import { Project } from '@ohos/hvigor';
import { AbstractProjectPlugin } from '../common/abstract-project-plugin';
/**
 * 对外暴露app级别的接口和任务的plugin
 *
 * @since 2021/12/16
 */
export declare class LegacyAppPlugin extends AbstractProjectPlugin {
    constructor(project: Project);
    initTaskService(): void;
}
