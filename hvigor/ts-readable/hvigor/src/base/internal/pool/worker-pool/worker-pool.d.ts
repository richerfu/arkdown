/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/worker-pool.d.ts
 */
import { Level } from 'log4js';
import { CoreTask } from '../../../external/task/core-task';
import { LogType } from '../enum/log-type';
import { PoolState } from '../enum/pool-state';
import { Log } from '../log-collection/log';
import { TCB } from '../model/tcb';
import { SubmitOption } from './submit-option';
/**
 * 工作池
 * 提供池化管理模块与外部交互的入口
 *
 * @since 2022/8/12
 */
export interface WorkerPool {
    /**
     * 提交一个工作单元
     *
     * @param task 工作单元所属任务
     * @param workPath 工作单元路径
     * @param submitOption 提交的其他配置项
     * @returns {TCB} 任务控制块
     */
    submit(task: CoreTask, workPath: string, submitOption?: SubmitOption): TCB;
    /**
     * 关闭池
     */
    terminate(): Promise<boolean>;
    /**
     * 回收非常驻或无内存缓存线程
     */
    recycle(): Promise<boolean>;
    /**
     * 获取池当前状态
     *
     * @returns {PoolState} 池状态
     */
    getState(): PoolState;
    /**
     * 设置池状态
     *
     * @param state 待设置的池状态
     */
    setState(state: PoolState): void;
    /**
     * 设置池回收的间隔时间
     *
     * @param recycleInterval 待设置的间隔时间
     */
    setRecycleInterval(recycleInterval: number): void;
    /**
     * 设置worker的最大空闲时间
     *
     * @param maxIdleTime 待设置的最大空闲时间
     */
    setMaxIdleTime(maxIdleTime: number): void;
    /**
     * 设置work执行失败的回调函数
     * @param {Function} errorCallback 回调函数
     */
    setErrorCallback(errorCallback: Function): void;
    /**
     * 获得日志
     *
     * @param type 日志类型
     * @returns {Set<Log>|undefined} 日志集合
     */
    getLog(type: LogType): Set<Log> | undefined;
    /**
     * 清空日志
     *
     * @param type 日志类型
     */
    clearLog(type?: LogType): void;
    /**
     * 获取池的最大worker数量
     */
    getMaxPoolNum(): number;
    /**
     * 获取池的最小worker数量
     */
    getMinPoolNum(): number;
    /**
     * 预热
     *
     * @param workPath 工作路径
     */
    warmUp(workPath: string): boolean;
    /**
     * 一次构建结束后是否常驻
     */
    isResident(): boolean;
    /**
     * 设置是否常驻
     *
     * @param resident
     */
    setResident(resident: boolean): void;
    /**
     * 更新日志级别
     * @param newLogLevel
     * @returns void
     */
    updateLogLevel(newLogLevel: Level): void;
}
