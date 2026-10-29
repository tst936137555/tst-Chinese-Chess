// 皮卡鱼进程内引擎 C 适配层，供 Android/iOS 的 Rust 引擎会话调用。
// tool/build_mobile_engine.py 构建静态库，Cargo 链入应用。
// 引擎在独立原生线程运行，调用方须安全地把回调投递到会话线程。
#ifndef PIKAFISH_SHIM_H
#define PIKAFISH_SHIM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Reap the previous thread and all callbacks before installing a new receiver.
__attribute__((visibility("default")))
int sf_prepare(void);

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
/// 放弃后再次 sf_start 会返回 -1（调用方按启动失败如实上报）。
__attribute__((visibility("default")))
void sf_stop(void);

/// 释放 on_line 回调传入的缓冲区（引擎输出行）。
__attribute__((visibility("default")))
void sf_free(void* ptr);

#ifdef __cplusplus
}
#endif

#endif  // PIKAFISH_SHIM_H
