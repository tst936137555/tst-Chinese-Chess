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
                    // Pikafish 2026-09-06 起 arm64-universal 单一二进制，
                    // 运行时自适应指令集，无需按 CPU 特性区分变体
                    result.success(applicationInfo.nativeLibraryDir + "/libpikafish.so")
                } else {
                    result.notImplemented()
                }
            }
    }
}
