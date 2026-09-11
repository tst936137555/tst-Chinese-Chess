// 对局页交互测试：点选走子、悔棋、切后台自动保存。
// 经 GamePage.engine 注入伪造引擎，无需真实引擎二进制（CI 可运行）。
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:tst_xiangqi/engine/pikafish.dart';
import 'package:tst_xiangqi/engine/rules.dart';
import 'package:tst_xiangqi/game/game_controller.dart';
import 'package:tst_xiangqi/ui/board_view.dart';
import 'package:tst_xiangqi/ui/game_screen.dart';

/// 可编程伪造引擎（与 game_controller_test 相同的轻量副本）
class FakeEngineClient implements EngineClient {
  FakeEngineClient({this.thinkMoveUci = 'h9g7'});

  final String thinkMoveUci;

  int thinkCalls = 0;
  final thinkFens = <String>[];

  @override
  Future<void> start() async {}

  @override
  Future<EngineResult> think(Board board, DifficultyLevel level) async {
    thinkCalls++;
    thinkFens.add(board.fen);
    return EngineResult(move: Move.fromUci(thinkMoveUci), scoreCp: -20);
  }

  @override
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
    int movetimeMs = 0,
  }) async {
    return const AnalysisResult(scoreCp: 30, bestMove: 'h2e2', pvMoves: ['h2e2']);
  }

  @override
  void dispose() {}
}

GameController _controllerOf(WidgetTester tester) =>
    (tester.state(find.byType(GamePage)) as dynamic).controller
        as GameController;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  /// 挂载对局页并等待异步初始化完成（新开局就绪）
  Future<GameController> pumpGamePage(WidgetTester tester,
      {required SharedPreferences prefs, required FakeEngineClient engine}) async {
    await tester.pumpWidget(MaterialApp(
      home: GamePage(
        prefs: prefs,
        initialLevel: DifficultyLevel.master,
        initialUserRed: true,
        engine: engine,
      ),
    ));
    // _init 异步创建控制器并开新局
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    return _controllerOf(tester);
  }

  /// 棋盘交点 (file, rank) 的全局点击坐标
  Offset squareOffset(WidgetTester tester, int file, int rank) {
    // BoardView 的 SizedBox 尺寸为 cell*10 × cell*11
    final cell = tester.getSize(find.byType(BoardView)).width / 10;
    return tester.getTopLeft(find.byType(BoardView)) +
        Offset((file + 1) * cell, (rank + 1) * cell);
  }

  testWidgets('点选红炮走炮二平五，AI 自动应答', (tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final engine = FakeEngineClient();
    final c = await pumpGamePage(tester, prefs: prefs, engine: engine);

    // 点击对方棋子（黑炮 h9）：不产生走子
    await tester.tapAt(squareOffset(tester, 7, 1));
    await tester.pump();
    expect(engine.thinkCalls, 0);

    // 选中红炮 h2（file 7, rank 7）→ 走 h2e2（炮二平五）
    await tester.tapAt(squareOffset(tester, 7, 7));
    await tester.pump();
    await tester.tapAt(squareOffset(tester, 4, 7));
    await tester.pump();
    // 动画（260ms）+ AI 应答
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();

    expect(engine.thinkCalls, 1);
    expect(engine.thinkFens.first, isNot(Board.startFen),
        reason: '引擎收到的是走子后的局面');
    expect(c.history.map((e) => e.move.uci), ['h2e2', 'h9g7']);
  });

  testWidgets('悔棋撤销用户与 AI 各一步', (tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final engine = FakeEngineClient();
    final c = await pumpGamePage(tester, prefs: prefs, engine: engine);

    await tester.tapAt(squareOffset(tester, 7, 7));
    await tester.pump();
    await tester.tapAt(squareOffset(tester, 4, 7));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();
    expect(c.history.length, 2);

    await tester.tap(find.text('悔棋'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    expect(c.history, isEmpty);
    expect(c.board.fen, Board.startFen);
    expect(engine.thinkCalls, 1, reason: '悔棋回到用户回合，不再触发 AI');
  });

  testWidgets('切后台时自动保存当前对局', (tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    final engine = FakeEngineClient();
    final c = await pumpGamePage(tester, prefs: prefs, engine: engine);

    await tester.tapAt(squareOffset(tester, 7, 7));
    await tester.pump();
    await tester.tapAt(squareOffset(tester, 4, 7));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();
    expect(c.history.length, 2);

    // 清除自动保存痕迹，隔离验证生命周期兜底路径独立生效
    await prefs.remove('saved_game');
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    await tester.pump();
    await tester.pump();

    final raw = prefs.getString('saved_game');
    expect(raw, isNotNull);
    final data = jsonDecode(raw!) as Map<String, dynamic>;
    expect((data['history'] as List).length, 2);
  });
}
