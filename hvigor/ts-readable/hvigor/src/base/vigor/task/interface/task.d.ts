/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/task/interface/task.d.ts
 */
export interface Task {
    /**
     * 任务名称，全局唯一
     */
    getName: () => string;
    /**
     * 当前Task依赖的Task列表
     * 前置依赖的tasks, 先执行前置依赖，再执行此task
     */
    getDependencies: () => string[];
    /**
     * 禁用任务
     *
     * @param enable
     */
    setEnable: (enable: boolean) => void;
    /**
     * beforeRun 先进后出
     *
     * @param fn
     */
    beforeRun: (fn: Function) => void;
    /**
     * afterRun 先进先出
     *
     * @param fn
     */
    afterRun: (fn: Function) => void;
}
