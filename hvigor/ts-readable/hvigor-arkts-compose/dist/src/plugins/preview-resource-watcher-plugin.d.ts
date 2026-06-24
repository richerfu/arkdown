/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/preview-resource-watcher-plugin.d.ts
 */
import { PluginContext } from '@ohos/hvigor-arkts-base';
/**
 * 预览模式下监听资源文件变化的插件
 *
 * @param resourceFiles 资源文件夹
 */
export declare function previewResourceWatcherPlugin(resourceFiles: string[]): {
    name: string;
    buildStart(this: PluginContext): void;
};
