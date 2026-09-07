package com.tst.xiangqi

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
                } else {
                    result.notImplemented()
                }
            }
    }
}
