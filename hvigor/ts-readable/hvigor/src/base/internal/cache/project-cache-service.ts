/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/project-cache-service.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as const_js_1 from "../../../common/const/const";
import * as local_file_writer_js_1 from "../../../common/util/local-file-writer";
import * as path_util_js_1 from "../../../common/util/path-util";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as meta_util_js_1 from "../../util/meta-util";
import * as global_core_parameters_js_1 from "../data/global-core-parameters";
import * as snapshot_generator_service_js_1 from "../snapshot/service/snapshot-generator-service";
import * as task_proxy_manager_js_1 from "../task/proxy/task-proxy-manager";
import * as filesnapshot_cache_service_js_1 from "./service/filesnapshot-cache-service";
import * as tasksnapshot_cache_service_js_1 from "./service/tasksnapshot-cache-service";
import * as cache_util_js_1 from "./util/cache-util";
import * as buildInfo_cache_service_js_1 from "./service/buildInfo-cache-service";
const logger = hvigor_log_js_1.HvigorLogger.getLogger("project-cache-service");
export class ProjectCacheService {
  constructor(e) {
    this.projectName = e.getName(), this.projectNodeDir = e.getNodeDir(), this.projectTaskSnapShotCacheFilePath = this.getProjectTaskSnapShotCacheFilePath(), this.projectFileSnapShotCacheFilePath = this.getProjectFileSnapShotCacheFilePath(), this.projectBuildInfoCacheFilePath = this.getProjectBuildInfoCacheFilePath(), this.taskSnapShotCacheService = new tasksnapshot_cache_service_js_1.TaskSnapShotCacheService(this.projectTaskSnapShotCacheFilePath), this.fileSnapShotCacheService = new filesnapshot_cache_service_js_1.FileSnapShotCacheService(this.projectFileSnapShotCacheFilePath), this.buildInfoCacheService = new buildInfo_cache_service_js_1.BuildInfoCacheService(this.projectBuildInfoCacheFilePath), this.currentTaskSnapShotCacheService = new tasksnapshot_cache_service_js_1.TaskSnapShotCacheService(this.projectTaskSnapShotCacheFilePath), this.currentFileSnapShotCacheService = new filesnapshot_cache_service_js_1.FileSnapShotCacheService(this.projectFileSnapShotCacheFilePath);
  }
  getProjectFileSnapShotCacheFilePath() {
    return path_1.resolve(path_util_js_1.PathUtil.getHvigorCacheDir(logger), "./cache/file-cache.json");
  }
  getProjectTaskSnapShotCacheFilePath() {
    return path_1.resolve(path_util_js_1.PathUtil.getHvigorCacheDir(logger), "./cache/task-cache.json");
  }
  getProjectBuildInfoCacheFilePath() {
    return path_1.resolve(path_util_js_1.PathUtil.getHvigorCacheDir(), "./cache/last-build-info.json");
  }
  initialize() {
    this.checkMetaInfo(), this.taskSnapShotCacheService.initialize(), this.fileSnapShotCacheService.initialize(), this.currentTaskSnapShotCacheService.clone(this.taskSnapShotCacheService), this.currentFileSnapShotCacheService.clone(this.fileSnapShotCacheService), this.buildInfoCacheService.initialize();
  }
  getTaskSnapShot(e, t) {
    return this.taskSnapShotCacheService.get((0, cache_util_js_1.getTaskSnapCacheEntryUniqueKey)(e, t));
  }
  getFileSnapShot(e) {
    return this.fileSnapShotCacheService.get(e);
  }
  updateFileSnapShot(e, t) {
    this.currentFileSnapShotCacheService.set(e, t);
  }
  updateTaskSnapShot(e, t) {
    this.currentTaskSnapShotCacheService.set(e, t);
  }
  getBuildInfo() {
    return this.buildInfoCacheService.getCacheEntryContentMap();
  }
  close() {
    if (ProjectCacheService.instance = undefined, global_core_parameters_js_1.coreParameter.startParams.incrementalExecution) try {
      task_proxy_manager_js_1.taskProxyManager.getAllIncrementalTaskProxy().forEach(e => {
        e.postExecute();
      }), task_proxy_manager_js_1.taskProxyManager.reset();
      const e = snapshot_generator_service_js_1.SnapshotGeneratorService.getInstance().serializeTaskSnapshotCacheToJson(this.currentTaskSnapShotCacheService.getCacheEntryContentMap()),
        t = snapshot_generator_service_js_1.SnapshotGeneratorService.getInstance().serializeFileSystemSnapshotToJson(this.currentFileSnapShotCacheService.getCacheEntryContentMap()),
        a = snapshot_generator_service_js_1.SnapshotGeneratorService.getInstance().serializeBuildInfoSnapshotCacheToJson();
      local_file_writer_js_1.LocalFileWriter.getInstance().writeStr(this.projectTaskSnapShotCacheFilePath, e), local_file_writer_js_1.LocalFileWriter.getInstance().writeStr(this.projectFileSnapShotCacheFilePath, t), local_file_writer_js_1.LocalFileWriter.getInstance().writeStr(this.projectBuildInfoCacheFilePath, a);
    } catch (e) {}
  }
  static getInstance(e) {
    return ProjectCacheService.instance || (ProjectCacheService.instance = new ProjectCacheService(e)), ProjectCacheService.instance;
  }
  checkMetaInfo() {
    const e = (0, meta_util_js_1.getLastHvigorVersion)();
    e !== const_js_1.CUR_HVIGOR_VERSION && (logger.debug(`Since current hvigor version ${const_js_1.CUR_HVIGOR_VERSION} differs from last hvigor version \n      ${e}, delete file-cache.json and task-cache.json.`), fs_1.existsSync(this.projectTaskSnapShotCacheFilePath) && fs_1.rmSync(this.projectTaskSnapShotCacheFilePath), fs_1.existsSync(this.projectFileSnapShotCacheFilePath) && fs_1.rmSync(this.projectFileSnapShotCacheFilePath));
  }
}
