/// 结束此局确认弹窗：打开时立即分析当前局势，展示评分与即将判定的胜负和。
library;

import 'package:flutter/material.dart';

import '../engine/pikafish.dart';
import '../game/game_controller.dart';
import 'theme.dart';

/// 结束此局确认弹窗。
///
/// 弹出值：(是否结束, 预评分)；预评分为 null 表示分析失败，
/// 由 [GameController.endGameByScore] 兜底现场分析（失败按平局结算并提示原因）。
class EndGameDialog extends StatefulWidget {
  const EndGameDialog({super.key, required this.controller});

  final GameController controller;

  @override
  State<EndGameDialog> createState() => _EndGameDialogState();
}

class _EndGameDialogState extends State<EndGameDialog> {
  int? _score;
  String? _error;

  @override
  void initState() {
    super.initState();
    _analyze();
  }

  Future<void> _analyze() async {
    try {
      final s = await widget.controller.analyzeEndingScore();
      if (!mounted) return;
      setState(() => _score = s);
    } catch (e) {
      if (!mounted) return;
      // 引擎故障透传原始信息（含 fail-fast 时的"请重启应用"引导）
      setState(() =>
          _error = e is EngineUnavailableException ? e.message : '$e');
    }
  }

  /// 评分展示（红方视角厘兵，±9000 以上为绝杀分，与复盘走势图同一口径）
  String _scoreText(int score) {
    if (score >= 9000) return '红方绝杀（${10000 - score} 步）';
    if (score <= -9000) return '黑方绝杀（${10000 + score} 步）';
    if (score > 0) return '红方 +$score 厘兵';
    if (score < 0) return '黑方 +${-score} 厘兵';
    return '均势';
  }

  /// 即将判定（与 GameController.endGameByScore 同一分差标准）
  String _verdictText(int score) {
    if (score > GameController.endScoreCp) return '红方获胜';
    if (score < -GameController.endScoreCp) return '黑方获胜';
    return '平局';
  }

  @override
  Widget build(BuildContext context) {
    final ready = _score != null || _error != null;
    return XqDialog(
      title: '结束此局',
      actions: [
        XqButton(
          label: '继续下',
          variant: XqButtonVariant.tonal,
          onPressed: () => Navigator.of(context).pop((false, null)),
        ),
        XqButton(
          label: '结束',
          variant: XqButtonVariant.primary,
          onPressed: ready
              ? () => Navigator.of(context).pop((true, _score))
              : null,
        ),
      ],
      child: _score != null
          ? Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('当前评分：${_scoreText(_score!)}',
                    style: const TextStyle(fontSize: 15, height: 1.7)),
                const SizedBox(height: 4),
                Text('即将判定：${_verdictText(_score!)}',
                    style: const TextStyle(
                        fontSize: 15,
                        height: 1.7,
                        fontWeight: FontWeight.w600)),
                const SizedBox(height: 8),
                Text(
                  '确认结束将按上述结果结算本局。',
                  style: TextStyle(fontSize: 13, color: XqColors.wood),
                ),
              ],
            )
          : _error != null
              ? Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('局势分析失败：$_error',
                        style: TextStyle(
                            fontSize: 14,
                            height: 1.7,
                            color: Colors.red.shade700)),
                    const SizedBox(height: 8),
                    Text(
                      '结束此局将按平局结算。',
                      style: TextStyle(fontSize: 13, color: XqColors.wood),
                    ),
                  ],
                )
              : const SizedBox(
                  height: 56,
                  child: Center(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        SizedBox(
                            width: 18,
                            height: 18,
                            child:
                                CircularProgressIndicator(strokeWidth: 2)),
                        SizedBox(width: 10),
                        Text('正在分析当前局势…',
                            style: TextStyle(fontSize: 14)),
                      ],
                    ),
                  ),
                ),
    );
  }
}
