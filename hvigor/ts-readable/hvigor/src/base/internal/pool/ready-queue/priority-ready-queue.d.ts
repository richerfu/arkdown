/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/ready-queue/priority-ready-queue.d.ts
 */
import { Work } from '../model/work';
import { ReadyQueue } from './ready-queue';
/**
 * ReadyQueue的实现类
 *
 * @since 2022/8/12
 */
export declare class PriorityReadyQueue implements ReadyQueue {
    private heap;
    constructor();
    clear(): void;
    pop(): Work | undefined;
    push(work: Work): void;
    update(work: Work): void;
    empty(): boolean;
    /**
     * 判断队列中是否存在FIRST优先级别的工作单元
     *
     * @returns {boolean} 判断结果
     */
    private hasFirstWork;
}
