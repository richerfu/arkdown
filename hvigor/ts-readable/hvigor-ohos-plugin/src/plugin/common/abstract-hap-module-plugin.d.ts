/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/common/abstract-hap-module-plugin.d.ts
 */
import { Module } from '@ohos/hvigor';
import { AssembleHap } from '../../tasks/hook/assemble/assemble-hap';
import { BuildPreviewerRes } from '../../tasks/hook/previewer/build-previewer-res';
import { PreviewBuild } from '../../tasks/hook/previewer/preview-build';
import { UnitTestBuild } from '../../tasks/unitTest/unit-test-build';
import { AbstractModulePlugin } from './abstract-module-plugin';
/**
 * 对外暴露hap级别的接口和任务的plugin
 *
 * @since 2021/12/16
 */
export declare abstract class AbstractHapModulePlugin extends AbstractModulePlugin {
    protected buildPreviewerRes: BuildPreviewerRes | undefined;
    protected previewBuild: PreviewBuild | undefined;
    protected unitTestBuild: UnitTestBuild | undefined;
    assembleHap: AssembleHap | undefined;
    protected constructor(module: Module);
    abstract initHapTasks(): void;
    initGlobalData(): void;
}
