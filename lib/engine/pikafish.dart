/// 皮卡鱼 UCI 引擎封装：进程管理、难度设置、走棋请求。
///
/// 使用 Dart Isolate 中运行 Process 的方式与引擎通信，
/// 通过 SendPort 将结果回传到主 isolate。
///
/// 所有走棋/分析请求在主 isolate 侧串行排队，
/// 同一时刻只向引擎发送一个搜索指令，避免请求被丢弃。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';
import 'dart:math' as math;


import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'rules.dart';

/// 难度等级定义（应用层削弱：深度/时间双限 + MultiPV 分差容差加权随机选路）
class DifficultyLevel {
  const DifficultyLevel({
    required this.name,
    required this.depth,
    required this.multiPv,
    required this.movetimeMs,
    this.toleranceCp = 0,
    this.minThinkMs = 0,
    this.fullStrength = false,
  });

  final String name;
  /// 搜索深度上限（与 [movetimeMs] 先到先停）；0 表示不限深度。
  final int depth;
  /// MultiPV 候选路数：引擎返回前 N 优走法，按"越优越常被选"的加权随机选路。
  final int multiPv;
  /// 每步思考时间上限（毫秒）。
  final int movetimeMs;
  /// 分差容差（厘兵）：仅从与最佳着法分差不超过该值的候选中随机选路，
  /// 避免按名次削弱时选出大劣着（亏 2-3 兵）。0 = 只走最佳。
  final int toleranceCp;
  /// 最低思考时长（毫秒）：低档深度极浅，实际几十毫秒即完成搜索，
  /// 补足最少思考时间让落子节奏自然。0 = 不补足。
  final int minThinkMs;
  /// true = 满强度（大师）：MultiPV 1，随机选路不生效。
  final bool fullStrength;

  /// 皮卡鱼 2026-09-06 起移除了 UCI_Elo / UCI_LimitStrength 官方削弱机制，
  /// 改为应用层模拟棋力梯度：档位越低，深度/时间越保守、候选越宽、越易偏离最佳。
  /// 容差按旧版 Elo 档位（1500-2400）的手感标定：档位越低容差越大、越"人味"。
  /// depth 取值贴近当前主流设备的实测有效深度：
  /// 快设备上深度先到（棋力跨设备一致），慢设备上时间先到（延迟可控）。
  static const beginner = DifficultyLevel(
      name: '入门',
      depth: 6,
      multiPv: 5,
      movetimeMs: 1000,
      toleranceCp: 300,
      minThinkMs: 500);
  static const easy = DifficultyLevel(
      name: '简单',
      depth: 8,
      multiPv: 4,
      movetimeMs: 1500,
      toleranceCp: 180,
      minThinkMs: 500);
  static const medium = DifficultyLevel(
      name: '中等',
      depth: 10,
      multiPv: 3,
      movetimeMs: 2000,
      toleranceCp: 100,
      minThinkMs: 600);
  static const hard = DifficultyLevel(
      name: '困难',
      depth: 14,
      multiPv: 2,
      movetimeMs: 2500,
      toleranceCp: 40,
      minThinkMs: 800);
  static const master = DifficultyLevel(
      name: '大师',
      // 深度 20 与提示/复盘分析同一评判标准；快设备 3s 内先触达时间上限，
      // 慢设备由深度 20 兜底（先到先停，跨设备表现一致）
      depth: 20,
      multiPv: 1,
      movetimeMs: 3000,
      fullStrength: true);

  static const all = [beginner, easy, medium, hard, master];
}

/// 引擎搜索结果
class EngineResult {
  const EngineResult({required this.move, required this.scoreCp});
  final Move move;
  /// 引擎局面评分（红方视角，单位厘兵）
  final int scoreCp;
}

/// 复盘分析结果：对某局面的引擎评估与最佳走法
class AnalysisResult {
  const AnalysisResult({
    required this.scoreCp,
    required this.bestMove,
    required this.pvMoves,
    this.pvList = const [],
  });

  /// 局面评分（红方视角，厘兵；mate 时为大分值）
  final int scoreCp;
  /// 引擎推荐走法（UCI）
  final String bestMove;
  /// 主变化（UCI 走法序列）
  final List<String> pvMoves;
  /// MultiPV 各路变化：[{move, scoreCp, pv}]（bestMove 即第一路）
  final List<({String move, int scoreCp, List<String> pv})> pvList;
}

/// 一次走棋请求参数（发送到 isolate）
class _GoRequest {
  const _GoRequest({
    required this.sendPort,
    required this.fen,
    required this.level,
    this.analysis = false,
    this.analysisDepth = 12,
    this.multiPv = 1,
    this.analysisMovetimeMs = 0,
  });
  final SendPort sendPort;
  final String fen;
  final DifficultyLevel level;
  /// true = 复盘分析模式（满强度）
  final bool analysis;
  /// 分析模式搜索深度
  final int analysisDepth;
  /// MultiPV 路数（提示功能用）
  final int multiPv;
  /// 分析模式思考时间上限（毫秒）；0 = 不限时，仅深度控制
  final int analysisMovetimeMs;
}

/// 皮卡鱼引擎管理类
class PikafishEngine {
  PikafishEngine._();
  static final PikafishEngine instance = PikafishEngine._();

  Isolate? _isolate;
  SendPort? _engineSendPort;

  /// 请求串行队列：主 isolate 侧保证同一时刻只有一个搜索在跑。
  /// 引擎 isolate 是单进程单线程，并发请求会被丢弃导致 UI 挂起，
  /// 故 think/analyze 统一在此排队。
  Future<void> _queue = Future.value();

  Future<T> _enqueue<T>(Future<T> Function() task) {
    final completer = Completer<void>();
    final prev = _queue;
    _queue = completer.future;
    return () async {
      await prev;
      try {
        return await task();
      } finally {
        completer.complete();
      }
    }();
  }

  /// 初始化引擎进程
  Future<void> start() async {
    if (_isolate != null) return;
    final exePath = await _resolveEngineExecutable();
    final nnuePath = await _copyNnueToTmp();
    final receivePort = ReceivePort();
    _isolate = await Isolate.spawn(
      _engineIsolateEntry,
      [receivePort.sendPort, exePath, nnuePath],
      debugName: 'pikafish',
    );
    // isolate 发回请求端口即视为就绪（此时 UCI 初始化命令已发出）
    _engineSendPort = await receivePort.first as SendPort;
  }

  /// 解析平台对应的引擎可执行文件路径
  Future<String> _resolveEngineExecutable() async {
    if (Platform.isAndroid) {
      final path = await const MethodChannel('tst_xiangqi/engine')
          .invokeMethod<String>('getEnginePath');
      return path!;
    }
    if (Platform.isIOS) {
      // iOS 沙盒不允许派生子进程，无法运行外部 UCI 引擎
      throw UnsupportedError('iOS 不支持外部引擎进程');
    }
    if (Platform.isMacOS) {
      final env = Platform.environment['TST_XIANGQI_ENGINE_PATH'];
      if (env != null && env.isNotEmpty) return env;
      // macOS：优先 bundle 内引擎（Contents/MacOS 与 Contents/engine/），
      // 其次工作目录（调试布局：项目根或 engine/ 子目录）。
      final exeDir = File(Platform.resolvedExecutable).parent.path;
      final candidates = [
        '$exeDir/pikafish',
        '$exeDir/engine/pikafish',
        'pikafish',
        'engine/pikafish',
      ];
      for (final c in candidates) {
        if (File(c).existsSync()) return c;
      }
      throw StateError('未找到 macOS 引擎，请将 pikafish 放入 App bundle（Contents/MacOS）或设置 TST_XIANGQI_ENGINE_PATH');
    }
    if (Platform.isWindows) {
      // Windows：优先在 exe 同目录找引擎（打包发布布局），
      // 其次工作目录（调试布局：项目根或 engine/ 子目录）。
      final exeDir = File(Platform.resolvedExecutable).parent.path;
      final candidates = [
        '$exeDir\\pikafish.exe',
        '$exeDir\\engine\\pikafish.exe',
        'pikafish.exe',
        'engine/pikafish.exe',
      ];
      for (final c in candidates) {
        if (File(c).existsSync()) return c;
      }
      throw StateError('未找到 Windows 引擎，请将 pikafish-*.exe 放到应用目录（或项目根目录）并重命名为 pikafish.exe');
    }
    throw UnsupportedError('不支持的平台');
  }

  /// 复制 NNUE 权重到引擎可访问的临时目录
  Future<String> _copyNnueToTmp() async {
    final dir = Directory.systemTemp;
    final nnue = File('${dir.path}${Platform.pathSeparator}pikafish.nnue');
    final data = await rootBundle.load('assets/engine/pikafish.nnue');
    final bytes = data.buffer.asUint8List();
    // 大小与内置权重一致则跳过写入，减少启动 IO；
    // writeAsBytes+flush 完整写入后大小恒定，损坏/截断的残留文件大小必然不同，
    // 权重升级体积变化也必然触发重写，确保 App 携带的新权重生效
    if (!await nnue.exists() || await nnue.length() != bytes.lengthInBytes) {
      await nnue.writeAsBytes(bytes, flush: true);
    }
    return nnue.path;
  }

  /// 请求引擎走棋（排队串行执行）
  Future<EngineResult> think(Board board, DifficultyLevel level) {
    return _enqueue(() async {
      await start();
      final sw = Stopwatch()..start();
      final response = ReceivePort();
      try {
        _engineSendPort!.send(_GoRequest(
          sendPort: response.sendPort,
          fen: board.fen,
          level: level,
        ));
        final result = await response.first
            .timeout(const Duration(seconds: 30), onTimeout: () {
          throw TimeoutException('引擎思考超时');
        }) as List;
        // 低档深度上限极浅（实测几十毫秒完成），补足最少思考时间，
        // 让落子节奏自然（观感上"引擎在思考"而非瞬间应答）
        final remain = level.minThinkMs - sw.elapsedMilliseconds;
        if (remain > 0) {
          await Future<void>.delayed(Duration(milliseconds: remain));
        }
        final uci = result[0] as String;
        final score = result[1] as int;
        final m = Move(
          uci.codeUnitAt(0) - 'a'.codeUnitAt(0),
          9 - int.parse(uci[1]),
          uci.codeUnitAt(2) - 'a'.codeUnitAt(0),
          9 - int.parse(uci[3]),
        );
        return EngineResult(move: m, scoreCp: score);
      } finally {
        // 超时/异常时也必须关闭，否则 ReceivePort 泄漏
        response.close();
      }
    });
  }

  /// 请求引擎分析（复盘用，满强度）。请求会排队串行执行。
  /// [multiPv] > 1 时返回多路最佳走法（提示功能用）。
  /// [movetimeMs] > 0 时为深度/时间双限（先到先停），兜底慢设备延迟。
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
    int movetimeMs = 0,
  }) {
    return _enqueue(() async {
      await start();
      final response = ReceivePort();
      try {
        _engineSendPort!.send(_GoRequest(
          sendPort: response.sendPort,
          fen: board.fen,
          level: DifficultyLevel.master,
          analysis: true,
          analysisDepth: depth,
          multiPv: multiPv,
          analysisMovetimeMs: movetimeMs,
        ));
        final result = await response.first
            .timeout(const Duration(seconds: 60), onTimeout: () {
          throw TimeoutException('引擎分析超时');
        }) as List;
        return AnalysisResult(
          scoreCp: result[0] as int,
          bestMove: result[1] as String,
          pvMoves: (result[2] as List).cast<String>(),
          pvList: (result[3] as List)
              .map((e) => (
                    move: e[0] as String,
                    scoreCp: e[1] as int,
                    pv: (e[2] as List).cast<String>(),
                  ))
              .toList(),
        );
      } finally {
        // 超时/异常时也必须关闭，否则 ReceivePort 泄漏
        response.close();
      }
    });
  }

  void dispose() {
    _engineSendPort?.send('quit');
    _isolate?.kill(priority: Isolate.immediate);
    _isolate = null;
    _engineSendPort = null;
  }
}

/// 线性加权随机：返回 [0, n) 下标，权重 n, n-1, ..., 1（越靠前越易被选中）。
int _pickWeighted(int n, math.Random rng) {
  var r = rng.nextInt(n * (n + 1) ~/ 2);
  for (var i = 0; i < n; i++) {
    if (r < n - i) return i;
    r -= n - i;
  }
  return n - 1;
}

/// 引擎 isolate 入口：与皮卡鱼进程保持长连接并处理走棋请求
void _engineIsolateEntry(List args) {
  final mainPort = args[0] as SendPort;
  final exePath = args[1] as String;
  final nnuePath = args[2] as String;
  final rng = math.Random();

  final process = Process.start(exePath, []);

  process.then((proc) {
    final requests = ReceivePort();
    mainPort.send(requests.sendPort);

    /// 当前请求的上下文（串行处理，一次只有一个在跑）
    _RequestContext? current;

    /// 引擎忙时挂起的新请求（主侧超时后会继续发新请求，需排队处理）
    final pending = <_GoRequest>[];

    /// 进程已退出：此后不再写 stdin（写入会抛异步异常）
    bool dead = false;

    void send(String s) {
      if (!dead) proc.stdin.writeln(s);
    }

    /// 处理一条走棋/分析请求
    void handleRequest(_GoRequest req) {
      current = _RequestContext(req);

      // 发送局面与搜索指令
      send('stop');
      send('position fen ${req.fen}');
      // 满强度 = 复盘分析或大师档；否则按档位 MultiPV 加权随机选路削弱
      final fullStrength = req.analysis || req.level.fullStrength;
      send('setoption name MultiPV value '
          '${fullStrength ? req.multiPv : req.level.multiPv}');
      send('isready');
      if (req.analysis) {
        // 深度为主，movetimeMs > 0 时叠加时间上限（先到先停）
        final t = req.analysisMovetimeMs > 0
            ? ' movetime ${req.analysisMovetimeMs}'
            : '';
        send('go depth ${req.analysisDepth}$t');
      } else {
        // 深度/时间双限，先到先停（depth 0 = 不限深度，仅时间控制）
        final depthLimit =
            req.level.depth > 0 ? 'depth ${req.level.depth} ' : '';
        send('go ${depthLimit}movetime ${req.level.movetimeMs}');
      }

      // 捕获当前 ctx（完成回调触发时 current 可能已被 failCurrent 置空）
      final ctxRef = current!;
      ctxRef.future.then((_) {
        final ctx = ctxRef;
        // 评分是“行棋方视角”，统一转为红方视角
        final board = Board.fromFen(req.fen);
        final flip = board.redToMove != true;

        int toRed(int s) => flip ? -s : s;

        final score = toRed(ctx.scores[1] ?? 0);
        final pv1 = ctx.pvs[1] ?? const <String>[];

        if (req.analysis) {
          // MultiPV 各路变化（按引擎行棋方视角转红方视角）
          final pvList = <List<dynamic>>[];
          for (final idx in (ctx.scores.keys.toList()..sort())) {
            final mv = ctx.pvs[idx]?.firstOrNull ?? '';
            if (mv.isEmpty) continue;
            pvList.add([mv, toRed(ctx.scores[idx] ?? 0), ctx.pvs[idx] ?? const []]);
          }
          req.sendPort.send([score, ctx.bestmove, pv1, pvList]);
        } else if (fullStrength) {
          req.sendPort.send([ctx.bestmove, score]);
        } else {
          // 难度档：从 MultiPV 候选中加权随机选路（棋力削弱的核心）。
          // 先按档位分差容差过滤（只选不明显劣于最佳的着法，避免选出大劣着），
          // 再按名次线性加权随机（越优越常被选）。
          final idxs = ctx.scores.keys.toList()..sort();
          final best = ctx.scores[1];
          final k = math.min(req.level.multiPv, idxs.length);
          final pool = <int>[];
          for (final i in idxs.take(k)) {
            if (best == null || best - (ctx.scores[i] ?? 0) <= req.level.toleranceCp) {
              pool.add(i);
            }
          }
          var chosenIdx = pool.isEmpty ? 1 : pool.first;
          if (pool.length > 1) {
            chosenIdx = pool[_pickWeighted(pool.length, rng)];
          }
          final mv = ctx.pvs[chosenIdx]?.firstOrNull;
          final chosenScore = ctx.scores[chosenIdx] ?? ctx.scores[1] ?? 0;
          req.sendPort.send([
            mv != null && mv.isNotEmpty ? mv : ctx.bestmove,
            toRed(chosenScore),
          ]);
        }
        current = null;
        // 旧请求结束后，若队列中还有新请求则继续处理
        if (pending.isNotEmpty) {
          handleRequest(pending.removeAt(0));
        }
      });
    }

    /// 完成当前请求（bestmove 缺失时以兜底结果完成，避免主侧挂起）
    void failCurrent() {
      final ctx = current;
      if (ctx == null || ctx.done) return;
      final req = ctx.req;
      // 无有效搜索结果时给出空/默认结果
      final board = Board.fromFen(req.fen);
      final moves = board.legalMoves();
      final uci = moves.isEmpty
          ? '0000'
          : moves[DateTime.now().millisecondsSinceEpoch % moves.length].uci;
      if (req.analysis) {
        req.sendPort.send([0, uci, <String>[], [
          if (uci != '0000') [uci, 0, <String>[]]
        ]]);
      } else {
        req.sendPort.send([uci, 0]);
      }
      ctx.complete();
      current = null;
    }

    // 统一的 stdout 监听（只建一次）
    proc.stdout
        .transform(const Utf8Decoder())
        .transform(const LineSplitter())
        .listen((line) {
      final ctx = current;
      if (ctx == null) return;
      if (line.startsWith('info') && line.contains(' score ')) {
        final m = RegExp(r'score (cp|mate) (-?\d+)').firstMatch(line);
        if (m != null) {
          final v = int.parse(m.group(2)!);
          // mate 转换：1 步绝杀 = ±9999，2 步 = ±9998，以此类推；
          // mate 0 表示当前行棋方已被绝杀（无棋可走），记为最深败势 -10000
          final score = m.group(1) == 'mate'
              ? (v == 0 ? -10000 : (v > 0 ? 10000 - v : -10000 - v))
              : v;
          // MultiPV 行号（无该字段 = 1）
          final pvIdx = int.tryParse(
                  RegExp(r'multipv (\d+)').firstMatch(line)?.group(1) ??
                      '1') ??
              1;
          ctx.scores[pvIdx] = score;
        }
        // 主变化（最后一条 info 的 pv 最深）
        final pvMatch = RegExp(r' pv ((?:\S+ )*\S+)').firstMatch(line);
        if (pvMatch != null) {
          final pvIdx = int.tryParse(
                  RegExp(r'multipv (\d+)').firstMatch(line)?.group(1) ??
                      '1') ??
              1;
          ctx.pvs[pvIdx] = pvMatch.group(1)!.split(' ');
        }
      } else if (line.startsWith('bestmove')) {
        final parts = line.split(' ');
        ctx.bestmove = parts.length > 1 ? parts[1] : '0000';
        ctx.complete();
      }
    });

    // 进程退出/崩溃：完成等待中的请求，避免主 isolate 挂起
    proc.exitCode.then((code) {
      debugPrint('[Pikafish] 引擎进程退出 exit=$code（非 0 或提前退出 = 启动失败/崩溃）');
      dead = true;
      requests.close();
      failCurrent();
      // 排队中的请求也无法处理，逐个兜底完成
      while (pending.isNotEmpty) {
        final req = pending.removeAt(0);
        final board = Board.fromFen(req.fen);
        final moves = board.legalMoves();
        final uci = moves.isEmpty
            ? '0000'
            : moves[DateTime.now().millisecondsSinceEpoch % moves.length].uci;
        if (req.analysis) {
          req.sendPort.send([0, uci, <String>[], [
            if (uci != '0000') [uci, 0, <String>[]]
          ]]);
        } else {
          req.sendPort.send([uci, 0]);
        }
      }
    });

    // 初始化
    send('uci');
    send('setoption name EvalFile value $nnuePath');
    // 多线程搜索：单线程会浪费多核算力，大师/分析档的满强度依赖足够算力
    // （留 1 核给主 isolate/UI，最多 8 线程防低配设备过热）
    final threads = math.max(1, math.min(8, Platform.numberOfProcessors - 1));
    send('setoption name Threads value $threads');
    send('setoption name Hash value 128');
    send('isready');

    requests.listen((msg) {
      if (msg == 'quit') {
        send('quit');
        proc.kill();
        requests.close();
        return;
      }
      final req = msg as _GoRequest;
      if (current != null && !current!.done) {
        // 引擎仍在搜索（主侧超时后会提前放行新请求）：
        // 令引擎尽快结束当前搜索，新请求排队待 bestmove 后接管
        send('stop');
        pending.add(req);
        return;
      }
      handleRequest(req);
    });
  }).catchError((e) {
    // 引擎启动失败：向主 isolate 报错（后续请求走随机走法兜底）
    debugPrint('[Pikafish] 引擎启动失败（常见于 x86 模拟器跑 arm64 引擎）: $e');
    final errPort = ReceivePort();
    mainPort.send(errPort.sendPort);
    errPort.listen((msg) {
      if (msg is _GoRequest) {
        // 无引擎时的随机走法回退
        debugPrint('[Pikafish] 无引擎，本步返回随机走法（AI 表现为"乱下"即此兜底生效）');
        final board = Board.fromFen(msg.fen);
        final moves = board.legalMoves();
        final mv = moves.isEmpty
            ? null
            : moves[DateTime.now().millisecondsSinceEpoch % moves.length];
        final uci = mv?.uci ?? '0000';
        if (msg.analysis) {
          // 无引擎时返回单路空结果
          msg.sendPort.send([0, uci, <String>[], [
            if (mv != null) [uci, 0, <String>[]]
          ]]);
        } else {
          msg.sendPort.send([uci, 0]);
        }
      }
    });
  });
}

/// isolate 内单个请求的上下文
class _RequestContext {
  _RequestContext(this.req);
  final _GoRequest req;
  /// 各 MultiPV 路的评分（行棋方视角；键 1 = 最佳）
  final Map<int, int> scores = {};
  /// 各 MultiPV 路的主变化
  final Map<int, List<String>> pvs = {};
  String bestmove = '0000';
  bool done = false;
  final Completer<void> _completer = Completer<void>();
  Future<void> get future => _completer.future;

  void complete() {
    if (done) return;
    done = true;
    _completer.complete();
  }
}
