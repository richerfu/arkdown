/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/build-task-graph.d.ts
 */
import { Project } from '../../external/core/hvigor-core-node';
/**
 * 构建每个hvigor module的Task DAG图
 *
 * @param {Project} project
 */
export declare function buildTaskGraph(project: Project): void;
