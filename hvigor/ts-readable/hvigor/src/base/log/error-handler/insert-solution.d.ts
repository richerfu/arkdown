/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/log/error-handler/insert-solution.d.ts
 */
/**
 * 此类用于在未知报错中添加解决方案，此类报错使用统一的错误码00300xxx。如果未命中解决方案，则统一报错误码00300000.
 * 已有错误码的报错不应该调用此类。
 * 识别错误信息的key和相关解决方案存放在specific-solutions.json5中，后续新增对异常场景的处理时只需在specific-solutions.json5文件中新增字段即可。
 * specific-solutions.json5文件中字段介绍：
 * {
 *   "keys": [""], // 识别错误信息的key,所有key都命中了才算命中
 *   "code": "00307001", // 错误码
 *   "desc": "The file is occupied", // 错误码描述
 *   "solutions": [""], // 多个解决方案，如果有占位符以%s进行标记，按顺序对应InsertSolution中的params参数
 *   "moreInfo": {
 *     "cn": "https://developer.huaw***cn-37", // 官网FAQ地址,中文
 *     "en": "https://developer.huaw***en-37"
 *   }
 * }
 *
 */
export declare class InsertSolution {
    private readonly COLOR_STRING_PREFIX;
    private readonly RED_KEY;
    private readonly END_KEY;
    private errorCode;
    private errorDesc;
    private errorMessage;
    private params;
    private errorCodeReg;
    private specificSolutions;
    /**
     * 根据报错信息插入解决方案后返回最终结果
     * @param msg 报错原始信息
     * @param params 要替换的变量，与specific-solutions.json里的内容按顺序匹配
     */
    insertSolution(msg: string, params?: string[]): string | undefined;
    /**
     * 针对没有错误码的场景，插入错误码
     * @private
     */
    private insertErrorCode;
    /**
     * 将自定义参数替换到解决方案中。返回的字符串中每条解决方案都增加了前缀（'  > '）
     * @param solutions
     * @private
     */
    private formatSolution;
    /**
     * 处理参数，按顺序进行替换
     * @param solutions
     * @private
     */
    private handleParameters;
    /**
     * 根据报错信息找到一系列解决方案以及MoreInfo信息
     * @private
     */
    private findSolutionWithMoreInfo;
    /**
     * 根据系统语言返回moreInfo信息
     * @param moreInfo
     * @private
     */
    private getMoreInfo;
    /**
     * 判断keys中的关键字是否都在errorMessage中能命中
     * @param keys
     * @private
     */
    private isKeysMatch;
    /**
     * 将解决方案字符串插入到原始字符串的合适位置，并视情况添加Try the following
     * 1、有错误码的，不添加解决方案，有Try the following的也不添加解决方案
     * 2、只针对没有错误码的场景添加解决方案
     * @param solution 格式化后的字符串
     */
    private insertFormatedSolution;
    /**
     * 找到message中最后一个红色标记
     * @private
     */
    private findLastRedIndex;
    /**
     * 在字符串的两个下标之间查找第一个有颜色标记的位置
     * @param lastRedKeyIndex
     * @param lastEndKeyIndex
     * @private
     */
    private getColorIndex;
    /**
     * 寻找关键字最后一次出现的位置
     * @param keys
     * @private
     */
    private findLastIndex;
}
