/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/process/daemon-process-boot-script.js
 */
import cluster_1 from "cluster";
if (cluster_1.isPrimary) {
  const {
    masterProcessBoot: e
  } = require("../cluster/master-process-boot.js");
  e();
  const {
    startJavaDaemon: r
  } = require("../java/hvigor-java-daemon.js");
  r();
} else {
  const {
    registryWorkerProcessListener: e
  } = require("../cluster/worker-process-lifecycle.js");
  e();
}
