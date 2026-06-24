/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/worker/worker.d.ts
 */
import { HvigorNode } from '../../vigor/plugin/interface/hvigor-node';
import { Priority } from '../../internal/pool/enum/priority';
import { TCB } from '../../internal/pool/model/tcb';
/**
 * 向线程池提交一个job并执行,注意任务是挂在task上的，代表这个task并没有结束，但此时主线程可以执行其他任务，依赖此task的任务仍然会排队等待
 * @param node 要挂在什么node的task上
 * @param taskName taskName
 * @param workPath 要执行的job的文件路径,可以拼接函数名称,例如D:/xx/a.js/run是指运行a.js的run方法
 * @param workerOption worker配置
 */
export declare function submitWorker(node: HvigorNode, taskName: string, workPath: string, workerOption: WorkerOption): TCB;
/**
 * workInput 设置要执行的线程的输入值
 * priority 设置线程执行的优先级
 * targetWorkers 可以指定要执行在几号线程中
 * callback 执行完毕 进行的回调
 * callbackInput 回调函数的入参
 * useReturnVal 回调函数入参是否使用线程执行结束的返回值，优先级高于callbackInput
 */
export type WorkerOption = {
    workInput?: unknown;
    priority?: Priority;
    targetWorkers?: number[];
    callback?: Function;
    callbackInput?: unknown[];
    useReturnVal?: boolean;
};
