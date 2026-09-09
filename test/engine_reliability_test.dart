// PikafishEngine 可靠性测试：UCI 握手、评分解析、看门狗超时、崩溃恢复、fail-fast。
// 通过 debugIoFactory 注入内存伪造 UCI 引擎，在当前 isolate 内驱动真实会话逻辑，
// 无需真实引擎二进制（可在普通 Linux CI 运行）。
//
// 真实引擎端到端测试见 review_engine_integration_test.dart（无引擎环境自动跳过）。
import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:tst_xiangqi/engine/pikafish.dart';
import 'package:tst_xiangqi/engine/rules.dart';

/// 可编程内存 UCI 引擎：实现 EngineIo 传输协议，按脚本应答。
class ScriptedEngineIo implements EngineIo {
  ScriptedEngineIo({
    this.crashOnGo,
    this.hangOnGo = false,
    this.candidatesCp = const [30],
    this.mateIn,
    this.exitWhenIdle = false,
    this.bestmove = 'h2e2',
  });

  /// 第 N 次 `go` 时崩溃（1 起），模拟引擎运行中崩溃
  final int? crashOnGo;

  /// `go` 后永不响应（触发看门狗）
  final bool hangOnGo;

  /// MultiPV 候选评分（行棋方视角，厘兵）
  final List<int> candidatesCp;

  /// 非空时以 mate 分数应答（N = 绝杀步数）
  final int? mateIn;

  /// 握手完成后立即退出（模拟空闲期崩溃，验证健康通知）
  final bool exitWhenIdle;

  /// bestmove 应答走法（默认红方开局炮二平五；需与局面行棋方合法走法一致）
  final String bestmove;

  void Function(String line)? _out;
  void Function(int code)? _exit;
  int _go = 0;
  int _multiPv = 1;
  bool _exited = false;

  void bind({
    required void Function(String line) onLine,
    required void Function(int code) onExit,
  }) {
    _out = onLine;
    _exit = onExit;
  }

  /// 模拟进程退出（微任务异步，贴近真实进程退出时序）
  void _die(int code) {
    if (_exited) return;
    _exited = true;
    scheduleMicrotask(() => _exit?.call(code));
  }

  void _search() {
    // pv 首步须与 bestmove 一致（真实引擎行为），
    // 且必须是行棋方合法走法（引擎侧 isLegal 兜底会拒绝非法候选）
    final reply = bestmove == 'h9g7' ? 'h2e2' : 'h9g7';
    for (var i = 1; i <= _multiPv; i++) {
      final idx = (i - 1).clamp(0, candidatesCp.length - 1);
      final score =
          mateIn != null ? 'mate ${mateIn! + i - 1}' : 'cp ${candidatesCp[idx]}';
      _out!('info depth 8 multipv $i score $score pv $bestmove $reply');
    }
    _out!('bestmove $bestmove');
  }

  @override
  void send(String line) {
    if (_exited) return;
    if (line == 'uci') {
      _out!('id name scripted-engine');
      _out!('uciok');
    } else if (line == 'isready') {
      _out!('readyok');
      if (exitWhenIdle) {
        // 用真实 Timer 确保退出事件晚于握手完成与就绪消息投递
        // （微任务会抢在 _IsolateReady 之前，被会话误判为握手期退出）
        Timer(const Duration(milliseconds: 20), () => _die(9));
      }
    } else if (line.startsWith('setoption name MultiPV')) {
      _multiPv = int.parse(line.split('value ').last);
    } else if (line == 'quit') {
      _exited = true;
    } else if (line.startsWith('go')) {
      _go++;
      if (crashOnGo != null && _go >= crashOnGo!) {
        _die(3);
        return;
      }
      if (hangOnGo) return;
      _search();
    }
    // position / 其余 setoption / stop：忽略
  }

  @override
  void kill() => _die(0);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  final engine = PikafishEngine.instance;
  const level = DifficultyLevel(
      name: '测试', depth: 3, multiPv: 1, movetimeMs: 100);
  final startBoard = Board(); // 初始局面，红先行

  /// 注册工厂：每次引擎启动创建一个新脚本实例（贴近真实重启）
  void installFactory(ScriptedEngineIo Function() create) {
    engine.debugIoFactory = ({required onLine, required onExit}) async {
      final e = create()..bind(onLine: onLine, onExit: onExit);
      return e;
    };
  }

  void normalFactory() => installFactory(ScriptedEngineIo.new);

  setUp(() {
    engine.debugIoFactory = null;
  });

  tearDown(() {
    // 复位单例状态（关闭会话、清零启动失败计数），隔离用例
    engine.dispose();
    engine.debugIoFactory = null;
  });

  test('UCI 握手后正常走棋并解析评分（红方视角不翻转）', () async {
    normalFactory();

    final r = await engine.think(startBoard, level);

    expect(r.move.uci, 'h2e2');
    expect(r.scoreCp, 30, reason: '红方行棋，评分原样');
    expect(engine.debugEngineReady, isTrue);
  });

  test('黑方行棋时评分翻转为红方视角', () async {
    final parts = Board.startFen.split(' ');
    parts[1] = 'b';
    final blackBoard = Board.fromFen(parts.join(' '));
    // 黑方行棋局面：脚本引擎应答黑方合法走法（马8进7），
    // 引擎侧 isLegal 兜底会拒绝行棋方的非法走法
    installFactory(() => ScriptedEngineIo(bestmove: 'h9g7'));

    final r = await engine.think(blackBoard, level);

    expect(r.move.uci, 'h9g7');
    expect(r.scoreCp, -30, reason: '行棋方（黑）视角 +30 = 红方视角 -30');
  });

  test('mate 分数转换为绝杀分值', () async {
    installFactory(() => ScriptedEngineIo(mateIn: 2));

    final r = await engine.think(startBoard, level);

    expect(r.scoreCp, 9998, reason: 'mate 2 → 10000 - 2');
  });

  test('MultiPV 分析返回多路候选', () async {
    installFactory(() => ScriptedEngineIo(candidatesCp: [30, 10]));

    final r = await engine.analyze(startBoard,
        depth: 6, multiPv: 2, movetimeMs: 100);

    expect(r.bestMove, 'h2e2');
    expect(r.scoreCp, 30);
    expect(r.pvMoves, ['h2e2', 'h9g7']);
    expect(r.pvList.length, 2);
    expect(r.pvList[0].scoreCp, 30);
    expect(r.pvList[1].scoreCp, 10);
  });

  test('请求中崩溃：自动重启引擎并重试成功（崩溃恢复）', () async {
    // 每次启动的脚本引擎在第 2 次 go 时崩溃：
    // 第 1 次 think 正常；第 2 次 think 首请求崩溃 → 错误应答 →
    // 请求级恢复重启引擎 → 重试（新引擎第 1 次 go）成功
    installFactory(() => ScriptedEngineIo(crashOnGo: 2));

    final r1 = await engine.think(startBoard, level);
    expect(r1.move.uci, 'h2e2');

    final r2 = await engine.think(startBoard, level);
    expect(r2.move.uci, 'h2e2', reason: '崩溃后重试应成功，不向上抛错');
    expect(engine.debugEngineReady, isTrue);
  });

  test('空闲期崩溃：健康通知清空状态，下次请求自动重启', () async {
    var exitWhenIdle = true;
    installFactory(() => ScriptedEngineIo(exitWhenIdle: exitWhenIdle));

    await engine.start();
    expect(engine.debugEngineReady, isTrue);

    // 等待握手后的空闲崩溃经健康端口传播
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(engine.debugEngineReady, isFalse,
        reason: '崩溃通知应触发主侧清空端口状态');

    // 下次请求自动重启（新脚本不再崩溃）
    exitWhenIdle = false;
    final r = await engine.think(startBoard, level);
    expect(r.move.uci, 'h2e2');
    expect(engine.debugEngineReady, isTrue);
  });

  test('引擎挂死：看门狗超时报错，不伪造结果', () async {
    installFactory(() => ScriptedEngineIo(hangOnGo: true));

    final sw = Stopwatch()..start();
    await expectLater(
      engine.think(startBoard, level),
      throwsA(predicate<EngineUnavailableException>(
          (e) => e.message.contains('看门狗'))),
      reason: '两次尝试（含崩溃恢复重试）各等 movetime+8s 看门狗，'
          '报"看门狗"而非主侧兜底超时，证明看门狗先于主侧超时生效',
    );
    expect(sw.elapsed, greaterThan(const Duration(seconds: 15)));

    // 看门狗杀掉引擎后可自动重启恢复
    normalFactory();
    final r = await engine.think(startBoard, level);
    expect(r.move.uci, 'h2e2');
  }, timeout: const Timeout(Duration(seconds: 90)));

  test('连续启动失败 3 次后快速失败，dispose 复位后可恢复', () async {
    // 模拟引擎文件缺失：传输层创建即抛错
    engine.debugIoFactory = ({required onLine, required onExit}) async {
      throw StateError('伪造：引擎文件缺失');
    };

    for (var i = 0; i < 3; i++) {
      await expectLater(
        engine.think(startBoard, level),
        throwsA(isA<EngineUnavailableException>()),
      );
    }

    // 第 4 次不再尝试握手，直接快速失败
    final sw = Stopwatch()..start();
    await expectLater(
      engine.think(startBoard, level),
      throwsA(predicate<EngineUnavailableException>(
          (e) => e.message.contains('连续启动失败'))),
    );
    expect(sw.elapsed, lessThan(const Duration(seconds: 2)));

    // dispose 复位启动失败计数后可重新拉起
    engine.dispose();
    normalFactory();
    final r = await engine.think(startBoard, level);
    expect(r.move.uci, 'h2e2');
  });
}
