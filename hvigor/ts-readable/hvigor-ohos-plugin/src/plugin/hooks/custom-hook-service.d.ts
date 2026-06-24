/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/custom-hook-service.d.ts
 */
export declare class CustomHookActionService {
    private moduleCustomHookActions;
    /**
     * 根据模块路径、Hook类型、target名称注册自定义钩子函数
     * @param modulePath 模块路径
     * @param customType Hook类型
     * @param fn 钩子函数
     * @param targetName Target名称
     */
    registerCustomAction(modulePath: string, customType: string, fn: Function, targetName?: string): void;
    /**
     * 根据模块路径、Hook类型、target名称获取自定义钩子函数
     * @param modulePath
     * @param customType
     * @param targetName
     */
    getCustomAction(modulePath: string, customType: string, targetName: string): Function | undefined;
    /**
     * 清理
     */
    clean(): void;
}
export declare const customHookActionService: CustomHookActionService;
