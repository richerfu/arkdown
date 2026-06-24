/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-resource-task.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { RestoolCommandBuilder } from '../../builder/restool-command-builder';
import { RestoolConfigBuilder } from '../../builder/restool-file-config-builder';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
export declare abstract class AbstractResource extends OhosHapTask {
    restoolConfigBuilder: RestoolConfigBuilder;
    restoolCommandBuilder?: RestoolCommandBuilder;
    protected outputDir: string;
    protected constructor(taskService: TargetTaskService, taskDetails: TaskDetails);
    /**
     * get common restool config builder
     *
     * @returns {RestoolBuilderInterface} resTool命令构造器
     */
    protected getRestoolConfigBuilder(): RestoolConfigBuilder;
    /**
     * get common restool command builder
     *
     * @returns {RestoolBuilderInterface} resTool编译、链接命令构造器
     * @protected
     */
    protected initConfigTargetResource(): RestoolConfigBuilder;
    protected getRestoolCommandBuilder(): RestoolCommandBuilder;
    protected initCommandTargetResource(): RestoolCommandBuilder;
}
