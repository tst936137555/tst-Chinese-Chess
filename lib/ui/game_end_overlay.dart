/// 对局结束遮罩（从 game_screen 拆出的展示组件）。
library;

import 'package:flutter/material.dart';

import 'theme.dart';

/// 对局结束遮罩：结果展示 + 操作按钮（点击或 3 秒后出现）。
/// 状态（标题文案 / 按钮就绪）由 GamePage 持有，本组件纯展示。
class GameEndOverlay extends StatelessWidget {
  const GameEndOverlay({
    super.key,
    required this.title,
    required this.message,
    required this.actionsReady,
    required this.onTap,
    required this.onReview,
    this.onNewGame,
    required this.onQuit,
    this.quitLabel = '返回主界面',
  });

  /// 结果标题与文案
  final String title;
  final String message;
  /// 是否已到可交互时间（点击遮罩或 3 秒后）
  final bool actionsReady;
  /// 未就绪时点击遮罩：立即出现操作按钮
  final VoidCallback onTap;
  final VoidCallback onReview;
  /// 「再来一局」回调；null = 不提供该选项（复盘续下无"再来一局"语义）
  final VoidCallback? onNewGame;
  final VoidCallback onQuit;
  /// 退出按钮文字；复盘续下返回的是复盘分析页，按钮相应显示「返回复盘」
  final String quitLabel;

  @override
  Widget build(BuildContext context) {
    return Positioned.fill(
      child: GestureDetector(
        // 未到时间前点击：立即出现操作按钮；已出现则不拦截
        onTap: onTap,
        child: Container(
          color: Colors.black.withValues(alpha: 0.55),
          child: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                // 结果标题
                Text(
                  title,
                  style: const TextStyle(
                    fontSize: 44,
                    fontWeight: FontWeight.w700,
                    color: Colors.white,
                  ),
                ),
                const SizedBox(height: 10),
                Text(
                  message,
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    fontSize: 16,
                    color: Colors.white.withValues(alpha: 0.85),
                  ),
                ),
                const SizedBox(height: 36),
                // 操作按钮：点击或 3 秒后出现
                AnimatedOpacity(
                  opacity: actionsReady ? 1.0 : 0.0,
                  duration: const Duration(milliseconds: 250),
                  child: IgnorePointer(
                    ignoring: !actionsReady,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        XqButton(
                          label: '复盘此局',
                          icon: Icons.history,
                          variant: XqButtonVariant.ghost,
                          onPressed: onReview,
                        ),
                        if (onNewGame != null) ...[
                          const SizedBox(width: 12),
                          XqButton(
                            label: '再来一局',
                            icon: Icons.refresh,
                            variant: XqButtonVariant.primary,
                            onPressed: onNewGame,
                          ),
                        ],
                        const SizedBox(width: 12),
                        XqButton(
                          label: quitLabel,
                          icon: Icons.home_outlined,
                          variant: XqButtonVariant.ghost,
                          onPressed: onQuit,
                        ),
                      ],
                    ),
                  ),
                ),
                if (!actionsReady)
                  Padding(
                    padding: const EdgeInsets.only(top: 20),
                    child: Text(
                      '点击任意处继续',
                      style: TextStyle(
                        fontSize: 12,
                        color: Colors.white.withValues(alpha: 0.6),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
