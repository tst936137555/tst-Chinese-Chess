**正式发布与签名**

版本以 `Cargo.toml` 为准，移动端 versionCode 按主版本×10000＋次版本×100＋修订号计算。正式 tag 必须是对应的 `v主.次.修订`；更新记录必须含有同名版本段落。

更新版本号与对应更新记录后推送到 `main`（兼容 `master`），完整 CI 成功即自动发布该版本，无需手动打 tag。发布使用同一次 CI 的正式安装包，并将 `v版本号` tag 固定到通过检查的提交。手动触发主分支 CI 或推送对应版本 tag 也可以补发；PR 不发布。已发布的正式版本会跳过，普通提交不会覆盖旧发行版。新主分支提交不会取消正在运行的版本 CI，发布任务串行执行。

发布前再次验证完整资产、Android 实际签名和 Manifest；上传期间保留草稿，所有资产上传成功后才公开为正式版。中断后可重跑同一 CI，继续同一提交的草稿；未发布 tag 指向其他提交时会失败，禁止移动版本 tag 或混用产物。

项目正式 Android 证书的 SHA256 指纹保存在 `tool/android_signing.json`。其中只有公开证书信息。构建时签名、构建后校验与发布前复验均使用该指纹，禁止意外更换证书或把调试包当作发布包。

本机私有配置默认位于 `~/.tst-xiangqi/signing/key.properties`，也可通过 `ANDROID_SIGNING_PROPERTIES` 指定。密钥路径相对于该配置文件解析。字段为 `storeFile`、`keyAlias`、`storePassword`、`keyPassword`。也可以直接设置 `ANDROID_KEYSTORE`、`ANDROID_KEY_ALIAS`、`ANDROID_KEYSTORE_PASSWORD`、`ANDROID_KEY_PASSWORD`，环境变量优先。配置与密钥不得放入源码、发布源码副本或日志。本次提供的签名已保存在仓库外并限制为当前用户访问。

```powershell
python tool/build_android_rust.py aarch64-linux-android
python tool/build_android_rust.py x86_64-linux-android
```

构建需要设置 SDK、NDK 与 JAVA_HOME，具体依赖见 README。缺少正式签名配置会立即失败。`--verify-only` 只生成 `target/android-verification/` 下的内部未签名验证包，不会写入 `dist/`，不能作为正式安装包发布。

仓库 Actions 使用四个 Secrets：`ANDROID_KEYSTORE_BASE64`、`ANDROID_KEY_ALIAS`、`ANDROID_KEYSTORE_PASSWORD`、`ANDROID_KEY_PASSWORD`。受信任分支和 tag 构建使用这些密钥；外部 fork PR 只进行未签名编译验证，不能取得签名密钥或上传正式 APK。

已有本机 GitHub 仓库管理凭据时，可显式运行以下命令配置签名 Secrets；该工具使用 GitHub 提供的公钥加密后再上传，不输出密钥或密码。可选依赖为 PyNaCl。

```powershell
python tool/configure_android_signing.py --github
```

每个正式 APK 配有同名 `.build.json`，记录实际证书、包名、版本、非调试属性、包 SHA256、引擎摘要与构建来源。正式 Release 先检查完整资产清单，再用 Android SDK 工具重新验证实际签名和 Manifest，最后生成 SHA256SUMS 并发布为正式版。Release 不设置 draft 或 prerelease。

正式资产目前包含 Windows 便携 ZIP、arm64-v8a APK 和 x86_64 APK。iOS 真机未签名 `.app` 和模拟器 ZIP 仅用于 CI 构建/运行验证，作为 `verification-ios-*` artifact 保留，不会混入正式 Release。增加正式 iOS 发行前，需要对应 Apple 签名和设备验收。

Android 的内部回归使用 `getExternalFilesDir()` 下的 smoke/interaction 目录，测试棋谱与正常用户的私有存档分离；读取测试结果无需 `run-as`，APK 的 debuggable 始终为 false。自动安装与输入回归只允许 Android 模拟器。测试发布升级时，应使用相同包名和相同正式证书，验证已有棋谱、设置和续局。不要卸载含数据的旧应用来绕过签名不匹配。
