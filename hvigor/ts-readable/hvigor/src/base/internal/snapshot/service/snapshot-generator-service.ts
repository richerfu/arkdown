/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/service/snapshot-generator-service.js
 */
import * as default_filesystem_snapshot_generator_js_1 from "../generator/default-filesystem-snapshot-generator";
import * as default_task_snapshot_generator_js_1 from "../generator/default-task-snapshot-generator";
import * as default_buildInfo_snapshot_generator_js_1 from "../generator/default-buildInfo-snapshot-generator";
export class SnapshotGeneratorService {
  constructor() {
    this.taskGenerator = new default_task_snapshot_generator_js_1.DefaultTaskSnapshotGenerator(), this.defaultFileSystemSnapshotGenerator = new default_filesystem_snapshot_generator_js_1.DefaultFilesystemSnapshotGenerator(), this.buildInfoGenerator = new default_buildInfo_snapshot_generator_js_1.DefaultBuildInfoSnapshotGenerator();
  }
  static getInstance() {
    return SnapshotGeneratorService._instance || (SnapshotGeneratorService._instance = new SnapshotGeneratorService()), SnapshotGeneratorService._instance;
  }
  generateTaskSnapshot(e, t) {
    return t ? t.snapshot(e) : this.taskGenerator.snapshot(e);
  }
  generateFileSystemSnapshot(e, t, a) {
    return a ? a.snapshot(e, t) : this.defaultFileSystemSnapshotGenerator.snapshot(e, t);
  }
  loadTaskSnapshotFromJson(e) {
    return this.taskGenerator.loadSnapshotFromJson(e);
  }
  loadBuildInfoSnapshotCacheFromJson(e) {
    return this.buildInfoGenerator.loadSnapshotCacheFromJson(e);
  }
  serializeBuildInfoSnapshotCacheToJson() {
    return this.buildInfoGenerator.serializeSnapshotCacheToJson();
  }
  loadTaskSnapshotCacheFromJson(e) {
    return this.taskGenerator.loadSnapshotCacheFromJson(e);
  }
  serializeTaskSnapshotCacheToJson(e) {
    return this.taskGenerator.serializeSnapshotCacheToJson(e);
  }
  loadFileSystemSnapshotFromJson(e) {
    return this.defaultFileSystemSnapshotGenerator.loadSnapshotFromJson(e);
  }
  loadFileSystemSnapshotCacheFromJson(e) {
    return this.defaultFileSystemSnapshotGenerator.loadSnapshotCacheFromJson(e);
  }
  serializeFileSystemSnapshotToJson(e) {
    return this.defaultFileSystemSnapshotGenerator.serializeSnapshotToJson(e);
  }
}
SnapshotGeneratorService._instance = null;
