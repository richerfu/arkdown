/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/previewer/reuse-watch-worker.d.ts
 */
interface ReuseMetaData {
    modulePath: string;
    pageType: string;
    pagesPath: string[];
}
/**
 * 用于管理预览时上一次预览已创建的worker是否能复用
 */
export declare class ReuseWatchWorkerManager {
    private static logger;
    private static preWorkerConfig?;
    /**
     * 复用已有的预览worker
     * @returns {boolean} 是否成功复用
     */
    static reuseWatchWorker(reuseMetaData: ReuseMetaData, useNormalWorker: boolean, previewPagePath: string): Promise<boolean>;
    /**
     * 废弃 preWorkerConfig 中保存的信息
     * @returns {boolean} 复用是否成功
     */
    static clearPreWorkerConfig(): void;
    /**
     * 创建worker并缓存新worker的元数据
     * @returns {boolean} 复用是否成功
     */
    static createWorker(reuseMetaData: ReuseMetaData, useNormalWorker: boolean, createPreviewWorker: () => string): void;
    private static updatePreWorkerConfig;
    private static canReuseWorker;
}
export {};
