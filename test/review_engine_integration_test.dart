// 临时集成测试：真实引擎驱动 ReviewController 复盘分析
import 'package:flutter_test/flutter_test.dart';
import 'package:xiangqi/engine/chinese_notation.dart';
import 'package:xiangqi/engine/pikafish.dart';
import 'package:xiangqi/engine/rules.dart';
import 'package:xiangqi/game/game_controller.dart';
import 'package:xiangqi/game/review_controller.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('ReviewController 真实引擎分析端到端', () async {
    final engine = PikafishEngine.instance;

    // 用几步已知着法构造历史
    final board = Board();
    final history = <HistoryEntry>[];
    for (final uci in ['b2e2', 'b9c7', 'h0g2', 'h9g7']) {
      final m = Move.fromUci(uci);
      expect(board.isLegal(m), isTrue, reason: uci);
      final captured = board.pieceAt(m.toFile, m.toRank)?.fenChar;
      final notation = moveToChinese(board, m);
      board.makeMove(m);
      history.add(HistoryEntry(
        move: m,
        capturedPiece: captured,
        notation: notation,
        fenAfter: board.fen,
      ));
    }

    final review = ReviewController(
      engine: engine,
      history: history,
      userPlaysRed: true,
    );

    await review.analyzeAll();

    // 打印结果便于诊断
    for (var i = 0; i < review.entries.length; i++) {
      final e = review.entries[i];
      // ignore: avoid_print
      print('#$i ${e.notation} before=${e.scoreBefore} after=${e.scoreAfter} '
          'loss=${e.loss} quality=${e.quality} best=${e.bestMoveUci}');
    }
    // ignore: avoid_print
    print('scoreSeries = ${review.scoreSeries}');

    expect(review.analyzedCount, review.entries.length);
    expect(review.entries.first.quality, isNotNull);
    expect(review.scoreSeries.length, review.entries.length + 1);

    // mate 0（已被绝杀局面）映射为 -10000：红方视角由 toRed 翻转
    // 引擎侧已修正符号，此处仅验证普通局面评分非全 0
    expect(review.scoreSeries.any((s) => s != 0), isTrue);

    engine.dispose();
  }, timeout: const Timeout(Duration(minutes: 5)));

  test('scoreSeries 全长且未分析步沿用最近评分', () async {
    final engine = PikafishEngine.instance;

    // 构造 3 步历史（不执行分析）
    final board = Board();
    final history = <HistoryEntry>[];
    for (final uci in ['b2e2', 'b9c7', 'h0g2']) {
      final m = Move.fromUci(uci);
      final captured = board.pieceAt(m.toFile, m.toRank)?.fenChar;
      final notation = moveToChinese(board, m);
      board.makeMove(m);
      history.add(HistoryEntry(
        move: m,
        capturedPiece: captured,
        notation: notation,
        fenAfter: board.fen,
      ));
    }

    final review = ReviewController(
      engine: engine,
      history: history,
      userPlaysRed: true,
    );

    // 未分析：全长序列、全 0
    expect(review.scoreSeries.length, 4);
    expect(review.scoreSeries, everyElement(0));

    // 模拟第 1 步分析完成
    review.entries[0]
      ..scoreBefore = 30
      ..scoreAfter = 55
      ..quality = MoveQuality.excellent;
    expect(review.scoreSeries, [30, 55, 55, 55]);
  });
}
