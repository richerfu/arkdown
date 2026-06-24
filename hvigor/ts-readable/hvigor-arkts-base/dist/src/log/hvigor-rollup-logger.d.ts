/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/log/hvigor-rollup-logger.d.ts
 */
import { HvigorErrorInfo, MergeKeyEnum, MergeOption, HvigorLoggerInterface, THvigorErrorInfo } from '@ohos/hvigor-logger';
/**
 * 为rollup框架提供的打印类。
 * rollup框架通过启动任务传递给子任务日志对象，子任务仅进行错误信息的压栈，等任务结束后，hvigor根据收集到的信息进行打印。
 */
export declare class HvigorRollupLogger implements HvigorLoggerInterface {
    private subSystem;
    private hvigorErrorCommonAdapter;
    private hvigorLogList;
    private mergeLogList;
    private logEventType;
    constructor(subSystem: string, type: string);
    /**
     * 打印一个错误信息到控制台，使用方可以传一个纯对象
     * @param error
     */
    printError(error: HvigorErrorInfo | THvigorErrorInfo): void;
    /**
     * 打印一个错误信息到控制台,不阻塞构建
     * @param error
     */
    printErrorAndExit(error: HvigorErrorInfo | THvigorErrorInfo): void;
    /**
     * 打印一个告警信息到控制台
     * @param warn
     */
    printWarn(warn: string): void;
    /**
     * 打印一个提示信息到控制台
     * @param info
     */
    printInfo(info: string): void;
    /**
     * 打印一个调试信息到控制台
     * @param debug
     */
    printDebug(debug: string): void;
    /**
     * 收集错误信息到数组
     * @param error
     * @param isForMerge
     */
    pushError(error: HvigorErrorInfo | THvigorErrorInfo, isForMerge?: boolean): void;
    /**
     * 按收集顺序打印收集到的所有错误信息，优先使用入参数组进行打印
     */
    printAllError(errorList?: HvigorErrorInfo[] | THvigorErrorInfo[]): void;
    /**
     * 针对收集到的错误信息列表，相同key的ErrorInfo合并到一起，其余字段可配置是否合并或保留最后一个
     * @param mergeKey 指定当code或cause或solutions或moreInfo相同时，对错误信息列表里的元素进行合并
     * @param option mergeKey相同的元素，其余字段的合并策略。不配置的字段默认按COLLECT_LAST策略合并。
     *
     * 示例：
     * 已通过pushError()函数将如下错误压栈。
     * hvigorLogList：[{
     *  code: '11020100',
     *  description: 'API not support.',
     *  cause: 'The function `getDomInfo()` is not support under API level 12.',
     *  position: 'line 34 of D://path/to/the/invalid/file/test.ets',
     *  solutions: ['Update API level to 12 or higher.','Use function `getDom()` instead of `getDomInfo()`.'],
     *  moreInfo?: {
     *      cn: 'https://123/cn/235',
     *      en: 'https://123/en/235'
     *  },
     * }, {
     *  code: '11020100',
     *  description: 'API not support.',
     *  cause: 'The function `setWidth()` is not support under API level 10.',
     *  position: 'line 134 of D://path/to/the/invalid/file/demo.ets.',
     *  solutions: ['Update API level to 10 or higher.'],
     *  moreInfo?: {
     *      cn: 'https://123/cn/67',
     *      en: 'https://123/en/67'
     *  },
     * }, {
     *  code: '11020100',
     *  description: 'API not support.',
     *  cause: 'The function `maxmise()` is not support under API level 10.',
     *  position: 'line 13 of D://path/to/the/invalid/file/demo.ets.',
     *  solutions: ['Update API level to 10 or higher.'],
     *  moreInfo?: {
     *      cn: 'https://123/cn/536',
     *      en: 'https://123/en/536'
     *  },
     * }, {
     *  code: '21030400',
     *  description: 'The capacity exceeds the threshold.',
     *  cause: 'The `maxOldSpace` configed in file hvigor-config.json5 is higher than 2000.',
     *  position: 'the `maxOldSpace` of `nodeOptions` in D://path/to/the/invalid/file/hvigor-config.json5.',
     *  solutions: ['Adjusting `maxOldSpace` no higher than 2000.'],
     *  moreInfo?: {
     *      cn: 'https://123/cn/2234',
     *      en: 'https://123/en/2334'
     *  },
     * }]
     *
     * 示例1：调用函数 printMergedError('code', {cause: MergeType.COLLECT_ALL, position: MergeType.COLLECT_ALL}) 则打印效果如下：
     * > hvigor ERROR Code: 11020100 :entry:default@CompileResource
     * ERROR: API not support.
     * Error Message: The function `getDomInfo()` is not support under API level 12. At line 34 of D://path/to/the/invalid/file/test.ets.
     * The function `setWidth()` is not support under API level 10. At line 134 of D://path/to/the/invalid/file/demo.ets.
     * The function `maxmise()` is not support under API level 10. At line 13 of D://path/to/the/invalid/file/demo.ets.
     *
     * * Try the following:
     *   > Update API level to 10 or higher.
     *   > More info: https://123/cn/536   // 说明：中文环境下使用cn，英文环境下使用en
     *
     * > hvigor ERROR Code: 21030400 :entry:default@CompileResource
     * ERROR: The capacity exceeds the threshold.
     * Error Message: The `maxOldSpace` configed in file hvigor-config.json5 is higher than 2000. At the `maxOldSpace` of `nodeOptions` in D://path/to/the/invalid/file/hvigor-config.json5.
     *
     * * Try the following:
     *   > Adjusting `maxOldSpace` no higher than 2000.
     *   > More info: https://123/cn/2234
     *
     * 示例2：调用函数 printMergedError('code', {cause: MergeType.COLLECT_ALL, solutions: MergeType.COLLECT_FIRST}) 则打印效果如下：
     * > hvigor ERROR Code: 11020100 :entry:default@CompileResource
     * ERROR: API not support.
     * Error Message: The function `getDomInfo()` is not support under API level 12. At line 13 of D://path/to/the/invalid/file/demo.ets.
     * The function `setWidth()` is not support under API level 10. At line 13 of D://path/to/the/invalid/file/demo.ets.
     * The function `maxmise()` is not support under API level 10. At line 13 of D://path/to/the/invalid/file/demo.ets.
     *
     * * Try the following:
     *   > Update API level to 12 or higher.
     *   > Use function `getDom()` instead of `getDomInfo()`.
     *   > More info: https://123/cn/536
     *
     * > hvigor ERROR Code: 21030400 :entry:default@CompileResource
     * ERROR: The capacity exceeds the threshold.
     * Error Message: The `maxOldSpace` configed in file hvigor-config.json5 is higher than 2000. At the `maxOldSpace` of `nodeOptions` in D://path/to/the/invalid/file/hvigor-config.json5.
     *
     * * Try the following:
     *   > Adjusting `maxOldSpace` no higher than 2000.
     *   > More info: https://123/cn/2234
     *
     *
     */
    printMergedError(mergeKey: MergeKeyEnum, option?: MergeOption): void;
    private getLogEvent;
}
