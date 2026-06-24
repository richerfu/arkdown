/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/task-initializer/project-task-initializer.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as build_info_js_1 from "../../tasks/build-info/build-info";
import * as clean_js_1 from "../../tasks/common/clean";
import * as task_names_js_1 from "../../tasks/common/task-names";
import * as coverage_collect_js_1 from "../../tasks/coverageCollect/coverage-collect";
import * as duplicate_dependency_check_js_1 from "../../tasks/duplicate-dependency-check";
import * as generate_pack_res_js_1 from "../../tasks/generate-pack-res";
import * as assemble_app_js_1 from "../../tasks/hook/assemble/assemble-app";
import * as make_project_pack_info_js_1 from "../../tasks/make-project-pack-info";
import * as process_project_privacy_profile_js_1 from "../../tasks/pac-process/process-project-privacy-profile";
import * as package_app_js_1 from "../../tasks/package-app";
import * as pre_build_app_js_1 from "../../tasks/pre-build-app";
import * as sign_app_js_1 from "../../tasks/sign-app";
import * as sign_packages_from_app_js_1 from "../../tasks/sign-packages-from-app";
import * as sync_project_js_1 from "../../tasks/sync/sync-project";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
var SyncTask = task_names_js_1.TaskNames.SyncTask;
export class ProjectTaskInitializer {
  static initialize(e) {
    const s = process.hrtime(),
      r = e._project,
      a = e.getTaskService();
    r.registry(new assemble_app_js_1.AssembleApp(r, a)), r.registry(new clean_js_1.Clean(r, a)), r.registry(new coverage_collect_js_1.CoverageCollect(r, a)), r.registry(new sync_project_js_1.SyncProject(r, SyncTask.SYNC_TASK)), r.registry(new sign_app_js_1.SignApp(r, a)), r.registry(new package_app_js_1.PackageApp(r, a)), r.registry(new sign_packages_from_app_js_1.SignPackagesFromApp(r, a)), r.registry(new process_project_privacy_profile_js_1.ProcessProjectPrivacyProfile(r, a)), r.registry(new generate_pack_res_js_1.GeneratePackRes(r, a)), r.registry(new pre_build_app_js_1.PreBuildApp(r, a)), r.registry(new duplicate_dependency_check_js_1.DuplicateDependencyCheck(r, a)), r.registry(new make_project_pack_info_js_1.MakeProjectPackInfo(r, a)), r.registry(new build_info_js_1.BuildInfo(r)), this._log.debug(`Project task initialization takes ${(0, hvigor_1.formatTime)(process.hrtime(s))}`);
  }
}
ProjectTaskInitializer._log = ohos_logger_js_1.OhosLogger.getLogger(ProjectTaskInitializer.name);
