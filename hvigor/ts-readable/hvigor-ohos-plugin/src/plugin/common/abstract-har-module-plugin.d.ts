/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/common/abstract-har-module-plugin.d.ts
 */
import { Module } from '@ohos/hvigor';
import { AssembleHar } from '../../tasks/hook/assemble/assemble-har';
import { BuildHarPreviewerRes } from '../../tasks/hook/previewer/build-har-previewer-res';
import { PreviewBuild } from '../../tasks/hook/previewer/preview-build';
import { UnitTestBuild } from '../../tasks/unitTest/unit-test-build';
import { AbstractModulePlugin } from './abstract-module-plugin';
/**
 * 对外暴露hap级别的接口和任务的plugin
 *
 * @since 2021/12/16
 */
export declare abstract class AbstractHarModulePlugin extends AbstractModulePlugin {
    protected assembleHar: AssembleHar | undefined;
    protected buildHarPreviewerRes: BuildHarPreviewerRes | undefined;
    protected previewBuild: PreviewBuild | undefined;
    protected unitTestBuild: UnitTestBuild | undefined;
    protected constructor(module: Module);
    abstract initHarTasks(): void;
    initGlobalData(): void;
}
