/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/anonymize-stack.d.ts
 */
export declare class AnonymizeStack {
    private stack?;
    private pathList;
    private errorCodeReg;
    private undefinedReg;
    private whitelist;
    constructor(stack?: string | undefined);
    /**
     * 日志脱敏函数
     */
    anonymize(): string;
    /**
     * 从文本中提取8位错误码
     * @param text
     */
    getErrorCode(text: string): string;
    /**
     * 按matchReg进行匹配，返回匹配的字符串，未命中则返回null
     * @private
     */
    private getAnonymizedByMatchReg;
    /**
     * 按contains进行路径脱敏，未命中则返回null
     * 算法：
     * 1、先按IDE安装路径白名单进行匹配
     * 2、若命中路径则需要收集此行数据
     * 3、将IDE安装路径脱敏
     * 4、将路径中的文件名进行脱敏
     *
     * @private
     */
    private getAnonymizedByContains;
    /**
     * 对某行堆栈信息进行脱敏处理
     * @param stack
     * @private
     */
    private getAnonymizedString;
    /**
     * 整理白名单列表中的Contains数组
     * @private
     */
    private prepareWhitelistContains;
    /**
     * 获取当前目录
     * @private
     */
    private getCurrentHome;
    /**
     * 将成员变量stack字符串按\n进行换行分割，返回去掉颜色标签整理后的数组
     * @private
     */
    private splitLines;
}
