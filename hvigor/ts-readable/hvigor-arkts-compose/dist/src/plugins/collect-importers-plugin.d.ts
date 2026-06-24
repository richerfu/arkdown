/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/collect-importers-plugin.d.ts
 */
import { PluginContext } from '@ohos/hvigor-arkts-base';
import { ProjectConfig } from '../project-config/project-config';
/**
 * 冷重载时处理changeFileList文件
 * 1. 去除不需要参与编译的文件
 * 2. 判断是否有导入新的文件
 */
export declare function collectImportersPlugin(projectConfig: ProjectConfig): {
    name: string;
    options: (options: InputOptions) => void;
    buildEnd: (this: PluginContext) => void;
};
interface InputOptions {
    input: string[];
}
export {};
