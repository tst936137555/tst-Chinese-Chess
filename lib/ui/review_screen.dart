/// 复盘：复盘上局入口与复盘分析界面（存档选择见 archive_picker_screen.dart）。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../engine/chinese_notation.dart';
import '../engine/pikafish.dart';
import '../engine/rules.dart';
import '../game/game_controller.dart';
import '../game/review_controller.dart';
import 'board_view.dart';
import 'eval_chart.dart';
import 'game_screen.dart';
import 'help_dialog.dart';
import 'theme.dart';

/// 复盘上局：使用当前 GameController 的历史
Future<void> openReviewLastGame(
  BuildContext context, {
  required GameController game,
  required SharedPreferences prefs,
}) {
  return Navigator.of(context).push(MaterialPageRoute(
    settings: const RouteSettings(name: '/review'),
    fullscreenDialog: true,
    builder: (_) => ReviewScreen(
      history: game.history,
      userPlaysRed: game.userPlaysRed,
      startFen: game.startFen,
      prefs: prefs,
    ),
  ));
}

/// 复盘分析界面：上方棋盘，中部折线图，底部操作按钮。
class ReviewScreen extends StatefulWidget {
  const ReviewScreen({
    super.key,
    required this.history,
    required this.userPlaysRed,
    this.startFen = Board.startFen,
    required this.prefs,
    this.engine,
  });

  final List<HistoryEntry> history;
  final bool userPlaysRed;
  /// 复盘基准局面：复盘续下的棋谱从自定义局面开始
  final String startFen;
  /// 「当前局面续下」创建对局页所需偏好（与全应用同一实例）
  final SharedPreferences prefs;
  /// 测试注入伪造引擎；空则使用全局单例（生产路径不受影响）
  @visibleForTesting
  final EngineClient? engine;

  @override
  State<ReviewScreen> createState() => _ReviewScreenState();
}

class _ReviewScreenState extends State<ReviewScreen> {
  late final ReviewController _review;

  @override
  void initState() {
    super.initState();
    _review = ReviewController(
      engine: widget.engine ?? PikafishEngine.instance,
      history: widget.history,
      userPlaysRed: widget.userPlaysRed,
      startFen: widget.startFen,
    );
    // 进入复盘自动开始引擎分析
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _review.analyzeAll();
    });
  }

  @override
  void dispose() {
    _review.cancelAnalysis();
    _review.dispose();
    super.dispose();
  }

  /// 「当前局面续下」：从当前浏览到的局面开始新对局。
  /// 先取消复盘分析（引擎客户端内部队列串行化，与对局页引擎调用不冲突），
  /// 选执子后压入对局页；返回后恢复剩余分析（已分析步自动跳过）。
  Future<void> _startFromPosition() async {
    if (_review.analyzing) return;
    _review.cancelAnalysis();
    final userRed = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => XqDialog(
        title: '选择执子',
        actions: [
          XqButton(
            label: '执黑',
            variant: XqButtonVariant.tonal,
            onPressed: () => Navigator.of(ctx).pop(false),
          ),
          XqButton(
            label: '执红',
            variant: XqButtonVariant.primary,
            onPressed: () => Navigator.of(ctx).pop(true),
          ),
        ],
        child: const Padding(
          padding: EdgeInsets.only(top: 2),
          child: Text(
            '从当前局面继续对弈。',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 15, height: 1.7),
          ),
        ),
      ),
    );
    if (userRed == null || !mounted) return;
    await Navigator.of(context).push<void>(MaterialPageRoute<void>(
      // 路由名纳入常亮白名单（app.dart WakeLockRouteObserver）
      settings: const RouteSettings(name: '/game'),
      builder: (_) => GamePage(
        prefs: widget.prefs,
        initialUserRed: userRed,
        startFen: _review.board.fen,
        gameMode: GameMode.review,
      ),
    ));
    // 从对局返回复盘：补算剩余步（analyzeAll 自动跳过已分析步），
    // 发后不管（页面自持状态），避免本函数挂起等待整个分析流程
    if (mounted) unawaited(_review.analyzeAll());
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: _review,
      builder: (context, _) => Scaffold(
        appBar: AppBar(
          title: const Text('复盘分析'),
          centerTitle: true,
          leading: IconButton(
            icon: const Icon(Icons.close),
            tooltip: '退出',
            onPressed: () => Navigator.of(context).maybePop(),
          ),
          actions: [
            // 当前局面续下：分析进行中禁用（分析完成/出错后均可点），
            // 从当前浏览到的局面（cursor 处）继续对弈
            TextButton(
              onPressed: _review.analyzing ? null : _startFromPosition,
              style: TextButton.styleFrom(
                foregroundColor: Colors.white,
                textStyle: const TextStyle(fontSize: 13),
              ),
              child: const Text('当前局面续下'),
            ),
            // 规则说明：复盘交互（翻页/折线图跳转/徽标含义）与归档页共用同一弹窗
            IconButton(
              icon: const Icon(Icons.info_outline, size: 20),
              tooltip: '规则说明',
              onPressed: () => showRulesHelpDialog(context),
            ),
            if (_review.analyzing)
              const Padding(
                padding: EdgeInsets.all(16),
                child: SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
              ),
          ],
        ),
        body: SafeArea(
          child: Column(
            children: [
              // 上方：棋盘
              Expanded(
                child: Center(
                  child: Padding(
                    padding: const EdgeInsets.all(8),
                    child: BoardView(
                      board: _review.board,
                      onTapSquare: (_, _) {},
                      flipBoard: !widget.userPlaysRed,
                      lastMove: _review.currentMove,
                      suggestedMove: _review.suggestedMove,
                      quality: _review.cursor > 0
                          ? _review.entries[_review.cursor - 1].quality
                          : null,
                    ),
                  ),
                ),
              ),
              // 中部：当前步信息
              _buildMoveInfo(),
              // 中部：折线图
              _buildEvalChart(),
              // 底部：操作按钮（分析未完成时仅退出可操作）
              _buildControls(),
            ],
          ),
        ),
      ),
    );
  }

  /// 当前步信息卡
  ///
  /// 固定内容高度：建议走法 / 亏损文字出现与否都不改变卡片高度，
  /// 避免棋盘区域被挤压抖动。
  Widget _buildMoveInfo() {
    const contentHeight = 56.0;
    if (_review.cursor == 0) {
      return XqPanel(
        margin: const EdgeInsets.fromLTRB(12, 4, 12, 4),
        child: SizedBox(
          height: contentHeight,
          child: const Align(
            alignment: Alignment.centerLeft,
            child: Text('初始局面',
                style: TextStyle(fontWeight: FontWeight.w600, fontSize: 14)),
          ),
        ),
      );
    }
    final e = _review.entries[_review.cursor - 1];
    final isUserMove = (_review.cursor - 1).isEven == widget.userPlaysRed;
    final q = e.quality;
    final suggestion = (e.bestMoveUci != null && e.bestMoveUci != e.move.uci)
        ? '建议：${_uciToNotation(e.bestMoveUci!, _review.cursor)}'
        : null;
    // 走法与引擎最佳一致：第二行给正向反馈（保持占位，防止布局跳动）
    final consistent = (e.bestMoveUci != null && e.bestMoveUci == e.move.uci)
        ? '与引擎最佳走法一致'
        : null;
    final lossText = (e.loss > 100 && q != null)
        ? '亏损 ${e.loss} 厘兵'
          '${isUserMove ? '（你走的）' : ''}'
        : null;

    return XqPanel(
      margin: const EdgeInsets.fromLTRB(12, 4, 12, 4),
      child: SizedBox(
        height: contentHeight,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Flexible(
                  child: Text(
                    '${_review.cursor}. ${e.notation}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 15, fontWeight: FontWeight.w700),
                  ),
                ),
                const SizedBox(width: 8),
                // 棋盘棋子上已显示单字角标，此处只显示完整评语，避免重复
                if (q != null)
                  Container(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 8, vertical: 2),
                    decoration: BoxDecoration(
                      color: q.color.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Text(
                      q.label,
                      style: TextStyle(
                          fontSize: 13,
                          color: q.color,
                          fontWeight: FontWeight.w600),
                    ),
                  )
                else if (_review.analyzing)
                  const Text('分析中…',
                      style: TextStyle(fontSize: 12, color: XqColors.wood)),
              ],
            ),
            const SizedBox(height: 3),
            // 第二行固定占位：无"建议/亏损"时显示"与引擎一致"或留空，
            // 避免该行消失导致标题文字在卡片内上下跳动
            SizedBox(
              height: 18,
              child: (suggestion != null || lossText != null)
                  ? Row(
                      children: [
                        if (suggestion != null)
                          Flexible(
                            child: Text(
                              suggestion,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                  fontSize: 13, color: XqColors.red),
                            ),
                          ),
                        if (suggestion != null && lossText != null)
                          const SizedBox(width: 10),
                        if (lossText != null)
                          Flexible(
                            child: Text(
                              lossText,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                  fontSize: 12, color: XqColors.wood),
                            ),
                          ),
                      ],
                    )
                  : consistent == null
                      ? null
                      : Align(
                          alignment: Alignment.centerLeft,
                          child: Text(
                            consistent,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                                fontSize: 13,
                                color: q?.color ?? XqColors.wood,
                                fontWeight: FontWeight.w600),
                          ),
                        ),
            ),
          ],
        ),
      ),
    );
  }

  /// 将 UCI 走法转为可读记谱（在走这步之前的局面上）
  String _uciToNotation(String uci, int cursor) {
    if (uci == '0000' || uci.length < 4) return uci;
    try {
      final m = Move.fromUci(uci);
      return moveToChinese(_review.boardBefore(cursor), m);
    } catch (_) {
      return uci;
    }
  }

  /// 评估折线图 + 当前局面分文字
  Widget _buildEvalChart() {
    final scores = _review.scoreSeries;
    final hasData = _review.analyzedCount > 0 || _review.analyzing;

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 4, 16, 0),
          child: Row(
            children: [
              const Text('局势走势',
                  style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600)),
              const SizedBox(width: 6),
              Expanded(child: _currentScoreText()),
              if (_review.analyzing)
                Text(
                  '${_review.analyzedCount}/${_review.entries.length}',
                  style: const TextStyle(fontSize: 11, color: Colors.grey),
                ),
            ],
          ),
        ),
        if (_review.analysisError != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 2, 16, 0),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text(
                _review.analysisError!,
                style: TextStyle(fontSize: 11, color: Colors.red.shade700),
              ),
            ),
          ),
        Padding(
          padding: const EdgeInsets.fromLTRB(10, 0, 8, 2),
          child: hasData
              ? EvalChart(
                  scores: scores,
                  currentIndex: _review.cursor,
                  onTapIndex: _review.analyzing
                      ? null
                      : (i) => _review.goTo(i),
                )
              : SizedBox(
                  height: 108,
                  child: Center(
                    child: Text(
                      _review.analysisError ??
                          (_review.analyzing ? '正在分析棋谱…' : '暂无分析数据'),
                      style: TextStyle(
                          fontSize: 12,
                          color: _review.analysisError != null
                              ? Colors.red.shade700
                              : Colors.grey.withValues(alpha: 0.7)),
                    ),
                  ),
                ),
        ),
      ],
    );
  }

  /// 当前局面的评分文字
  Widget _currentScoreText() {
    // 当前步是否已完成引擎评估（quality 在该步整步评估结束后才填充）
    bool analyzed;
    int score;
    if (_review.cursor == 0) {
      analyzed = _review.entries.isNotEmpty &&
          _review.entries.first.quality != null;
      score = analyzed ? _review.entries.first.scoreBefore : 0;
    } else {
      final e = _review.entries[_review.cursor - 1];
      analyzed = e.quality != null;
      score = e.scoreAfter;
    }
    if (!analyzed) {
      return const Text('待分析',
          style: TextStyle(fontSize: 14, color: Colors.grey));
    }
    String text;
    if (score >= 9000) {
      final mateIn = 10000 - score;
      text = mateIn == 0 ? '红方已将死' : '红方绝杀（$mateIn 步）';
    } else if (score <= -9000) {
      final mateIn = 10000 + score;
      text = mateIn == 0 ? '黑方已将死' : '黑方绝杀（$mateIn 步）';
    } else if (score > 0) {
      text = '红方 +$score 厘兵';
    } else if (score < 0) {
      text = '黑方 +${-score} 厘兵';
    } else {
      text = '均势';
    }
    return Text(text,
        style: const TextStyle(
            fontSize: 14, fontWeight: FontWeight.w600, color: Colors.grey));
  }

  /// 底部操作按钮：上一步、下一步、退出（分析未完成时仅退出可操作）
  Widget _buildControls() {
    final busy = _review.analyzing;
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 4, 12, 12),
      child: Row(
        children: [
          Expanded(
            child: XqButton(
              label: '上一步',
              icon: Icons.chevron_left,
              variant: XqButtonVariant.tonal,
              onPressed: busy ? null : (_review.canBack ? _review.back : null),
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: XqButton(
              label: '下一步',
              icon: Icons.chevron_right,
              variant: XqButtonVariant.tonal,
              onPressed: busy
                  ? null
                  : (_review.canForward ? _review.forward : null),
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: XqButton(
              label: '退出',
              icon: Icons.exit_to_app,
              variant: XqButtonVariant.outline,
              onPressed: () => Navigator.of(context).maybePop(),
            ),
          ),
        ],
      ),
    );
  }
}
