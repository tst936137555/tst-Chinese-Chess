// 复盘界面棋盘布局回归测试：
// BoardView 原本仅按宽度计算格距（cell = width/10，板高 = cell*11），
// 复盘页纵向空间被信息卡/折线图/按钮挤压时棋盘底部被父级裁切
// （红方底线棋子被切半、红方纵线号不可见）。
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:tst_xiangqi/engine/pikafish.dart';
import 'package:tst_xiangqi/engine/rules.dart';
import 'package:tst_xiangqi/game/game_controller.dart';
import 'package:tst_xiangqi/ui/board_view.dart';
import 'package:tst_xiangqi/ui/review_screen.dart';

/// 立即返回的伪造引擎：复盘页进入即自动分析，测试中无需真实引擎
class _FakeEngine implements EngineClient {
  @override
  Future<void> start() async {}

  @override
  Future<EngineResult> think(Board board, DifficultyLevel level) async =>
      EngineResult(move: Move.fromUci('a0a1'), scoreCp: 0);

  @override
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
    int movetimeMs = 0,
  }) async =>
      const AnalysisResult(scoreCp: 12, bestMove: 'h2e2', pvMoves: ['h2e2']);

  @override
  void dispose() {}
}

/// 生成几步真实走子的历史
List<HistoryEntry> _sampleHistory() {
  final board = Board();
  final history = <HistoryEntry>[];
  const ucis = ['h2e2', 'h9g7', 'b2e2', 'b9c7'];
  for (final uci in ucis) {
    final m = Move.fromUci(uci);
    board.makeMove(m);
    history.add(HistoryEntry(
      move: m,
      capturedPiece: null,
      notation: '记谱',
      fenAfter: board.fen,
      posHash: board.positionHash,
    ));
  }
  return history;
}

Future<void> _pumpReview(WidgetTester tester, Size windowSize) async {
  tester.view.physicalSize = windowSize;
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(MaterialApp(
    home: ReviewScreen(
      history: _sampleHistory(),
      userPlaysRed: true,
      engine: _FakeEngine(),
    ),
  ));
  // postFrame 自动分析（伪造引擎立即完成）+ 若干帧稳定布局
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 50));
  await tester.pump(const Duration(milliseconds: 50));
}

void _noopTap(int file, int rank) {}

void main() {
  testWidgets('复盘界面：棋盘按高度收缩，不被父级裁切（460x820 默认窗口）',
      (tester) async {
    await _pumpReview(tester, const Size(460, 820));
    final box = tester.renderObject<RenderBox>(find.byType(BoardView));
    final size = box.size;
    // ignore: avoid_print
    print('BoardView size: $size');
    expect(size.height, closeTo(size.width * 11 / 10, 0.5),
        reason: '棋盘底部被父级裁切：渲染高度 ${size.height} 小于按格距所需高度');
  });

  testWidgets('复盘界面：窗口较矮时同样完整适配（400x560 最小窗口）',
      (tester) async {
    await _pumpReview(tester, const Size(400, 560));
    final box = tester.renderObject<RenderBox>(find.byType(BoardView));
    final size = box.size;
    // ignore: avoid_print
    print('BoardView size (short): $size');
    expect(size.height, closeTo(size.width * 11 / 10, 0.5),
        reason: '矮窗口下棋盘底部被父级裁切');
  });

  testWidgets('BoardView：纵向受限时按高度收缩并保持棋盘比例', (tester) async {
    tester.view.physicalSize = const Size(800, 800);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    // 与实际使用一致：Center 松约束（尺寸由 BoardView 自行决定），
    // SizedBox 仅提供可用区域
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 450,
            height: 400,
            child: Center(
              child: BoardView(
                board: Board(),
                onTapSquare: _noopTap,
                flipBoard: false,
              ),
            ),
          ),
        ),
      ),
    ));
    final box = tester.renderObject<RenderBox>(find.byType(BoardView));
    expect(box.size.height, 400);
    expect(box.size.width, closeTo(400 * 10 / 11, 0.5));
  });

  testWidgets('BoardView：宽度受限时行为不变（450x600 → 450x495）', (tester) async {
    tester.view.physicalSize = const Size(800, 800);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 450,
            height: 600,
            child: Center(
              child: BoardView(
                board: Board(),
                onTapSquare: _noopTap,
                flipBoard: false,
              ),
            ),
          ),
        ),
      ),
    ));
    final box = tester.renderObject<RenderBox>(find.byType(BoardView));
    expect(box.size, const Size(450, 495));
  });
}
