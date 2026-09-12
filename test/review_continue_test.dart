// 复盘续下（自定义起始局面 startFen + GameMode）单元测试：
// GameController 起始局面摆盘与 AI 先行调度、悔棋回到起始局面、
// 存档恢复与损坏回滚、归档字段与（复盘）标题前缀、
// ReviewController 以自定义局面为分析基准与 analyzeAll 续跑跳过。
// 伪造引擎复用 game_controller_test.dart 的 FakeEngineClient，CI 可运行。
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:tst_xiangqi/engine/chinese_notation.dart';
import 'package:tst_xiangqi/engine/pikafish.dart';
import 'package:tst_xiangqi/engine/rules.dart';
import 'package:tst_xiangqi/game/game_archive.dart';
import 'package:tst_xiangqi/game/game_controller.dart';
import 'package:tst_xiangqi/game/review_controller.dart';
import 'package:tst_xiangqi/ui/game_end_overlay.dart';
import 'package:tst_xiangqi/ui/review_screen.dart';

import 'game_controller_test.dart';

/// 记录 analyze() 收到局面的伪造引擎（验证分析以自定义起始局面为基准）
class _FenRecordingEngine implements EngineClient {
  final analyzeFens = <String>[];

  @override
  Future<void> start() async {}

  @override
  Future<EngineResult> think(Board board, DifficultyLevel level) async =>
      EngineResult(move: Move.fromUci('h9g7'), scoreCp: -20);

  @override
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
    int movetimeMs = 0,
  }) async {
    analyzeFens.add(board.fen);
    return const AnalysisResult(
        scoreCp: 30, bestMove: 'h2e2', pvMoves: ['h2e2']);
  }

  @override
  void dispose() {}
}

/// 让控制器内的微任务队列（AI 调度/结果应用）跑完
Future<void> _settle() =>
    Future<void>.delayed(const Duration(milliseconds: 30));

Future<SharedPreferences> _freshPrefs() async {
  SharedPreferences.setMockInitialValues({});
  return SharedPreferences.getInstance();
}

/// 标准开局走一步炮二平五后的局面（轮黑方行棋），作为自定义起始局面
final String _customStart = (Board()..makeMove(Move.fromUci('h2e2'))).fen;

/// 起始局面 + 黑应着 h9g7 后的局面
final String _afterAiReply =
    (Board.fromFen(_customStart)..makeMove(Move.fromUci('h9g7'))).fen;

/// 按起始局面 + 走法序列构建历史（与归档重放 _historyFromArchive 同构）
List<HistoryEntry> _buildHistoryFrom(String startFen, List<String> ucis) {
  final board = Board.fromFen(startFen);
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

Future<GameController> _newController(
  FakeEngineClient engine, {
  SharedPreferences? prefs,
  GameMode mode = GameMode.normal,
}) async {
  final p = prefs ?? await _freshPrefs();
  // 存档注入临时文件，避免测试触碰平台通道（path_provider）
  final tmp = await Directory.systemTemp.createTemp('xq_rc_test');
  addTearDown(() => tmp.delete(recursive: true));
  final c = GameController(
    engine: engine,
    prefs: p,
    initialLevel: DifficultyLevel.master,
    archiveFile: File('${tmp.path}${Platform.pathSeparator}archive.json'),
    mode: mode,
  );
  return c;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('newGame 自定义起始局面：摆盘/模式/行棋方与 AI 先行调度', () async {
    final engine = FakeEngineClient()..thinkMoves = ['h9g7'];
    final prefs = await _freshPrefs();
    // 与真实入口一致：构造时即指定复盘模式（对局页透传 gameMode）
    final c = await _newController(engine, prefs: prefs,
        mode: GameMode.review);
    addTearDown(c.dispose);

    // 续下点轮黑方行棋，玩家执红 → AI（黑）应自动先行
    c.newGame(userRed: true, startFen: _customStart, mode: GameMode.review);
    await _settle();

    expect(c.startFen, _customStart);
    expect(c.mode, GameMode.review);
    expect(c.history, hasLength(1));
    expect(c.history.first.move.uci, 'h9g7');
    expect(c.board.fen, _afterAiReply);
    // 轮红方（玩家）行棋：合法走法均属红子
    for (final m in c.legalMoves) {
      expect(c.board.pieceAt(m.fromFile, m.fromRank)!.isRed, isTrue);
    }
    // 非普通模式不得污染全局难度/执子偏好
    expect(prefs.getString('level'), isNull);
  });

  test('悔棋撤光后回到本局起始局面（非标准开局）', () async {
    final engine = FakeEngineClient()..thinkMoves = ['h9g7', 'b9c7'];
    final c = await _newController(engine);
    addTearDown(c.dispose);

    c.newGame(userRed: true, startFen: _customStart, mode: GameMode.review);
    await _settle(); // AI 黑：h9g7
    c.tryMove(Move.fromUci('h0g2')); // 玩家红：马二进三
    await _settle(); // AI 黑：b9c7
    expect(c.history, hasLength(3));

    c.undo(); // 撤玩家 + AI 各一步
    expect(c.history, hasLength(1));
    expect(c.board.fen, _afterAiReply);
    c.undo(); // 再撤 AI 一步 → 历史撤光
    expect(c.history, isEmpty);
    expect(c.board.fen, _customStart,
        reason: '悔棋撤光应回到续下起始局面，而非标准开局');
  });

  test('恢复存档：startFen 与 mode 随档恢复', () async {
    SharedPreferences.setMockInitialValues({
      'saved_game': jsonEncode({
        'history': [
          {'uci': 'h9g7'}
        ],
        'userRed': true,
        'levelName': '大师',
        'startFen': _customStart,
        'mode': 'review',
      }),
    });
    final engine = FakeEngineClient();
    final prefs = await SharedPreferences.getInstance();
    final c = await _newController(engine, prefs: prefs);
    addTearDown(c.dispose);

    expect(await c.restoreGame(), isTrue);
    expect(c.startFen, _customStart);
    expect(c.mode, GameMode.review);
    expect(c.history, hasLength(1));
    expect(c.board.fen, _afterAiReply);
  });

  test('恢复损坏存档：失败后回滚到恢复前的起始局面与模式', () async {
    final engine = FakeEngineClient()..thinkMoves = ['h9g7'];
    final prefs = await _freshPrefs();
    final c = await _newController(engine, prefs: prefs);
    addTearDown(c.dispose);

    // 恢复前状态：自定义起始局面 + 复盘模式，已走一步
    c.newGame(userRed: true, startFen: _customStart, mode: GameMode.review);
    await _settle();
    expect(c.history, hasLength(1));

    // 合法 JSON 但走法不合法（红兵三步）→ 放弃恢复
    await prefs.setString(
        'saved_game',
        jsonEncode({
          'history': [
            {'uci': 'e3e6'}
          ],
          'userRed': true,
          'levelName': '大师',
          'startFen': _customStart,
          'mode': 'review',
        }));
    expect(await c.restoreGame(), isFalse);
    // 回滚：保持恢复前的起始局面/模式，不残留半截恢复状态
    expect(c.startFen, _customStart);
    expect(c.mode, GameMode.review);
    expect(c.history, isEmpty);
    expect(c.board.fen, _customStart);
  });

  test('归档：startFen/mode 序列化往返与（复盘）标题前缀', () {
    final t = DateTime(2026, 9, 12, 10, 30);
    final game = ArchivedGame(
      time: t,
      userRed: true,
      levelName: '大师',
      result: 'redWin',
      history: const [],
      startFen: _customStart,
      mode: 'review',
    );
    final restored = ArchivedGame.fromJson(game.toJson());
    expect(restored.startFen, _customStart);
    expect(restored.mode, 'review');
    expect(restored.title, startsWith('（复盘）'));
    // 收藏复制不得丢失起始局面与模式
    expect(game.withFavorite(true).startFen, _customStart);
    expect(game.withFavorite(true).mode, 'review');

    // 旧档兼容：无新字段 → 标准开局/普通模式，标题无前缀
    final legacy = ArchivedGame.fromJson({
      'time': t.millisecondsSinceEpoch,
      'userRed': true,
      'levelName': '大师',
      'result': 'redWin',
      'history': <Map<String, dynamic>>[],
      'favorite': false,
    });
    expect(legacy.startFen, isNull);
    expect(legacy.mode, 'normal');
    expect(legacy.title, isNot(contains('（复盘）')));
  });

  test('ReviewController：自定义起始局面为重放与分析基准', () async {
    final history = _buildHistoryFrom(_customStart, ['h9g7', 'h0g2']);
    final engine = _FenRecordingEngine();
    final review = ReviewController(
      engine: engine,
      history: history,
      userPlaysRed: true,
      startFen: _customStart,
    );
    addTearDown(review.dispose);

    // 游标 0 与建议记谱基准均为自定义局面
    expect(review.board.fen, _customStart);
    expect(review.boardBefore(0).fen, _customStart);
    expect(review.boardBefore(1).fen, _customStart);
    expect(review.boardBefore(2).fen, history[0].fenAfter);

    await review.analyzeAll();
    expect(review.analysisError, isNull);
    // 2 步 → 3 个局面，首个分析局面即自定义起始局面
    expect(engine.analyzeFens, hasLength(3));
    expect(engine.analyzeFens.first, _customStart);
    expect(review.entries.every((e) => e.quality != null), isTrue);
  });

  test('analyzeAll 续跑：跳过已完成步，只评估剩余局面', () async {
    final history = _buildHistoryFrom(_customStart, ['h9g7', 'h0g2']);
    final engine = FakeEngineClient();
    final review = ReviewController(
      engine: engine,
      history: history,
      userPlaysRed: true,
      startFen: _customStart,
    );
    addTearDown(review.dispose);
    const result =
        AnalysisResult(scoreCp: 30, bestMove: 'h2e2', pvMoves: ['h2e2']);

    // 第一轮：门控让第一步（前/后两次评估）评完、第二步评估挂起，然后取消
    engine.analyzeGate = Completer<AnalysisResult>();
    unawaited(review.analyzeAll());
    await _settle();
    engine.analyzeGate!.complete(result);
    // 同步换新门：下一次评估挂到新门上
    engine.analyzeGate = Completer<AnalysisResult>();
    await _settle();
    engine.analyzeGate!.complete(result); // 第一步"之后"局面评估完成 → 第一步齐套
    engine.analyzeGate = Completer<AnalysisResult>();
    await _settle();
    expect(review.entries[0].quality, isNotNull);
    expect(review.entries[1].quality, isNull);
    expect(engine.analyzeCalls, 3, reason: '首轮评估 3 个局面（第 3 个挂起中）');

    review.cancelAnalysis();
    engine.analyzeGate!.complete(result); // 放行挂起请求 → 循环因取消而中止
    await _settle();
    expect(review.analyzing, isFalse);
    expect(review.entries[1].quality, isNull, reason: '取消后不得伪造第二步结果');

    // 第二轮：仅补算第二步（第一步与已评估局面不再重复请求引擎）
    final callsBefore = engine.analyzeCalls;
    await review.analyzeAll();
    expect(review.analysisError, isNull);
    expect(review.entries.every((e) => e.quality != null), isTrue);
    expect(review.analyzedCount, 2);
    expect(review.cursor, 2);
    expect(engine.analyzeCalls - callsBefore, 2,
        reason: '续跑只评估剩余局面，不重复消耗引擎时间');
  });

  testWidgets('当前局面续下：一方被将死时禁止续走', (tester) async {
    // 将死局面：黑帅宫底被红车正面将军，另两车封死两侧全部逃点
    // （黑方无子可动，轮黑行棋）
    const matedFen = '4k4/9/9/3RRR3/9/9/9/9/9/4K4 b';
    // 自校验：该局面确为终局（黑被将死 → 红胜）
    expect(Board.fromFen(matedFen).statusAfterMove(), GameStatus.redWin);

    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();

    Future<TextButton> pumpButton(
        String startFen, List<HistoryEntry> history) async {
      // UniqueKey 强制重建 State：同一测试连续 pumpWidget 时，
      // 同类型无键 widget 会复用旧 State（initState 不重跑，控制器
      // 仍持有上一个局面的 startFen/历史）
      await tester.pumpWidget(MaterialApp(
        key: UniqueKey(),
        home: ReviewScreen(
          history: history,
          userPlaysRed: true,
          startFen: startFen,
          prefs: prefs,
          engine: _FenRecordingEngine(),
        ),
      ));
      // 等自动分析完成（伪造引擎立即返回）：分析完成后加载圈消失，
      // 无连续动画帧，pumpAndSettle 正常收敛
      await tester.pumpAndSettle();
      final finder = find.widgetWithText(TextButton, '当前局面续下');
      expect(finder, findsOneWidget);
      return tester.widget<TextButton>(finder);
    }

    // 将死局面：按钮禁用
    final mated = await pumpButton(matedFen, const []);
    expect(mated.onPressed, isNull, reason: '一方被将死时不允许续走');

    // 普通局面（行棋方有合法走法）：按钮可用
    final normal = await pumpButton(
        Board.startFen, _buildHistoryFrom(Board.startFen, ['h2e2']));
    expect(normal.onPressed, isNotNull);
  });

  test('复盘续下 0 步直接结束：按自定义起始局面评分结算，不无视局面判和', () async {
    final engine = FakeEngineClient()
      ..analyzeResult = const AnalysisResult(
          scoreCp: 1200, bestMove: 'h2e2', pvMoves: ['h2e2']);
    final prefs = await _freshPrefs();
    final c = await _newController(engine, prefs: prefs, mode: GameMode.review);
    addTearDown(c.dispose);

    // 自定义起始局面红方先行（轮玩家执红），一步未走直接结束
    const redUpFen = '4k4/9/9/9/9/9/9/9/9/4KR3 w';
    c.newGame(userRed: true, startFen: redUpFen, mode: GameMode.review);
    await _settle();
    expect(c.history, isEmpty, reason: '轮玩家（红）行棋，AI 不先走');

    // 弹窗预览评分：真实分析当前摆盘局面，不再恒回 0
    expect(await c.analyzeEndingScore(), 1200);

    await c.endGameByScore();
    expect(c.status, GameStatus.redWin,
        reason: '红优 1200 厘兵应判红胜，而非无视局面直接判和');
  });

  test('普通对局 0 步直接结束：维持均势判和捷径，不消耗引擎分析', () async {
    final engine = FakeEngineClient()
      ..analyzeResult = const AnalysisResult(
          scoreCp: 1200, bestMove: 'h2e2', pvMoves: ['h2e2']);
    final c = await _newController(engine);
    addTearDown(c.dispose);

    c.newGame(userRed: true);
    await _settle();
    expect(c.history, isEmpty);

    await c.endGameByScore();
    expect(c.status, GameStatus.draw);
    expect(engine.analyzeCalls, 0, reason: '标准开局无局势可评，不应发起分析');
  });

  testWidgets('当前局面续下：前后导航切换将死/非将死局面，按钮状态跟随', (tester) async {
    // 起始局面：红三车，黑仅剩将且未被将军（红方行棋，有合法走法）；
    // 车a6-e6 横移至宫顶线正面将军，d/f 两车封死两侧逃点 → 黑被将死
    const startFen = '4k4/9/9/R8/3R1R3/9/9/9/9/3K5 w';
    final history = _buildHistoryFrom(startFen, ['a6e6']);
    // 自校验：走子前可继续（playing），走子后黑被将死（redWin）
    expect(Board.fromFen(startFen).statusAfterMove(), GameStatus.playing);
    expect(
        Board.fromFen(history.last.fenAfter).statusAfterMove(), GameStatus.redWin);

    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await tester.pumpWidget(MaterialApp(
      home: ReviewScreen(
        history: history,
        userPlaysRed: true,
        startFen: startFen,
        prefs: prefs,
        engine: _FenRecordingEngine(),
      ),
    ));
    await tester.pumpAndSettle(); // 自动分析完成，cursor 停在末尾（将死局面）

    Future<TextButton> readButton() async {
      await tester.pumpAndSettle();
      return tester.widget<TextButton>(
          find.widgetWithText(TextButton, '当前局面续下'));
    }

    // 分析结束停在将死局面：禁用
    expect((await readButton()).onPressed, isNull,
        reason: '将死局面不允许续走');

    // 上一步回退到非终局局面：恢复可用（isPositionOver 按 cursor 重算）
    await tester.tap(find.text('上一步'));
    expect((await readButton()).onPressed, isNotNull,
        reason: '回退到非终局局面后应恢复可续走');

    // 下一步回到将死局面：再次禁用
    await tester.tap(find.text('下一步'));
    expect((await readButton()).onPressed, isNull,
        reason: '再前进到将死局面应再次禁用');
  });

  testWidgets('续下对局结束遮罩不提供「再来一局」', (tester) async {
    Future<void> pump(VoidCallback? onNewGame, {String? quitLabel}) async {
      await tester.pumpWidget(MaterialApp(
        home: Scaffold(
          body: Stack(children: [
            GameEndOverlay(
              title: '红方胜',
              message: '黑方被将死',
              actionsReady: true,
              onTap: () {},
              onReview: () {},
              onNewGame: onNewGame,
              onQuit: () {},
              quitLabel: quitLabel ?? '返回主界面',
            ),
          ]),
        ),
      ));
      await tester.pumpAndSettle();
    }

    // 复盘续下（onNewGame = null）：仅复盘此局 / 返回复盘
    await pump(null, quitLabel: '返回复盘');
    expect(find.text('复盘此局'), findsOneWidget);
    expect(find.text('再来一局'), findsNothing);
    expect(find.text('返回复盘'), findsOneWidget);

    // 普通对局（onNewGame 非空）：三按钮齐全（防普通模式回归）
    await pump(() {});
    expect(find.text('再来一局'), findsOneWidget);
    expect(find.text('返回主界面'), findsOneWidget);
  });
}
