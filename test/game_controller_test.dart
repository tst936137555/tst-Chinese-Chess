// GameController 单元测试：AI 调度、引擎故障传播、FEN 指纹作废、存档恢复。
// 以伪造 EngineClient 注入，无需真实引擎二进制（CI 可运行）。
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:tst_xiangqi/engine/pikafish.dart';
import 'package:tst_xiangqi/engine/rules.dart';
import 'package:tst_xiangqi/game/game_controller.dart';

/// 可编程伪造引擎：按用例注入正常结果 / 挂起门 / 故障
class FakeEngineClient implements EngineClient {
  FakeEngineClient({this.thinkMoveUci = 'h9g7', this.thinkScoreCp = -20});

  final String thinkMoveUci;
  final int thinkScoreCp;

  bool failThink = false;
  bool failAnalyze = false;

  /// 非引擎类异常（验证通用异常同样透传给用户）
  bool failAnalyzeGeneric = false;

  /// 非空时 think() 挂起直到测试放行（模拟慢思考）
  Completer<EngineResult>? thinkGate;
  Completer<AnalysisResult>? analyzeGate;

  /// 非空时按序应答（自然限着等需整段走法序列的用例），耗尽后回落 [thinkMoveUci]
  List<String> thinkMoves = const [];
  int _thinkMoveIdx = 0;

  /// analyze() 的固定应答
  AnalysisResult analyzeResult =
      const AnalysisResult(scoreCp: 30, bestMove: 'h2e2', pvMoves: ['h2e2']);

  int thinkCalls = 0;
  int analyzeCalls = 0;
  final thinkFens = <String>[];

  @override
  Future<void> start() async {}

  @override
  Future<EngineResult> think(Board board, DifficultyLevel level) async {
    thinkCalls++;
    thinkFens.add(board.fen);
    if (failThink) throw const EngineUnavailableException('伪造引擎故障');
    final gate = thinkGate;
    if (gate != null) return gate.future;
    final uci = _thinkMoveIdx < thinkMoves.length
        ? thinkMoves[_thinkMoveIdx++]
        : thinkMoveUci;
    return EngineResult(move: Move.fromUci(uci), scoreCp: thinkScoreCp);
  }

  @override
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
    int movetimeMs = 0,
  }) async {
    analyzeCalls++;
    if (failAnalyze) throw const EngineUnavailableException('伪造引擎故障');
    if (failAnalyzeGeneric) throw StateError('boom');
    final gate = analyzeGate;
    if (gate != null) return gate.future;
    return analyzeResult;
  }

  @override
  void dispose() {}
}

/// 让控制器内的微任务队列（AI 调度/结果应用）跑完
Future<void> _settle() => Future<void>.delayed(const Duration(milliseconds: 30));

/// 测试创建的临时目录（tearDown 统一清理）
final _tempDirs = <Directory>[];

Future<GameController> _newController(FakeEngineClient engine,
    {SharedPreferences? prefs, bool userRed = true, File? archiveFile}) async {
  final p = prefs ?? await _freshPrefs();
  // 存档注入临时文件，避免测试触碰平台通道（path_provider）
  final tmp = await Directory.systemTemp.createTemp('xq_gc_test');
  _tempDirs.add(tmp);
  final c = GameController(
    engine: engine,
    prefs: p,
    initialLevel: DifficultyLevel.master,
    archiveFile: archiveFile ??
        File('${tmp.path}${Platform.pathSeparator}archive.json'),
  );
  c.userPlaysRed = userRed;
  return c;
}

/// 仅写盘失败的伪造 prefs：验证自动保存失败的用户提示路径
class _FailingPrefs implements SharedPreferences {
  @override
  Future<bool> setString(String key, String value) async =>
      throw Exception('模拟磁盘写入失败');

  @override
  dynamic noSuchMethod(Invocation invocation) => throw UnimplementedError(
      '测试未实现: ${invocation.memberName}');
}

Future<SharedPreferences> _freshPrefs() async {
  SharedPreferences.setMockInitialValues({});
  return SharedPreferences.getInstance();
}

/// 生成一条无吃子、无重复局面的走法序列（DFS：每步选第一条
/// 走到未访问局面的合法无吃子走法），供自然限着用例驱动 120 半回合。
List<Move> _wanderLine(Board board, int plies) {
  final path = <Move>[];
  final seen = <int>{board.positionHash};

  bool dfs(Board b) {
    if (path.length >= plies) return true;
    for (final m in b.legalMoves()) {
      if (b.pieceAt(m.toFile, m.toRank) != null) continue; // 无吃子
      final captured = b.pieceAt(m.toFile, m.toRank);
      b.makeMove(m);
      if (seen.contains(b.positionHash)) {
        b.undoMove(m, captured);
        continue;
      }
      seen.add(b.positionHash);
      path.add(m);
      if (dfs(b)) return true;
      path.removeLast();
      seen.remove(b.positionHash);
      b.undoMove(m, captured);
    }
    return false;
  }

  if (!dfs(board)) throw StateError('未找到 $plies 步的无吃子无重复序列');
  return path;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  tearDown(() async {
    // Windows 下归档异步写入可能仍持有句柄，删除短暂重试避免偶发 errno 32
    for (final d in _tempDirs) {
      for (var attempt = 0; attempt < 5; attempt++) {
        if (!await d.exists()) break;
        try {
          await d.delete(recursive: true);
          break;
        } on FileSystemException catch (e) {
          if (attempt == 4) rethrow;
          // ignore: avoid_print
          print('临时目录清理重试 ${attempt + 1}/5: ${e.path}');
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
      }
    }
    _tempDirs.clear();
  });

  test('用户走子后引擎应答并自动落子', () async {
    final engine = FakeEngineClient();
    final c = await _newController(engine);

    expect(c.tryMove(Move.fromUci('h2e2')), isTrue);
    await _settle();

    expect(c.history.length, 2, reason: '用户一步 + AI 一步');
    expect(c.lastMove?.uci, 'h9g7');
    expect(c.thinking, isFalse);
    expect(c.engineNotice, isNull);
    expect(c.isUserTurn, isTrue, reason: 'AI 走完回到用户');
  });

  test('引擎不可用时如实提示，不伪造走法', () async {
    final engine = FakeEngineClient()..failThink = true;
    final c = await _newController(engine);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();

    expect(c.engineNotice, 'AI 暂停走棋：伪造引擎故障');
    expect(c.history.length, 1, reason: 'AI 未能落子');
    expect(c.thinking, isFalse);
    expect(c.status, GameStatus.playing);
  });

  test('局面变化后过期的思考结果被 FEN 指纹作废', () async {
    final engine = FakeEngineClient();
    engine.thinkGate = Completer<EngineResult>();
    final c = await _newController(engine);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();
    expect(engine.thinkCalls, 1);

    // 思考期间开新局：请求时的局面已不复存在
    c.newGame(userRed: true);
    await _settle();
    engine.thinkGate!
        .complete(EngineResult(move: Move.fromUci('b0c2'), scoreCp: 100));
    await _settle();

    expect(c.history, isEmpty, reason: '过期结果不得落子');
    expect(c.board.fen, Board.startFen);
    expect(c.thinking, isFalse);
  });

  test('dispose 后不再应用引擎结果且不崩溃', () async {
    final engine = FakeEngineClient();
    engine.thinkGate = Completer<EngineResult>();
    final c = await _newController(engine);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();
    c.dispose();
    engine.thinkGate!
        .complete(EngineResult(move: Move.fromUci('b0c2'), scoreCp: 100));
    await _settle();

    expect(c.history.length, 1, reason: '页面销毁后结果丢弃');
  });

  test('悔棋：撤销用户与 AI 各一步', () async {
    final engine = FakeEngineClient();
    final c = await _newController(engine);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();
    expect(c.history.length, 2);

    c.undo();
    expect(c.history, isEmpty);
    expect(c.board.fen, Board.startFen);
  });

  test('AI 思考中禁止悔棋', () async {
    final engine = FakeEngineClient();
    engine.thinkGate = Completer<EngineResult>();
    final c = await _newController(engine);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();
    expect(c.thinking, isTrue);

    c.undo();
    expect(c.history.length, 1, reason: '思考中悔棋应被忽略');

    engine.thinkGate!
        .complete(EngineResult(move: Move.fromUci('h9g7'), scoreCp: -20));
    await _settle();
    expect(c.history.length, 2);
  });

  test('恢复损坏存档：JSON 非法时重置棋盘并返回 false', () async {
    SharedPreferences.setMockInitialValues({'saved_game': '{{{bad json'});
    final engine = FakeEngineClient();
    final prefs = await SharedPreferences.getInstance();
    final c = await _newController(engine, prefs: prefs);

    expect(await c.restoreGame(), isFalse);
    expect(c.board.fen, Board.startFen);
    expect(c.history, isEmpty);
  });

  test('恢复损坏存档：走法非法时放弃恢复', () async {
    SharedPreferences.setMockInitialValues({
      'saved_game': jsonEncode({
        'history': [
          {'uci': 'zz', 'captured': null, 'notation': 'x', 'fen': 'bad'}
        ],
        'userRed': true,
        'levelName': '入门',
        'status': 0,
      }),
    });
    final engine = FakeEngineClient();
    final prefs = await SharedPreferences.getInstance();
    final c = await _newController(engine, prefs: prefs);

    expect(await c.restoreGame(), isFalse);
    expect(c.board.fen, Board.startFen);
    expect(c.history, isEmpty);
  });

  test('恢复有效存档：重放历史并触发 AI 应答', () async {
    final board = Board()..makeMove(Move.fromUci('h2e2'));
    SharedPreferences.setMockInitialValues({
      'saved_game': jsonEncode({
        'history': [
          {
            'uci': 'h2e2',
            'captured': null,
            'notation': '炮二平五',
            'fen': board.fen,
          }
        ],
        'userRed': true,
        'levelName': '入门',
        'status': 0,
      }),
    });
    final engine = FakeEngineClient();
    final prefs = await SharedPreferences.getInstance();
    final c = await _newController(engine, prefs: prefs);

    expect(await c.restoreGame(), isTrue);
    await _settle();

    expect(c.userPlaysRed, isTrue);
    expect(c.level, DifficultyLevel.beginner);
    expect(c.history.length, 2, reason: '重放 1 步 + AI 应答 1 步');
    expect(engine.thinkCalls, 1, reason: '恢复后轮到 AI 应自动思考');
  });

  test('结束对局：引擎分析失败时按和棋处理且正常归档', () async {
    final engine = FakeEngineClient()..failAnalyze = true;
    final c = await _newController(engine);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();

    await c.endGameByScore();
    await c.flushArchives();
    expect(c.status, GameStatus.draw);
    expect(c.ending, isFalse);
    // 故障兜底判和须如实提示并透传具体原因，而非伪装成局势判定结果
    expect(c.endReason, '引擎分析失败，按平局结算：伪造引擎故障');
    expect(c.engineNotice, '引擎分析失败，按平局结算：伪造引擎故障');
  });

  test('结束对局：按引擎分差判定胜负', () async {
    final engine = FakeEngineClient();
    engine.analyzeResult =
        const AnalysisResult(scoreCp: 700, bestMove: 'h2e2', pvMoves: ['h2e2']);
    final c = await _newController(engine);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();

    await c.endGameByScore();
    await c.flushArchives();
    expect(c.status, GameStatus.redWin);
  });

  test('60 回合未吃子：自然限着判和', () async {
    // 生成 120 半回合无吃子、无重复局面的走法序列；
    // 偶数下标为用户走法，奇数下标由伪造引擎按序应答
    final line = _wanderLine(Board(), Board.naturalDrawHalfmoves);
    final engine = FakeEngineClient()
      ..thinkMoves = [for (var i = 1; i < line.length; i += 2) line[i].uci];
    final c = await _newController(engine);

    for (var i = 0; i < line.length; i += 2) {
      if (c.status != GameStatus.playing) break;
      final moved = c.tryMove(line[i]);
      expect(moved, isTrue, reason: '第 ${i ~/ 2 + 1} 回合用户走法 ${line[i].uci}');
      await _settle();
    }

    expect(c.status, GameStatus.draw);
    expect(c.endReason, '双方 60 回合未吃子，自然限着作和');
  });

  test('提示失败时展示引擎不可用提示', () async {
    final engine = FakeEngineClient()..failAnalyze = true;
    final c = await _newController(engine);

    await c.hint();
    expect(c.engineNotice, '无法获取提示：伪造引擎故障');
    expect(c.hinting, isFalse);
  });

  test('难度设置持久化到 SharedPreferences', () async {
    SharedPreferences.setMockInitialValues({});
    final engine = FakeEngineClient();
    final prefs = await SharedPreferences.getInstance();
    final c = await _newController(engine, prefs: prefs);

    c.setLevel(DifficultyLevel.beginner);
    expect(prefs.getString('level'), '入门');
    expect(c.level, DifficultyLevel.beginner);
  });

  test('自动保存失败时如实提示（不再静默吞错）', () async {
    final c = await _newController(FakeEngineClient(), prefs: _FailingPrefs());

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();

    expect(c.saveNotice, contains('对局保存失败'));
    expect(c.saveNotice, contains('模拟磁盘写入失败'));
  });

  test('自动保存仅存 uci 序列（不再冗余 fen/captured/notation/status）', () async {
    final prefs = await _freshPrefs();
    final c = await _newController(FakeEngineClient(), prefs: prefs);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();

    final raw = prefs.getString('saved_game');
    expect(raw, isNotNull);
    final data = jsonDecode(raw!) as Map<String, dynamic>;
    expect(data.containsKey('status'), isFalse);
    expect(data['history'], [
      {'uci': 'h2e2'},
      {'uci': 'h9g7'},
    ]);
  });

  test('saveNow 生命周期兜底：AI 思考中也落盘当前局面', () async {
    final engine = FakeEngineClient();
    engine.thinkGate = Completer<EngineResult>();
    final prefs = await _freshPrefs();
    final c = await _newController(engine, prefs: prefs);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();
    await c.saveNow();
    var data =
        jsonDecode(prefs.getString('saved_game')!) as Map<String, dynamic>;
    expect((data['history'] as List).length, 1, reason: 'AI 未应答时保存当前 1 步');

    engine.thinkGate!
        .complete(EngineResult(move: Move.fromUci('h9g7'), scoreCp: -20));
    await _settle();
    data = jsonDecode(prefs.getString('saved_game')!) as Map<String, dynamic>;
    expect((data['history'] as List).length, 2);
  });

  test('归档棋谱不再冗余 fen 字段', () async {
    final tmp = await Directory.systemTemp.createTemp('xq_gc_test');
    _tempDirs.add(tmp);
    final archive = File('${tmp.path}${Platform.pathSeparator}archive.json');
    final c = await _newController(FakeEngineClient(), archiveFile: archive);

    c.tryMove(Move.fromUci('h2e2'));
    await _settle();
    await c.endGameByScore();
    await c.flushArchives();

    final data = jsonDecode(await archive.readAsString()) as List;
    final hist = (data.first as Map)['history'] as List;
    expect(hist.first, {'uci': 'h2e2', 'captured': null, 'notation': '炮二平五'});
  });

  test('提示遇非引擎异常时同样透传给用户', () async {
    final engine = FakeEngineClient()..failAnalyzeGeneric = true;
    final c = await _newController(engine);

    await c.hint();
    expect(c.engineNotice, contains('无法获取提示'));
    expect(c.engineNotice, contains('boom'));
    expect(c.hinting, isFalse);
  });

  test('legalMoves 缓存随走子/悔棋/新局同步', () async {
    final c = await _newController(FakeEngineClient());
    List<String> cached() =>
        c.legalMoves.map((m) => m.uci).toList()..sort();
    List<String> fresh() =>
        c.board.legalMoves().map((m) => m.uci).toList()..sort();

    expect(cached(), fresh());
    c.tryMove(Move.fromUci('h2e2'));
    await _settle();
    expect(cached(), fresh());
    c.undo();
    expect(cached(), fresh());
    c.newGame(userRed: true);
    expect(cached(), fresh());
  });
}
