/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/util.d.ts
 */
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
/**
 * 设置环境变量，供loader使用，后续等loader适配完成可删除
 *
 * @param config 编译参数
 */
export declare const generateLoaderEnv: (config: ProjectConfig) => void;
export declare function setLogLevel(level: string): void;
export declare function onTerminateWorker(): {
    updatePkgName2SourceRoots(data: Record<string, string[]>): void;
};
/**
 * 定义worker线程中监听CustomWorkerRequest的监听函数
 * @param data
 */
export declare function onCustomWorkerRequest(data: {
    customType: string;
    param: object;
}): void;
