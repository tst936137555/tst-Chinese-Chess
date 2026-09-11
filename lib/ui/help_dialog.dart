/// 规则说明弹窗：棋谱保留/删除/导出与复盘分析交互说明。
/// 归档页与复盘页共用同一份内容（单一出处）。
library;

import 'package:flutter/material.dart';

import 'theme.dart';

/// 打开"规则说明"弹窗
void showRulesHelpDialog(BuildContext context) {
  showDialog<void>(
    context: context,
    builder: (ctx) => XqDialog(
      title: '规则说明',
      actions: [
        XqButton(
          label: '我知道了',
          variant: XqButtonVariant.primary,
          onPressed: () => Navigator.pop(ctx),
        ),
      ],
      child: ConstrainedBox(
        // 内容较长：小屏时限高滚动，避免溢出
        constraints: const BoxConstraints(maxHeight: 420),
        child: const SingleChildScrollView(
          child: Text(
            '· 棋谱最多保留100局，收藏上限50局，超出自动移除最早对局，收藏棋谱不会被自动移除。\n\n'
            '· 在棋谱列表中向左滑动任意一局即可删除该棋谱，删除前会弹出确认框，删除后不可恢复。\n\n'
            '· 点击棋谱条目上的分享图标，可把该局导出为 PGN 标准棋谱并复制到剪贴板，粘贴到其他象棋工具即可打开。\n\n'
            '· 进入复盘后引擎自动逐手分析：分析期间右上角显示进度，底部翻页按钮暂不可用。\n\n'
            '· 分析完成后，用底部"上一步 / 下一步"逐步查看，点击"局势走势"折线图任意位置可直接跳到对应一步。\n\n'
            '· 每步徽标为引擎对该步的评价：优（与引擎最佳走法一致或亏损不足30厘兵）、良（不足100）、平（不足250）、差（不足600）、错（达到600）；1 厘兵指百分之一兵的价值。"建议：X"表示引擎认为更优的走法。',
            style: TextStyle(fontSize: 14, height: 1.7),
          ),
        ),
      ),
    ),
  );
}
