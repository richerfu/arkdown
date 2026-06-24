/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/strategy.d.ts
 */
export type TStrategy<T> = {
    condition: boolean;
    strategy: T;
    name?: string;
};
/**
 * 获取策略模式中某个策略
 * @param strategyList 策略列表
 * @param defaultStrategy 默认的策略，当没命中时返回(默认返回策略列表中的第一个)
 */
export declare function getStrategy<T>(strategyList: TStrategy<T>[], defaultStrategy?: T): T;
