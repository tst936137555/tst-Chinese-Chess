/// 对局状态栏与最近着法条（从 game_screen 拆出的展示组件）。
library;

import 'package:flutter/material.dart';

import '../engine/rules.dart';
import '../game/game_controller.dart';
import 'theme.dart';

/// 状态栏：AI 难度 / 引擎工作状态（思考·提示·终局判定）/ 行棋方提示 / 回合数。
class GameStatusBar extends StatelessWidget {
  const GameStatusBar({super.key, required this.controller});

  final GameController controller;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    return XqPanel(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
      margin: const EdgeInsets.fromLTRB(12, 4, 12, 4),
      child: Row(
        children: [
          Text(
            'AI：${c.level.name}',
            style: const TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w600,
                color: XqColors.red),
          ),
          const SizedBox(width: 12),
          if (c.thinking)
            const Expanded(
              child: Row(
                children: [
                  SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  SizedBox(width: 8),
                  Text('皮卡鱼思考中…', style: TextStyle(fontSize: 15)),
                ],
              ),
            )
          else if (c.hinting)
            const Expanded(
              child: Row(
                children: [
                  SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  SizedBox(width: 8),
                  Text('引擎计算建议中…', style: TextStyle(fontSize: 15)),
                ],
              ),
            )
          else if (c.ending)
            const Expanded(
              child: Row(
                children: [
                  SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  SizedBox(width: 8),
                  Text('正在分析局势判定胜负…', style: TextStyle(fontSize: 15)),
                ],
              ),
            )
          else
            Expanded(
              child: Text(
                c.status == GameStatus.playing
                    ? (c.isUserTurn
                        ? '轮到你走棋（${c.userPlaysRed ? "红" : "黑"}方）'
                        : '轮到对方走棋')
                    : '对局结束',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                    fontSize: 15, fontWeight: FontWeight.w500),
              ),
            ),
          Text(
            '第 ${c.history.length ~/ 2 + 1} 回合',
            style: const TextStyle(fontSize: 13, color: XqColors.wood),
          ),
        ],
      ),
    );
  }
}

/// 最近着法条：横向滚动的中文记谱序列。
class GameMoveStrip extends StatelessWidget {
  const GameMoveStrip({super.key, required this.controller});

  final GameController controller;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    return SizedBox(
      height: 38,
      child: ListView.builder(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        itemCount: c.history.length,
        itemBuilder: (ctx, i) {
          final e = c.history[i];
          final isRedMove = i.isEven;
          return Padding(
            padding: const EdgeInsets.only(right: 12),
            child: Text(
              '${isRedMove ? '${i ~/ 2 + 1}. ' : ''}${e.notation}',
              style: TextStyle(
                fontSize: 15,
                color: isRedMove
                    ? const Color(0xFFB03020)
                    : const Color(0xFF222222),
                fontWeight: FontWeight.w500,
              ),
            ),
          );
        },
      ),
    );
  }
}
