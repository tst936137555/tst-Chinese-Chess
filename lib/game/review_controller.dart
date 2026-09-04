/// 复盘分析控制器：逐步浏览历史局面，引擎逐步评估走法质量。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../engine/pikafish.dart';
import '../engine/rules.dart';
import 'game_controller.dart';

/// 走法质量分级
enum MoveQuality {
  brilliant('妙手', '🟣', Color(0xFF7C4DFF)),
  best('最佳', '🟢', Color(0xFF2E7D32)),
  good('好棋', '🔵', Color(0xFF1565C0)),
  inaccuracy('失误', '🟡', Color(0xFFF9A825)),
  mistake('漏着', '🟠', Color(0xFFEF6C00)),
  blunder('败着', '🔴', Color(0xFFC62828));

  const MoveQuality(this.label, this.dot, this.color);
  final String label;
  final String dot;
  final Color color;
}

/// 单步复盘信息
class ReviewEntry {
  ReviewEntry({required this.notation, required this.move, required this.quality});

  /// 中文记谱（如"炮二平五"）
  String notation;
  final Move move;
  /// 走法质量（引擎评估完成后填充）
  MoveQuality? quality;
  /// 走这步前局面的引擎评分（红方视角，厘兵）
  int scoreBefore = 0;
  /// 走这步后局面的引擎评分（红方视角，厘兵）
  int scoreAfter = 0;
  /// 引擎推荐的最佳走法（若与实际走法不同）
  String? bestMoveUci;
  /// 引擎主变化（UCI 序列）
  List<String> pv = [];
  /// 评估损失（红方走法取正损失，黑方取负损失后再比较）
  int loss = 0;
}

/// 复盘控制器
class ReviewController extends ChangeNotifier {
  ReviewController({
    required this.engine,
    required this.history,
    required this.userPlaysRed,
  }) {
    _buildEntries();
  }

  final PikafishEngine engine;
  final List<HistoryEntry> history;
  final bool userPlaysRed;

  /// 当前浏览位置（0 = 初始局面，i = 走完第 i 步后）
  int cursor = 0;

  /// 各步的复盘信息
  late final List<ReviewEntry> entries;

  /// 分析是否正在进行
  bool analyzing = false;

  /// 已分析的步数（进度指示）
  int analyzedCount = 0;

  /// 分析取消标记
  bool _cancelled = false;

  /// 当前位置的局面
  Board get board {
    if (cursor == 0) return Board();
    return Board.fromFen(history[cursor - 1].fenAfter);
  }

  /// 当前显示的走法（最近一步）
  Move? get currentMove =>
      cursor == 0 ? null : history[cursor - 1].move;

  /// 建议走法（引擎推荐）
  Move? get suggestedMove {
    if (cursor == 0) return null;
    final e = entries[cursor - 1];
    final uci = e.bestMoveUci;
    if (uci == null || uci == '0000') return null;
    try {
      return Move.fromUci(uci);
    } catch (_) {
      return null;
    }
  }

  bool get canBack => cursor > 0;
  bool get canForward => cursor < history.length;
  bool get isAtEnd => cursor == history.length;

  /// 折线图评分序列（红方视角）：
  /// 索引 0 = 初始局面，i = 走完第 i 步后。
  /// 未分析的步不包含（分析过程中折线逐步生长）。
  List<int> get scoreSeries {
    if (entries.isEmpty) return [0];
    final list = <int>[entries.first.scoreBefore];
    for (final e in entries) {
      if (e.quality == null) break;
      list.add(e.scoreAfter);
    }
    return list;
  }

  void goTo(int pos) {
    cursor = pos.clamp(0, history.length);
    notifyListeners();
  }

  void back() {
    if (canBack) goTo(cursor - 1);
  }

  void forward() {
    if (canForward) goTo(cursor + 1);
  }

  void first() => goTo(0);
  void last() => goTo(history.length);

  void _buildEntries() {
    // 重建每步走法前的局面来生成记谱
    // HistoryEntry 中已保存记谱，直接复用
    entries = history
        .map((e) => ReviewEntry(
              notation: e.notation,
              move: e.move,
              quality: null,
            ))
        .toList();
  }

  /// 逐步分析整局（走法前后各评估一次）
  Future<void> analyzeAll({int depth = 10}) async {
    if (analyzing || entries.isEmpty) return;
    _cancelled = false;
    analyzing = true;
    analyzedCount = 0;
    notifyListeners();

    try {
      // 局面序列：posBoards[0] = 初始局面，posBoards[i] = 第 i 步后
      final posBoards = <Board>[Board()];
      for (final h in history) {
        posBoards.add(Board.fromFen(h.fenAfter));
      }

      /// 已评估局面缓存（fen → 分析 Future）：
      /// after[i] 与 before[i+1] 是同一局面，复用可省一半搜索量；
      /// 缓存 Future 本身可同时合并进行中的重复请求。
      final cache = <String, Future<AnalysisResult>>{};

      Future<AnalysisResult> eval(Board b) {
        return cache.putIfAbsent(b.fen, () => engine.analyze(b, depth: depth));
      }

      for (int i = 0; i < entries.length; i++) {
        if (_cancelled) break;
        final e = entries[i];

        // 评估走这步之前的局面（得到引擎最佳走法与评分）
        final before = await eval(posBoards[i]);
        if (_cancelled) break;
        e.scoreBefore = before.scoreCp;
        e.bestMoveUci = before.bestMove;
        e.pv = before.pvMoves;

        // 评估走这步之后的局面
        final after = await eval(posBoards[i + 1]);
        if (_cancelled) break;
        e.scoreAfter = after.scoreCp;

        // 从行棋方视角计算损失
        final isRedMove = i.isEven;
        // 红方希望评分高，黑方希望评分低
        final diff = isRedMove
            ? e.scoreBefore - e.scoreAfter // 红走后评分下降 = 损失
            : e.scoreAfter - e.scoreBefore; // 黑走后评分上升 = 损失
        e.loss = diff.clamp(0, 1 << 30);

        // 分级（阈值单位：厘兵 ≈ 1/100 兵）
        e.quality = _classify(e.loss, before.bestMove == history[i].move.uci);
        analyzedCount = i + 1;
        notifyListeners();
      }
    } catch (err) {
      debugPrint('复盘分析出错: $err');
    } finally {
      analyzing = false;
      notifyListeners();
    }
  }

  /// 分级标准
  MoveQuality _classify(int loss, bool isBest) {
    if (isBest && loss < 20) return MoveQuality.brilliant;
    if (loss < 30) return MoveQuality.best;
    if (loss < 100) return MoveQuality.good;
    if (loss < 250) return MoveQuality.inaccuracy;
    if (loss < 600) return MoveQuality.mistake;
    return MoveQuality.blunder;
  }

  void cancelAnalysis() {
    _cancelled = true;
  }

  @override
  void dispose() {
    _cancelled = true;
    super.dispose();
  }
}
