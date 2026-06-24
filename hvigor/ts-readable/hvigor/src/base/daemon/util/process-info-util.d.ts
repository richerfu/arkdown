/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/util/process-info-util.d.ts
 */
import { DaemonInfo } from '../registry/daemon-info';
/**
 * 检查目标进程是否为hvigor守护进程的方法
 * 该方法使用时间约在300ms左右，因此暂时只在时间不敏感的stop-daemon中使用
 *
 * @param {number} pid
 * @returns {boolean}
 */
export declare function isHvigorDaemonProcess(pid: number): boolean;
/**
 * 检查daemon进程是否真的存活
 * @param daemonInfo 要检查的daemon信息
 * @returns 如果进程存活返回true，否则返回false
 */
export declare function isDaemonProcessAlive(daemonInfo: DaemonInfo): boolean;
/**
 * 通过--stop-daemon命令,关闭所有daemon注册表中的守护进程
 * 在client侧被调用,打印日志使用daemon-client
 */
export declare function stopDaemons(closeAll: boolean): void;
/**
 * 通过--status-daemon命令,打印所有daemon注册表中的守护进程信息
 * 在client侧被调用,打印日志使用daemon-client
 */
export declare function logDaemonInfo(): void;
