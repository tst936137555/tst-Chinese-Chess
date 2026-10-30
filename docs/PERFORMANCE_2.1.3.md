# v2.1.3 性能验证（2026-10-30）

本轮保持五档难度的深度、步时、MultiPV、容错、选招逻辑及提示/复盘分析参数不变。优化集中在音频解码、动画调度、GPU 采样和无障碍数据复制。

## 方法与范围

Windows，Intel Core i5-10210U / Intel UHD Graphics，Rust 1.99.1 GNU release 构建。固定 1100×800 逻辑窗口、DPI 1；普通画布 1920×1397，4K 画布 3840×2160。每个场景独立启动一次，串行运行 10 秒，排除前 3 秒，保留剩余逐帧间隔。对照是优化前的 7053e80 加入同一动画采样夹具，优化后使用相同夹具。

本机 OpenGL 为 `3.1.0 - Build 27.20.100.9316`，验证的是无 GPU 阻塞等待的 CPU 反馈分支。OpenGL 3.3+ 的异步查询另由 Linux CI 连续动画场景验证。这里测量应用帧间隔，不能当作显示器实际呈现延迟、引擎棋力或手机耗电数据。

## 结果

| 场景 | 版本 | 样本帧数 | 中位帧间隔 | P95 帧间隔 | 超过 25ms 的帧 |
| --- | --- | ---: | ---: | ---: | ---: |
| 普通动画 | 优化前 | 349 | 17.373ms | 32.102ms | 22.3% |
| 普通动画 | 优化后 | 421 | 16.603ms | 19.281ms | 0.2% |
| 4K 动画 | 优化前 | 314 | 18.929ms | 33.143ms | 35.0% |
| 4K 动画 | 优化后 | 421 | 16.632ms | 18.400ms | 0.0% |

这是单机、每场景一次采样；数值会受系统负载影响。冷启动字形准备、首次画布切换和移动设备热稳定性不在该表的结论范围内，不能据此声称所有卡顿均已消除。原始结果位于本次工作目录的 `target/perf-{before,after}-animation{,-4k}/result.json`。

## 复现

```powershell
cargo +1.99.1-x86_64-pc-windows-gnu build --release --locked --bin tst-xiangqi --example profile_ui
# 七个场景串行采样，使用隔离存档目录：
cargo +1.99.1-x86_64-pc-windows-gnu run --release --locked --example profile_ui -- target/ui-profile
```

也可单独运行 `tst-xiangqi.exe --profile-ui <输出目录> --profile-scene animation --profile-seconds 10`，将场景改为 `animation-4k` 可测量 4K。原始间隔在 `animationFrameMsAfterWarmup`；排序后取索引 `floor(n * 0.95)` 为表中 P95。

本地回归包括 140 项 Rust 测试（要求真实引擎可用）、5 项混音/解码线程测试、39 项脚本测试、严格 Clippy、格式检查、29 场景界面冒烟及视觉基线，以及真实 Windows 鼠标/键盘、UIA、中文提交与冷启动续局。正式发布还需完整跨平台 CI 成功。
