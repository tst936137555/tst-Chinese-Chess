/// 局内提示横幅：将军 / 规则预警 / 引擎提示（从 game_screen 拆出的展示组件）。
library;

import 'package:flutter/material.dart';

import '../engine/rules.dart';
import '../game/game_controller.dart';

/// 局内醒目提示横幅：将军（红色）/ 重复局面与长将预警（橙色）。
/// 仅对局进行中显示；将军与预警可同时出现。
/// 返回 null 表示当前无横幅；横幅作为棋盘区 Stack 的悬浮层展示，
/// 不参与 Column 布局，出现/消失不引起棋盘抖动。
Widget? buildGameRuleBanner(GameController c) {
  if (c.status != GameStatus.playing) return null;
  final inCheck = c.checkPos != null;
  final notice = c.ruleNotice;
  final engineNotice = c.engineNotice;
  final saveNotice = c.saveNotice;
  if (!inCheck &&
      notice == null &&
      engineNotice == null &&
      saveNotice == null) {
    return null;
  }
  final text = [
    if (inCheck) '将军！',
    ?notice,
    ?engineNotice,
    ?saveNotice,
  ].join('　');
  final color = inCheck ? Colors.red.shade700 : Colors.orange.shade800;
  return Container(
    padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 12),
    decoration: BoxDecoration(
      color: color,
      borderRadius: BorderRadius.circular(10),
      boxShadow: [
        BoxShadow(
          color: Colors.black.withValues(alpha: 0.25),
          blurRadius: 8,
          offset: const Offset(0, 2),
        ),
      ],
    ),
    child: Row(
      children: [
        Icon(
          inCheck
              ? Icons.notification_important_rounded
              : Icons.warning_amber_rounded,
          color: Colors.white,
          size: 18,
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            text,
            // 允许三行：引擎故障/保存失败提示透传具体原因（如 fail-fast 的
            // "请重启应用"引导），单行省略号会把关键引导截掉
            maxLines: 3,
            overflow: TextOverflow.ellipsis,
            textAlign: TextAlign.center,
            style: const TextStyle(
              color: Colors.white,
              fontSize: 15,
              fontWeight: FontWeight.w700,
            ),
          ),
        ),
      ],
    ),
  );
}
