#include "../../native/pikafish/pikafish_shim.h"
#include <cassert>
#include <atomic>
#include <thread>
#include <chrono>
#include <condition_variable>
#include <iostream>
#include <mutex>
#include <string>

std::atomic<bool> delay_exit{false}, release_exit{false};
std::atomic<int> output_sender{0};
// A deterministic engine entry point, exercising the production C adapter.
namespace Stockfish {
int main(int, char**) {
  std::string line;
  while (std::getline(std::cin, line)) {
    if (line == "quit") {
      while (delay_exit && !release_exit) std::this_thread::sleep_for(std::chrono::milliseconds(1));
      break;
    }
    if (line == "uci") std::cout << "uciok" << std::endl;
  }
  std::cin.setstate(std::ios::eofbit | std::ios::failbit);
  return 0;
}
}
std::mutex mutex;
std::condition_variable ready;
int replies = 0;
void on_line(const uint8_t* line) {
  const std::string copy(reinterpret_cast<const char*>(line));
  sf_free(const_cast<uint8_t*>(line));
  if (copy == "uciok") {
    std::lock_guard<std::mutex> lock(mutex);
    ++replies;
    ready.notify_all();
  }
}
void on_exit(int32_t) { output_sender = 0; }
int main() {
  for (int i = 1; i <= 3; ++i) {
    assert(sf_prepare() == 0);
    output_sender = i;
    assert(sf_start(on_line, on_exit) == 0);
    assert(sf_send(reinterpret_cast<const uint8_t*>("uci")) == 0);
    {
      std::unique_lock<std::mutex> lock(mutex);
      assert(ready.wait_for(lock, std::chrono::seconds(2), [i] { return replies == i; }));
    }
    sf_stop();
  }
  delay_exit = true;
  assert(sf_prepare() == 0);
  output_sender = 4;
  assert(sf_start(on_line, on_exit) == 0);
  sf_stop();
  assert(sf_prepare() == -1);
  release_exit = true;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
  while (sf_prepare() != 0) {
    assert(std::chrono::steady_clock::now() < deadline);
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  assert(output_sender == 0);
  output_sender = 5;
  assert(sf_start(on_line, on_exit) == 0);
  assert(output_sender == 5);
  assert(sf_send(reinterpret_cast<const uint8_t*>("uci")) == 0);
  { std::unique_lock<std::mutex> lock(mutex); assert(ready.wait_for(lock, std::chrono::seconds(2), [] { return replies == 4; })); }
  sf_stop();
  std::cerr << "C ABI restart / EOF / late exit regression passed (5 sessions)\n";
}
