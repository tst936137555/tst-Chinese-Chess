/// 复盘：入口选择（复盘上局 / 复盘棋谱）与复盘分析界面。
library;

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../engine/chinese_notation.dart';
import '../engine/pikafish.dart';
import '../engine/rules.dart';
import '../game/game_archive.dart';
import '../game/game_controller.dart';
import '../game/review_controller.dart';
import 'board_view.dart';
import 'eval_chart.dart';

/// 复盘上局：使用当前 GameController 的历史
Future<void> openReviewLastGame(
  BuildContext context, {
  required GameController game,
}) {
  return Navigator.of(context).push(MaterialPageRoute(
    fullscreenDialog: true,
    builder: (_) => ReviewScreen(
      history: game.history,
      userPlaysRed: game.userPlaysRed,
    ),
  ));
}

/// 复盘棋谱：打开存档列表选择一局
Future<void> openReviewArchive(BuildContext context) async {
  final game = await Navigator.of(context).push(MaterialPageRoute(
    fullscreenDialog: true,
    builder: (_) => const ArchivePickerScreen(),
  ));
  if (game is ArchivedGame && context.mounted) {
    // 从存档重建历史
    final history = <HistoryEntry>[];
    for (final e in game.history) {
      final uci = e['uci'] as String? ?? '';
      if (uci.length < 4) break;
      history.add(HistoryEntry(
        move: Move.fromUci(uci),
        capturedPiece: e['captured'] as String?,
        notation: e['notation'] as String? ?? uci,
        fenAfter: e['fen'] as String? ?? '',
      ));
    }
    if (!context.mounted) return;
    if (history.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('该棋谱数据异常')),
      );
      return;
    }
    await Navigator.of(context).push(MaterialPageRoute(
      fullscreenDialog: true,
      builder: (_) => ReviewScreen(
        history: history,
        userPlaysRed: game.userRed,
      ),
    ));
  }
}

/// 存档选择页
class ArchivePickerScreen extends StatefulWidget {
  const ArchivePickerScreen({super.key});

  @override
  State<ArchivePickerScreen> createState() => _ArchivePickerScreenState();
}

class _ArchivePickerScreenState extends State<ArchivePickerScreen> {
  List<ArchivedGame> _games = [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final prefs = await SharedPreferences.getInstance();
    _games = await GameArchive.loadAll(prefs);
    _loading = false;
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('复盘棋谱'),
        centerTitle: true,
        actions: [
          IconButton(
            icon: const Icon(Icons.delete_outline),
            tooltip: '清空棋谱',
            onPressed: () async {
              final ok = await showDialog<bool>(
                context: context,
                builder: (ctx) => AlertDialog(
                  title: const Text('清空棋谱'),
                  content: const Text('确定删除全部历史棋谱吗？此操作不可恢复。'),
                  actions: [
                    TextButton(
                        onPressed: () => Navigator.pop(ctx, false),
                        child: const Text('取消')),
                    FilledButton(
                        onPressed: () => Navigator.pop(ctx, true),
                        child: const Text('清空')),
                  ],
                ),
              );
              if (ok == true && context.mounted) {
                final prefs = await SharedPreferences.getInstance();
                await GameArchive.clear(prefs);
                _load();
              }
            },
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _games.isEmpty
              ? const Center(child: Text('暂无历史棋谱\n完成一局对战后自动保存', textAlign: TextAlign.center))
              : ListView.separated(
                  itemCount: _games.length,
                  separatorBuilder: (_, _) => const Divider(height: 1),
                  itemBuilder: (ctx, i) {
                    final g = _games[i];
                    final isWin = g.resultLabel == '胜';
                    final isDraw = g.resultLabel == '和';
                    return ListTile(
                      leading: CircleAvatar(
                        backgroundColor: isWin
                            ? const Color(0xFF2E7D32)
                            : isDraw
                                ? Colors.grey
                                : const Color(0xFFB03020),
                        child: Text(
                          g.resultLabel,
                          style: const TextStyle(
                              color: Colors.white, fontWeight: FontWeight.w700),
                        ),
                      ),
                      // 标题：【对局时间-执红/执黑-胜负】
                      title: Text(
                        g.title,
                        style: const TextStyle(fontSize: 14),
                      ),
                      subtitle: Text('${g.history.length} 回合 · ${g.levelName}'),
                      trailing: const Icon(Icons.chevron_right),
                      onTap: () => Navigator.pop(context, g),
                    );
                  },
                ),
    );
  }
}

/// 复盘分析界面：上方棋盘，中部折线图，底部操作按钮。
class ReviewScreen extends StatefulWidget {
  const ReviewScreen({
    super.key,
    required this.history,
    required this.userPlaysRed,
  });

  final List<HistoryEntry> history;
  final bool userPlaysRed;

  @override
  State<ReviewScreen> createState() => _ReviewScreenState();
}

class _ReviewScreenState extends State<ReviewScreen> {
  late final ReviewController _review;

  @override
  void initState() {
    super.initState();
    _review = ReviewController(
      engine: PikafishEngine.instance,
      history: widget.history,
      userPlaysRed: widget.userPlaysRed,
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
            if (!_review.analyzing)
              IconButton(
                icon: const Icon(Icons.psychology),
                tooltip: '引擎分析',
                onPressed: _review.entries.isEmpty
                    ? null
                    : () => _review.analyzeAll(),
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
                          ? _review.entries[_review.cursor - 1].quality?.color
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
  Widget _buildMoveInfo() {
    if (_review.cursor == 0) {
      return const Padding(
        padding: EdgeInsets.symmetric(horizontal: 16, vertical: 4),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text('初始局面', style: TextStyle(fontWeight: FontWeight.w600)),
        ),
      );
    }
    final e = _review.entries[_review.cursor - 1];
    final isUserMove = (_review.cursor - 1).isEven == widget.userPlaysRed;
    final q = e.quality;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(
                '${_review.cursor ~/ 2 + 1}. ${e.notation}',
                style: const TextStyle(
                    fontSize: 15, fontWeight: FontWeight.w700),
              ),
              const SizedBox(width: 8),
              if (q != null)
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: q.color.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Text(
                    '${q.dot} ${q.label}',
                    style: TextStyle(
                        fontSize: 12, color: q.color, fontWeight: FontWeight.w600),
                  ),
                )
              else if (_review.analyzing)
                const Text('分析中…',
                    style: TextStyle(fontSize: 12, color: Colors.grey)),
              const Spacer(),
              if (e.bestMoveUci != null && e.bestMoveUci != e.move.uci)
                Flexible(
                  child: Text(
                    '建议：${_uciToNotation(e.bestMoveUci!, _review.cursor)}',
                    style: const TextStyle(
                        fontSize: 12, color: Color(0xFF1565C0)),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
          ),
          if (e.loss > 100 && q != null)
            Padding(
              padding: const EdgeInsets.only(top: 2),
              child: Text(
                '此步亏损 ${(e.loss / 100).toStringAsFixed(1)} 兵'
                '${isUserMove ? '（你走的）' : ''}',
                style: const TextStyle(fontSize: 11, color: Colors.grey),
              ),
            ),
        ],
      ),
    );
  }

  /// 将 UCI 走法转为可读记谱（在走这步之前的局面上）
  String _uciToNotation(String uci, int cursor) {
    if (uci == '0000' || uci.length < 4) return uci;
    try {
      final m = Move.fromUci(uci);
      final boardBefore = cursor <= 1
          ? Board()
          : Board.fromFen(_review.history[cursor - 2].fenAfter);
      return moveToChinese(boardBefore, m);
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
                      '点击右上角图标开始引擎分析',
                      style: TextStyle(
                          fontSize: 12,
                          color: Colors.grey.withValues(alpha: 0.7)),
                    ),
                  ),
                ),
        ),
      ],
    );
  }

  /// 当前局面的评分文字
  Widget _currentScoreText() {
    int score;
    if (_review.cursor == 0) {
      score = _review.entries.isNotEmpty ? _review.entries.first.scoreBefore : 0;
    } else {
      score = _review.entries[_review.cursor - 1].scoreAfter;
    }
    String text;
    if (score >= 9000) {
      final mateIn = 10000 - score;
      text = '红方绝杀（$mateIn 步）';
    } else if (score <= -9000) {
      final mateIn = 10000 + score;
      text = '黑方绝杀（$mateIn 步）';
    } else if (score > 0) {
      text = '红方 +${(score / 100).toStringAsFixed(1)}';
    } else if (score < 0) {
      text = '黑方 +${(-score / 100).toStringAsFixed(1)}';
    } else {
      text = '均势';
    }
    return Text(text,
        style: const TextStyle(fontSize: 11, color: Colors.grey));
  }

  /// 底部操作按钮：上一步、下一步、退出（分析未完成时仅退出可操作）
  Widget _buildControls() {
    final busy = _review.analyzing;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 4, 16, 12),
      child: Row(
        children: [
          Expanded(
            child: FilledButton.tonalIcon(
              onPressed: busy ? null : (_review.canBack ? _review.back : null),
              icon: const Icon(Icons.chevron_left),
              label: const Text('上一步'),
            ),
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10),
              child: FilledButton.tonalIcon(
                onPressed: busy
                    ? null
                    : (_review.canForward ? _review.forward : null),
                icon: const Icon(Icons.chevron_right),
                label: const Text('下一步'),
              ),
            ),
          ),
          Expanded(
            child: OutlinedButton.icon(
              onPressed: () => Navigator.of(context).maybePop(),
              icon: const Icon(Icons.exit_to_app),
              label: const Text('退出'),
            ),
          ),
        ],
      ),
    );
  }
}
