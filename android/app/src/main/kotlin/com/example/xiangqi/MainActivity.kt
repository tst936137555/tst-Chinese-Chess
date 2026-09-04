package com.example.xiangqi

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "xiangqi/engine")
            .setMethodCallHandler { call, result ->
                if (call.method == "getEnginePath") {
                    // 优先 dotprod 版本，其次基础 armv8 版本
                    val dotprod = applicationInfo.nativeLibraryDir + "/libpikafish.so"
                    val fallback = applicationInfo.nativeLibraryDir + "/libpikafish_armv8.so"
                    val path = if (java.io.File(dotprod).exists()) dotprod else fallback
                    result.success(path)
                } else {
                    result.notImplemented()
                }
            }
    }
}
