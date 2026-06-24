/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/log/clang-log-diagnostic.d.ts
 */
declare enum Level {
    INFO = 0,
    WARNING = 1,
    ERROR = 2
}
export declare class ClangLogHandler {
    /**
     * 表示当前日志分析状态
     *
     * 0: 当前不在告警或错误状态
     * 1: 当前在告警或错误状态
     *
     * @private
     */
    private state;
    /**
     * 当日志分析状态为1时, 将日志存储当lines中
     *
     * @private
     */
    private lines;
    /**
     * 当日志分析状态进入错误或告警状态时, 记录主诊断.
     * 主诊断类型的日志级别决定当前分析的所有日志行的日志级别
     *
     * @private
     */
    private currentDiagnostic?;
    /**
     * 记录分析并经过日志级别包装过的日志
     */
    private _text;
    log(line: string): void;
    /**
     * 根据日志诊断判断当前状态
     *
     * @param classification
     */
    updateState(classification: LogDiagnostic): void;
    get text(): string;
    /**
     * 根据当前日志级别追加到text
     *
     * @param classification
     * @param line
     */
    logging(classification: LogDiagnostic, line: string): void;
}
export declare class LogDiagnostic {
    /**
     * 诊断名称
     *
     * For example:
     *    CLANG_COMPILER_ERROR
     */
    readonly name: string;
    /**
     * 诊断pattern
     *
     * For example:
     *    .*
     */
    readonly pattern: RegExp;
    /**
     * 日志级别
     *
     * For example:
     *    Level.WARNING
     */
    readonly level: Level;
    /**
     * 当前日志是否可能是错误或告警的主要信息
     *
     * For example:
     *    /path/to/app.cpp:5:3: error: no matching function for call to 'bar'
     */
    readonly main: boolean;
    /**
     * 是否可能是跟在告警或错误的主要信息之后
     *
     * For example:
     *    /path/to/app.cpp:1:13: note: candidate not viable: no known conversion from...
     */
    readonly after: boolean;
    /**
     * 是否可能是在告警或错误的主要信息之前
     *
     * For example:
     *    In file included from /path/tp/include1.h:7:
     *    /path/to/include2.h:7:1: error: unknown type name 'snake'
     */
    readonly precede: boolean;
    /**
     * 是否结束当前诊断信息
     *
     * For example:
     *    6 warnings generated.
     */
    readonly complete: boolean;
    static readonly IN_FILES_INCLUDED_FROM: LogDiagnostic;
    static readonly CLANG_COMPILER_ERROR: LogDiagnostic;
    static readonly CLANG_LINKER_ERROR: LogDiagnostic;
    static readonly CLANG_COMPILER_WARNING: LogDiagnostic;
    static readonly CLANG_LINKER_WARNING: LogDiagnostic;
    static readonly LINK_FATAL: LogDiagnostic;
    static readonly CLANG_COMPILER_INFO: LogDiagnostic;
    static readonly CLANG_LINKER_INFO: LogDiagnostic;
    static readonly CLANG_ERRORS_GENERATED: LogDiagnostic;
    static readonly NINJA_ENTERING_DIRECTORY: LogDiagnostic;
    static readonly NINJA_BUILD_STOPPED: LogDiagnostic;
    static readonly FAILED_TARGET: LogDiagnostic;
    static readonly CLANG_COMMAND_LINE: LogDiagnostic;
    static readonly NONE: LogDiagnostic;
    static readonly classifications: LogDiagnostic[];
    constructor(
    /**
     * 诊断名称
     *
     * For example:
     *    CLANG_COMPILER_ERROR
     */
    name: string, 
    /**
     * 诊断pattern
     *
     * For example:
     *    .*
     */
    pattern: RegExp, 
    /**
     * 日志级别
     *
     * For example:
     *    Level.WARNING
     */
    level: Level, 
    /**
     * 当前日志是否可能是错误或告警的主要信息
     *
     * For example:
     *    /path/to/app.cpp:5:3: error: no matching function for call to 'bar'
     */
    main: boolean, 
    /**
     * 是否可能是跟在告警或错误的主要信息之后
     *
     * For example:
     *    /path/to/app.cpp:1:13: note: candidate not viable: no known conversion from...
     */
    after: boolean, 
    /**
     * 是否可能是在告警或错误的主要信息之前
     *
     * For example:
     *    In file included from /path/tp/include1.h:7:
     *    /path/to/include2.h:7:1: error: unknown type name 'snake'
     */
    precede: boolean, 
    /**
     * 是否结束当前诊断信息
     *
     * For example:
     *    6 warnings generated.
     */
    complete: boolean);
    /**
     * 使用当前诊断类型的pattern正则尝试匹配当前日志行
     *
     * @param line
     */
    match(line: string): LogDiagnostic | undefined;
}
export {};
