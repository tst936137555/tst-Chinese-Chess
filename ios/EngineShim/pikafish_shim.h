// 皮卡鱼进程内引擎 C 适配层（iOS 专用）—— 对 Dart 侧 lib/engine/pikafish.dart 的 _FfiIo。
//
// 背景：iOS 沙盒禁止派生子进程，引擎无法像 Android/macOS/Windows 那样以
// 子进程方式运行。本适配层把引擎静态库（tool/setup_ios_engine.sh 构建并
// 链入 Runner 主可执行文件）包装成三个 C 函数，UCI 行协议语义与子进程模式
// 完全一致，Dart 侧握手/看门狗/MultiPV 解析等逻辑全部复用。
//
// 符号导出：默认可见性（显式 attribute），Dart 侧以 DynamicLibrary.executable()
// 解析（静态链接官方推荐方式，需配合 -Wl,-force_load 防止链接器裁剪）。
//
// 线程模型：引擎在独立原生线程运行（sf_start 启动）；on_line/on_exit 回调
// 可能来自该线程，Dart 侧必须使用 NativeCallable.listener（跨线程投递）。
#ifndef PIKAFISH_SHIM_H
#define PIKAFISH_SHIM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// 启动引擎线程（运行 Stockfish::main，即原 main 收编版本）。
/// on_line：引擎 stdout 行回调（UTF-8，malloc 分配，调用方用完须 sf_free）；
/// on_exit：引擎线程结束回调（code = main 返回值；'quit' 后正常为 0）。
/// 返回 0 成功；-1 表示引擎已在运行（含上一轮挂死未回收）。
__attribute__((visibility("default")))
int sf_start(void (*on_line)(const uint8_t* line),
             void (*on_exit)(int32_t code));

/// 向引擎输入流注入一行（等同写 stdin；无需带换行）。
/// 返回 0 成功；-1 表示引擎未运行或参数非法。
__attribute__((visibility("default")))
int sf_send(const uint8_t* line);

/// 优雅停止：注入 UCI 'quit' 令引擎主循环返回，最多等待 2s。
/// 真挂死的引擎线程无法强杀（POSIX 不支持安全终止线程），只能放弃；
/// 放弃后再次 sf_start 会返回 -1（Dart 侧按启动失败如实上报）。
__attribute__((visibility("default")))
void sf_stop(void);

/// 释放 on_line 回调传入的缓冲区（引擎输出行）。
__attribute__((visibility("default")))
void sf_free(void* ptr);

#ifdef __cplusplus
}
#endif

#endif  // PIKAFISH_SHIM_H
