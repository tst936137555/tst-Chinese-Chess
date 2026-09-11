// ReviewController 单元测试：复盘调度、缓存复用、故障如实呈现。
// 以伪造 EngineClient 注入（复用 game_controller_test.dart 的 FakeEngineClient），
// 无需真实引擎二进制，CI 可运行——弥补此前复盘核心逻辑仅集成测试覆盖的缺口。
import 'package:flutter_test/flutter_test.dart';
import 'package:tst_xiangqi/engine/chinese_notation.dart';
import 'package:tst_xiangqi/engine/rules.dart';
import 'package:tst_xiangqi/game/game_controller.dart';
import 'package:tst_xiangqi/game/review_controller.dart';

import 'game_controller_test.dart';

/// 按走法序列构建历史（与对局时 GameController._applyMove 同构）
List<HistoryEntry> _buildHistory(List<String> ucis) {
  final board = Board();
  final history = <HistoryEntry>[];
  for (final uci in ucis) {
    final m = Move.fromUci(uci);
    final captured = board.pieceAt(m.toFile, m.toRank);
    // 记谱须在走子前的局面上生成
    final notation = moveToChinese(board, m);
    board.makeMove(m);
    history.add(HistoryEntry(
      move: m,
      capturedPiece: captured?.fenChar,
      notation: notation,
      fenAfter: board.fen,
      posHash: board.positionHash,
      givesCheck: board.inCheck,
    ));
  }
  return history;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('analyzeAll：逐局评估、缓存复用、进度推进', () async {
    final engine = FakeEngineClient();
    final review = ReviewController(
      engine: engine,
      history: _buildHistory(['h2e2', 'h9g7']),
      userPlaysRed: true,
    );
    addTearDown(review.dispose);

    await review.analyzeAll();

    expect(review.analyzing, isFalse);
    expect(review.analysisError, isNull);
    expect(review.analyzedCount, 2);
    expect(review.cursor, 2);
    // N 步共 N+1 个局面；after[i] 与 before[i+1] 同局面，缓存合并后恰为 N+1 次请求
    expect(engine.analyzeCalls, 3);
    // 伪造引擎恒评 30 厘兵：无损失 → 全部"优秀"
    for (final e in review.entries) {
      expect(e.quality, MoveQuality.excellent);
      expect(e.scoreBefore, 30);
      expect(e.scoreAfter, 30);
      expect(e.bestMoveUci, 'h2e2');
    }
    expect(review.scoreSeries, [30, 30, 30]);
  });

  test('analyzeAll：引擎故障如实设置 analysisError，不伪造分析结果', () async {
    final engine = FakeEngineClient()..failAnalyze = true;
    final review = ReviewController(
      engine: engine,
      history: _buildHistory(['h2e2']),
      userPlaysRed: true,
    );
    addTearDown(review.dispose);

    await review.analyzeAll();

    expect(review.analyzing, isFalse);
    expect(review.analysisError, isNotNull, reason: '故障必须向用户呈现');
    expect(review.analysisError, contains('引擎不可用'));
    expect(review.analyzedCount, 0);
    expect(review.entries.first.quality, isNull, reason: '不得以空评分冒充结果');
  });

  test('analyzeAll：空棋谱直接返回，不发起请求', () async {
    final engine = FakeEngineClient();
    final review = ReviewController(
      engine: engine,
      history: const [],
      userPlaysRed: true,
    );
    addTearDown(review.dispose);

    await review.analyzeAll();

    expect(review.analyzing, isFalse);
    expect(review.analysisError, isNull);
    expect(engine.analyzeCalls, 0);
  });
}
