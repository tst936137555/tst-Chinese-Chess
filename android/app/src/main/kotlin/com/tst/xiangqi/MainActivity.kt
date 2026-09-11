package com.tst.xiangqi

import java.io.File
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "tst_xiangqi/engine")
            .setMethodCallHandler { call, result ->
                if (call.method == "getEnginePath") {
                    // 按安装 ABI 解析：arm64-v8a = 官方 arm64-universal 预编译版（真机），
                    // x86_64 = NDK 交叉编译版（模拟器，官方无 x86_64 Android 发行版）
                    result.success(applicationInfo.nativeLibraryDir + "/libpikafish.so")
                } else if (call.method == "getNnuePath") {
                    try {
                        result.success(copyNnueToFile())
                    } catch (e: Exception) {
                        result.error("NNUE_COPY_FAILED", e.message, null)
                    }
                } else {
                    result.notImplemented()
                }
            }
    }

    /**
     * 将 APK 内 NNUE 资产流式复制到应用私有目录（64KB 分段，避免 ~40MB 整块载入内存）。
     *
     * 文件名携带 lastUpdateTime 指纹：同一次安装已落盘即直接复用，
     * 应用升级（NNUE 随包更新）后指纹变化必然重新复制；先写 .part 再改名，
     * 复制中断不会残留同名半截文件被误复用。旧指纹残留尽力清理。
     */
    @Suppress("DEPRECATION")
    private fun copyNnueToFile(): String {
        val stamp = packageManager.getPackageInfo(packageName, 0).lastUpdateTime
        val target = File(filesDir, "pikafish-$stamp.nnue")
        if (!target.exists()) {
            val part = File(filesDir, "pikafish-$stamp.nnue.part")
            assets.open("flutter_assets/assets/engine/pikafish.nnue").use { input ->
                part.outputStream().use { output ->
                    input.copyTo(output, 64 * 1024)
                }
            }
            if (!part.renameTo(target)) {
                part.delete()
                throw IllegalStateException("NNUE 复制后改名失败")
            }
            filesDir.listFiles()?.forEach { f ->
                val n = f.name
                if (n.startsWith("pikafish-") &&
                    (n.endsWith(".nnue") || n.endsWith(".nnue.part")) &&
                    n != target.name
                ) {
                    f.delete()
                }
            }
        }
        return target.absolutePath
    }
}
