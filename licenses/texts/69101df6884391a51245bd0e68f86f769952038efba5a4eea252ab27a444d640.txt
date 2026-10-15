# Third-party notices

The files under `src/assets/` are development-only signing materials derived from:

- OpenHarmony `developtools/hapsigner/dist/OpenHarmony.p12`
- OpenHarmony `developtools/hapsigner/dist/OpenHarmonyApplication.pem`
- OpenHarmony `developtools/hapsigner/dist/OpenHarmonyProfileDebug.pem`

Source project: <https://gitee.com/openharmony/developtools_hapsigner>

Copyright (c) Huawei Device Co., Ltd. and OpenHarmony contributors.
Licensed under the Apache License, Version 2.0.

These are publicly distributed test credentials. They provide no private identity
or production trust and must only be used for local OpenHarmony development/QEMU.

The signing-block, chunk-digest, CMS, ZIP-alignment, page-info, and code-sign
format implementation follows the same Apache-2.0 OpenHarmony source project
and `security_appverify`:

- <https://gitee.com/openharmony/developtools_hapsigner>
- <https://gitee.com/openharmony/security_appverify>
