/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/ohpm-load-install.d.ts
 */
import { AppPlugin } from '../app-plugin';
/**
 * 用于处理与ohpm相关的加载和安装任务。
 */
export declare class OhpmLoadInstallService {
    private _log;
    /**
     *
     * @param appPlugin
     */
    ohpmLoadInstall(appPlugin: AppPlugin): void;
    private ohpmLoadInstallForBuild;
    /**
     *
     * @protected
     */
    protected isOnlyCleanTask(): boolean;
}
