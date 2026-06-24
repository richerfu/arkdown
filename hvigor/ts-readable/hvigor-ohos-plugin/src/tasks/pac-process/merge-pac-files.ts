/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/merge-pac-files.js
 */
function processDataProcesses(e, s) {
  const a = (e, s) => {
      e.purposes = Array.from(new Set([...e.purposes, ...s.purposes]));
    },
    r = (e, s, a) => {
      undefined === e[a] && undefined === s[a] || (e[a] = !!e[a] || !!s[a]);
    };
  function o(e, s) {
    e.forEach(e => {
      if (s.has(e.label)) {
        const o = s.get(e.label);
        a(o, e), r(o, e, "userLinked"), r(o, e, "tracking"), s.set(e.label, o);
      } else s.set(e.label, e);
    });
  }
  return null == e || e.forEach(e => {
    s.has(e.dataType) ? (e => {
      const a = s.get(e.dataType);
      o(e.dataLabels, a);
    })(e) : (e => {
      const a = new Map();
      o(e.dataLabels, a), s.set(e.dataType, a);
    })(e);
  }), s;
}
function processSpecialAPIs(e, s) {
  return null == e || e.forEach(e => {
    s.has(e.apiType) ? e.reasons.forEach(a => s.get(e.apiType).add(a)) : s.set(e.apiType, new Set(e.reasons));
  }), s;
}
export function mergePacFiles(e) {
  const s = {
      dataProcess: [],
      specialAPIs: []
    },
    a = new Map(),
    r = new Map();
  e.forEach(e => {
    var s, o;
    processDataProcesses(null !== (s = e.dataProcess) && undefined !== s ? s : [], a), processSpecialAPIs(null !== (o = e.specialAPIs) && undefined !== o ? o : [], r);
  });
  for (const [e, r] of a.entries()) {
    const a = [];
    for (const [e, s] of r.entries()) {
      const r = {
        label: e,
        purposes: Array.from(s.purposes)
      };
      undefined !== s.tracking && (r.tracking = s.tracking), undefined !== s.userLinked && (r.userLinked = s.userLinked), a.push(r);
    }
    const o = {
      dataType: e,
      dataLabels: a
    };
    s.dataProcess.push(o);
  }
  for (const [e, a] of r.entries()) {
    const r = {
      apiType: e,
      reasons: Array.from(a)
    };
    s.specialAPIs.push(r);
  }
  return s;
}
