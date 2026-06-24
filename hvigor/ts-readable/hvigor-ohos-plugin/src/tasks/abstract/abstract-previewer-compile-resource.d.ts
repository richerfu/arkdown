/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-previewer-compile-resource.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { RestoolLinkParamObj } from '../../options/configure/restool-config-options';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { TargetTaskService } from '../service/target-task-service';
import { AbstractCompileResource } from './abstract-compile-resource';
/**
 * 预览资源编译任务的公共抽象类
 *
 * @since 2022/9/8
 */
export declare abstract class AbstractPreviewerCompileResource extends AbstractCompileResource {
    protected compileCommands: string[][];
    protected linkCommand: string[] | undefined;
    protected linkOutputPath: string | undefined;
    protected restoolLinkParamObj: RestoolLinkParamObj | undefined;
    protected constructor(taskService: TargetTaskService, taskDetails: TaskDetails);
    /**
     * 以命令为参数执行编译、链接resTool命令
     *
     * @param {OhosLogger} log 日志对象
     * @param {Function} callback 编译资源完成后的回调函数
     * @protected
     */
    protected compileLink(log: OhosLogger, callback?: Function): Promise<void>;
    /**
     * 执行previewer资源编译、链接
     *
     * @param {OhosLogger} log 日志对象
     * @param {Function} callback 回调函数
     */
    previewCompileLink(log: OhosLogger, callback?: Function): Promise<void>;
    /**
     * Preview 构建参数持久化
     * （预览热加载时使用此参数）
     */
    previewBuildParamPersistence(): Promise<void>;
    /**
     * 构建增量特性编译链接完整命令
     *
     * @protected
     */
    protected buildIncrementalCompileLinkFullCommand(): Promise<void>;
    /**
     * 链接（link）命令添加ResourcesMap转换的命令
     *
     * @protected
     */
    protected linkCommandPushResourcesMap(resourcesMap: Map<string, string>): void;
}
