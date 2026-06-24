/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/time-util.d.ts
 */
export declare function splitTime(source: string): string;
/**
 * 将[Number, Number]转化为ns格式
 *
 * @param time 待处理的时间
 */
export declare function formatTimeToNumber(time: [number, number] | undefined): number;
/**
 * 将ns转化为[Number, Number]格式   第一个Number的单位是s  第二个Number的单位是ns
 *
 * @param time 待处理的时间
 */
export declare function formatTimeToNumPair(time: number): [number, number];
/**
 * 获取worker任务的最短交集时间
 *
 * @param intervals
 */
export declare function getIntersect(intervals: [number, number][]): number;
/**
 * 对构建显示时间进行格式化处理
 *
 * @param time 待处理的时间
 */
export declare function formatTime(time: [number, number]): string;
/**
 * 判断日志是否是显示时间的Hvigor日志
 *
 * @param log 待判断的日志内容
 */
export declare function isHvigorLogWithTime(log: string): boolean;
