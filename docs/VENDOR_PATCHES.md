**随库第三方补丁维护**

`Cargo.toml` 的 `[patch.crates-io]` 固定使用 `vendor/miniquad` 和 `vendor/quad-snd`。升级依赖时应保留明确补丁清单，并在干净检出上重跑对应回归。

| 组件 | 项目需要的行为 | 验证入口 |
| --- | --- | --- |
| miniquad | 字体图集及渲染目标释放后维持正确 GL 绑定；EGL 无多重采样配置时降级单采样；Android 无 Surface 时清除更新标记并等待事件；Activity 重建沿用单个原生运行线程并按消息顺序更换 JNI 引用；阻塞循环绘制前处理待投递事件，合并定时唤醒，避免慢帧积压导致输入饥饿 | 字体密度循环、29 场景视觉回归、Windows/Android/iOS 输入、Activity 重建与启动工作流 |
| quad-snd | 双声道循环跨混音缓冲边界连续；空循环不会挂住；重叠提示限幅 | `cargo test --manifest-path vendor/quad-snd/Cargo.toml --lib`、应用音频单元测试 |

升级时记录上游版本/提交、保留或删除的本地改动及理由。重新生成 Android Java 桥接后必须对实际生成代码执行编译检查，模板插入点变化应令检查失败。目标平台的链接与设备运行需要分别验收。

依赖版本由 Cargo.lock 固定；当前修复没有以未经验证的大范围依赖升级代替缺陷修复。安全公告扫描与许可证清单核对可单独运行并保留日期、工具版本和结果，不能把未运行的检查写成通过。


v2.1.0 miniquad 补丁补充：Android 跨线程调用通过锁内 NewLocalRef 取得独立引用，替换全局引用时不影响读者；Activity 代际用于重新发布读屏/输入框/亮屏状态；初始 Surface 等待改为 recv，避免忙等。Windows framebuffer 直接从客户区物理像素计算，避免 225% 等小数 DPI 的整数逻辑尺寸往返。应用层不再直接读取 Android ACTIVITY 裸指针。验证包括 Android 目标编译、CheckJNI 三次重建、Windows 小数 DPI 单测与视觉基线。

`tool/dependency_inventory.py` 生成所有锁定依赖的许可证声明、仓库来源及两份 vendor 目录的受版本控制文件摘要（换行统一为 LF）。maintenance 工作流每周保留扫描与清单；声明清单本身不等同于完整法律合规结论。
