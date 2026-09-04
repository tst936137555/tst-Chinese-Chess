/// 对话框：难度选择、战绩查看。
library;

import 'package:flutter/material.dart';

import '../engine/pikafish.dart';
import '../game/game_controller.dart';
import '../game/stats_service.dart';

/// 难度选择对话框
Future<void> showLevelDialog(BuildContext context, GameController c) async {
  await showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      title: const Text('选择 AI 难度'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final l in DifficultyLevel.all)
            RadioListTile<DifficultyLevel>(
              value: l,
              groupValue: c.level,
              title: Text(l.name),
              subtitle: Text('约 ${l.elo} Elo'),
              contentPadding: EdgeInsets.zero,
              dense: true,
              // ignore: deprecated_member_use
              onChanged: (v) {
                if (v != null) c.setLevel(v);
                Navigator.of(ctx).pop();
              },
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text('取消'),
        ),
      ],
    ),
  );
}

/// 战绩对话框
Future<void> showStatsDialog(BuildContext context, GameController c) async {
  final stats = c.stats.stats;
  final total = stats.wins + stats.losses + stats.draws;
  await showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      title: const Text('我的战绩'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            '总对局 $total　胜 ${stats.wins}　负 ${stats.losses}　和 ${stats.draws}',
            style: const TextStyle(fontSize: 15),
          ),
          if (total > 0)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                '胜率 ${(stats.wins / total * 100).toStringAsFixed(1)}%',
                style: const TextStyle(fontSize: 13, color: Colors.grey),
              ),
            ),
          const Divider(height: 20),
          const Text('分难度战绩', style: TextStyle(fontWeight: FontWeight.w600)),
          const SizedBox(height: 4),
          for (final l in DifficultyLevel.all)
            Text(
              '${l.name}：${_fmt(c.stats.statsForLevel(l.name))}',
              style: const TextStyle(fontSize: 13),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text('关闭'),
        ),
      ],
    ),
  );
}

String _fmt(Stats s) => '${s.wins}胜 ${s.losses}负 ${s.draws}和';
