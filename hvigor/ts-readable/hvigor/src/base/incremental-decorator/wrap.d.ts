/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/wrap.d.ts
 */
import { IncrementalTask } from '../external/task/incremental-task';
/**
 * 新增使用装饰器来声明任务的增量输入/输出机制。
 * 该机制是基于原有的增量机制上进行扩展，所以可以保证两套机制的并存。
 * 该函数主要做了两部分工作：
 * 1.从当前任务开始，基于原型链向上寻找，找出所有使用装饰器来声明的增量输入/输出
 * 2.如果存在对应的增量输入/输出，为了保证这部分输入/输出能够被后续获取，重设该任务的declareInputs, declareInputFiles，declareOutputFiles，
 * @param task
 */
export declare const wrapIncrementalDeclarations: (task: IncrementalTask) => void;
