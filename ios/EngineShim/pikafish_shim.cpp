// 皮卡鱼进程内引擎 C 适配层（iOS 专用）——实现。
//
// 设计（详见项目 iOS FFI 可行性评估）：
// - 引擎以 -DUNIVERSAL_BINARY 编译为静态库：原 main() 收编为 Stockfish::main，
//   由本适配层在独立原生线程上驱动，引擎源码零改动。
// - std::cin/std::cout 重定向为行队列：Dart 经 sf_send 注入输入行，
//   引擎输出行攒齐一行（std::endl → flush → sync）后经 on_line 回调投递。
// - 线程安全：引擎经 sync_cout 串行化输出，适配层以互斥量兜底；
//   输入队列以条件变量阻塞 getline（引擎空闲时挂起，无忙等）。
// - 全局状态（攻击表/置换表/线程池）跨 sf_start 保留：Stockfish::main 每轮
//   自行重新初始化，Dart 侧每轮重发 setoption，与重启子进程等价。
// - 生命周期：sf_stop 注入 quit 优雅收线并最多等 2s；真挂死的线程无法强杀
//   （POSIX 无安全终止线程方案），只能放弃并在下次 sf_start 报 -1——失去
//   进程隔离是本方案的已知代价（可行性评估风险 #1）。
//
// 编译约束：与引擎一致的 -fno-exceptions（禁用 try/catch/throw），
// 且挂接于引擎构建体系（-Wall -Wextra -pedantic -Wshadow 等）。
#include "pikafish_shim.h"

#include <chrono>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>

namespace Stockfish {
// -DUNIVERSAL_BINARY 编译时由 src/main.cpp 提供（原 main 收编于此）
int main(int argc, char* argv[]);
}

namespace {

std::mutex g_mutex;
std::condition_variable g_cv;
std::deque<std::string> g_in;  // 待引擎读取的输入行（UCI 命令）
bool g_eof = false;            // 引擎输入流已结束（线程退出后置位）
bool g_running = false;
bool g_finished = false;       // 引擎线程已结束
void (*g_on_line)(const uint8_t*) = nullptr;
void (*g_on_exit)(int32_t) = nullptr;
std::thread g_thread;

/// 投递一行到 Dart（malloc 分配，Dart 复制后经 sf_free 释放）
void deliver(const std::string& line) {
  void (*cb)(const uint8_t*) = nullptr;
  {
    std::lock_guard<std::mutex> lock(g_mutex);
    cb = g_on_line;
  }
  if (cb == nullptr) return;
  uint8_t* copy = static_cast<uint8_t*>(std::malloc(line.size() + 1));
  if (copy == nullptr) return;  // 丢行：调用方以握手超时/看门狗兜底
  std::memcpy(copy, line.data(), line.size());
  copy[line.size()] = 0;
  cb(copy);
}

/// std::cin 替身：从队列供行，空队列时阻塞（等价管道读端）
class InBuffer : public std::streambuf {
 protected:
  int_type underflow() override {
    std::unique_lock<std::mutex> lock(g_mutex);
    g_cv.wait(lock, [] { return !g_in.empty() || g_eof; });
    if (g_in.empty()) return traits_type::eof();
    current_ = std::move(g_in.front());
    g_in.pop_front();
    current_.push_back('\n');
    char* data = &current_[0];
    setg(data, data, data + current_.size());
    return traits_type::to_int_type(current_[0]);
  }

 private:
  std::string current_;  // 供行缓冲（setg 指针必须指向存活内存）
};

/// std::cout 替身：攒行，flush（std::endl）时经回调投递
class OutBuffer : public std::streambuf {
 protected:
  int_type overflow(int_type ch) override {
    if (traits_type::not_eof(ch)) {
      std::lock_guard<std::mutex> lock(g_mutex);
      buf_.push_back(traits_type::to_char_type(ch));
    }
    return traits_type::not_eof(ch);
  }

  std::streamsize xsputn(const char* s, std::streamsize n) override {
    std::lock_guard<std::mutex> lock(g_mutex);
    buf_.append(s, static_cast<size_t>(n));
    return n;
  }

  int sync() override {
    std::string line;
    {
      std::lock_guard<std::mutex> lock(g_mutex);
      line = std::move(buf_);
      buf_.clear();
    }
    // 引擎以 sync_endl 收尾：剥掉行尾换行，保持与 Process.stdout 行协议一致
    while (!line.empty() && (line.back() == '\n' || line.back() == '\r')) {
      line.pop_back();
    }
    if (!line.empty()) deliver(line);
    return 0;
  }

 private:
  std::string buf_;
};

void engineThreadMain() {
  char arg0[] = "pikafish";
  char* argv[] = {arg0, nullptr};
  int code = Stockfish::main(1, argv);

  void (*cb)(int32_t) = nullptr;
  {
    std::lock_guard<std::mutex> lock(g_mutex);
    g_running = false;
    g_finished = true;
    g_eof = true;
    cb = g_on_exit;
  }
  g_cv.notify_all();
  if (cb != nullptr) cb(static_cast<int32_t>(code));
}

}  // namespace

extern "C" {

__attribute__((visibility("default")))
int sf_start(void (*on_line)(const uint8_t*), void (*on_exit)(int32_t)) {
  std::lock_guard<std::mutex> lock(g_mutex);
  // 回收上一轮线程：正常结束后仍 joinable，直到本次回收；未结束（挂死）则拒绝
  if (g_thread.joinable()) {
    if (!g_finished) return -1;
    g_thread.join();
  }

  static InBuffer in_buf;
  static OutBuffer out_buf;
  static bool installed = false;
  if (!installed) {
    std::cin.rdbuf(&in_buf);
    std::cout.rdbuf(&out_buf);
    installed = true;
  }

  g_in.clear();
  g_eof = false;
  g_finished = false;
  g_on_line = on_line;
  g_on_exit = on_exit;
  g_running = true;
  g_thread = std::thread(engineThreadMain);
  return 0;
}

__attribute__((visibility("default")))
int sf_send(const uint8_t* line) {
  if (line == nullptr) return -1;
  std::lock_guard<std::mutex> lock(g_mutex);
  if (!g_running) return -1;
  g_in.emplace_back(reinterpret_cast<const char*>(line));
  g_cv.notify_all();
  return 0;
}

__attribute__((visibility("default")))
void sf_stop(void) {
  std::thread finished;
  {
    std::unique_lock<std::mutex> lock(g_mutex);
    if (!g_running) {
      if (g_thread.joinable()) g_thread.join();
      return;
    }
    // 优雅收线：UCI 'quit' 令主循环返回（搜索中引擎会先停止搜索再退出）
    g_in.emplace_back("quit");
    g_cv.notify_all();
    // 最多等 2s；真挂死则放弃（线程留待下次 sf_start 回收或报错）
    g_cv.wait_for(lock, std::chrono::milliseconds(2000),
                  [] { return g_finished; });
    if (g_finished && g_thread.joinable()) {
      finished = std::move(g_thread);
    }
  }
  if (finished.joinable()) finished.join();
}

__attribute__((visibility("default")))
void sf_free(void* ptr) {
  std::free(ptr);
}

}  // extern "C"
