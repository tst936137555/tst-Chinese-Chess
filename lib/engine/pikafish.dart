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

import 'package:flutter/services.dart';

import 'rules.dart';

/// 难度等级定义（映射到皮卡鱼 UCI_Elo / Skill Level）
class DifficultyLevel {
  const DifficultyLevel({
    required this.name,
    required this.elo,
    required this.depth,
    required this.movetimeMs,
    required this.skill,
  });

  final String name;
  final int elo;
  final int depth;
  final int movetimeMs;
  final int skill;

  /// Skill Level 0-20 削弱棋力（20 = 满强度）；movetime 控制每步思考时间。
  /// 注：UCI_LimitStrength=true 时引擎忽略手动 Skill Level，故 skill 体系下 elo 置 0。
  static const beginner = DifficultyLevel(name: '入门', elo: 0, depth: 0, movetimeMs: 1000, skill: 4);
  static const easy = DifficultyLevel(name: '简单', elo: 0, depth: 0, movetimeMs: 1250, skill: 8);
  static const medium = DifficultyLevel(name: '中等', elo: 0, depth: 0, movetimeMs: 1500, skill: 12);
  static const hard = DifficultyLevel(name: '困难', elo: 0, depth: 0, movetimeMs: 1750, skill: 16);
  static const master = DifficultyLevel(name: '大师', elo: 0, depth: 0, movetimeMs: 2000, skill: 20);

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
  });
  final SendPort sendPort;
  final String fen;
  final DifficultyLevel level;
  /// true = 复盘分析模式（满强度、不限 Elo）
  final bool analysis;
  /// 分析模式搜索深度
  final int analysisDepth;
  /// MultiPV 路数（提示功能用）
  final int multiPv;
}

/// 皮卡鱼引擎管理类
class PikafishEngine {
  PikafishEngine._();
  static final PikafishEngine instance = PikafishEngine._();

  Isolate? _isolate;
  SendPort? _engineSendPort;
  final _readyCompleter = Completer<void>();

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
    _engineSendPort = await receivePort.first as SendPort;
    await _readyCompleter.future;
  }

  /// 解析平台对应的引擎可执行文件路径
  Future<String> _resolveEngineExecutable() async {
    if (Platform.isAndroid) {
      final path = await const MethodChannel('xiangqi/engine')
          .invokeMethod<String>('getEnginePath');
      return path!;
    }
    if (Platform.isIOS || Platform.isMacOS) {
      final env = Platform.environment['XIANGQI_ENGINE_PATH'];
      if (env != null && env.isNotEmpty) return env;
      // 开发/调试回退（macOS bundle 内）
      return 'pikafish';
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
    if (!nnue.existsSync()) {
      final data = await rootBundle.load('assets/engine/pikafish.nnue');
      await nnue.writeAsBytes(data.buffer.asUint8List(), flush: true);
    }
    return nnue.path;
  }

  /// 请求引擎走棋（排队串行执行）
  Future<EngineResult> think(Board board, DifficultyLevel level) {
    return _enqueue(() async {
      await start();
      final response = ReceivePort();
      _engineSendPort!.send(_GoRequest(
        sendPort: response.sendPort,
        fen: board.fen,
        level: level,
      ));
      final result = await response.first
          .timeout(const Duration(seconds: 30), onTimeout: () {
        throw TimeoutException('引擎思考超时');
      }) as List;
      response.close();
      final uci = result[0] as String;
      final score = result[1] as int;
      final m = Move(
        uci.codeUnitAt(0) - 'a'.codeUnitAt(0),
        9 - int.parse(uci[1]),
        uci.codeUnitAt(2) - 'a'.codeUnitAt(0),
        9 - int.parse(uci[3]),
      );
      return EngineResult(move: m, scoreCp: score);
    });
  }

  /// 请求引擎分析（复盘用，满强度）。请求会排队串行执行。
  /// [multiPv] > 1 时返回多路最佳走法（提示功能用）。
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
  }) {
    return _enqueue(() async {
      await start();
      final response = ReceivePort();
      _engineSendPort!.send(_GoRequest(
        sendPort: response.sendPort,
        fen: board.fen,
        level: DifficultyLevel.master,
        analysis: true,
        analysisDepth: depth,
        multiPv: multiPv,
      ));
      final result = await response.first
          .timeout(const Duration(seconds: 60), onTimeout: () {
        throw TimeoutException('引擎分析超时');
      }) as List;
      response.close();
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
    });
  }

  void dispose() {
    _engineSendPort?.send('quit');
    _isolate?.kill(priority: Isolate.immediate);
    _isolate = null;
    _engineSendPort = null;
  }
}

/// 引擎 isolate 入口：与皮卡鱼进程保持长连接并处理走棋请求
void _engineIsolateEntry(List args) {
  final mainPort = args[0] as SendPort;
  final exePath = args[1] as String;
  final nnuePath = args[2] as String;

  final process = Process.start(exePath, []);

  process.then((proc) {
    final requests = ReceivePort();
    mainPort.send(requests.sendPort);

    /// 当前请求的上下文（串行处理，一次只有一个在跑）
    _RequestContext? current;

    void send(String s) => proc.stdin.writeln(s);

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
          // mate 转换：1 步绝杀 = ±9999，2 步 = ±9998，以此类推
          final score = m.group(1) == 'mate'
              ? (v > 0 ? 10000 - v : -10000 - v)
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
    proc.exitCode.then((_) {
      requests.close();
      failCurrent();
    });

    // 初始化
    send('uci');
    send('setoption name EvalFile value $nnuePath');
    send('setoption name Threads value 1');
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
      // 若上一请求尚未结束则忽略新请求（正常不会发生，主侧已排队）
      if (current != null && !current!.done) return;
      current = _RequestContext(req);

      // 发送局面与搜索指令
      send('stop');
      send('position fen ${req.fen}');
      if (req.analysis) {
        send('setoption name UCI_LimitStrength value false');
        send('setoption name Skill Level value 20');
        send('setoption name MultiPV value ${req.multiPv}');
      } else if (req.level.elo > 0) {
        send('setoption name UCI_LimitStrength value true');
        send('setoption name UCI_Elo value ${req.level.elo}');
        send('setoption name MultiPV value 1');
      } else {
        send('setoption name UCI_LimitStrength value false');
        send('setoption name Skill Level value ${req.level.skill}');
        send('setoption name MultiPV value 1');
      }
      send('isready');
      if (req.analysis) {
        send('go depth ${req.analysisDepth}');
      } else if (req.level.depth > 0) {
        send('go depth ${req.level.depth}');
      } else {
        send('go movetime ${req.level.movetimeMs}');
      }

      current!.future.then((_) {
        final ctx = current!;
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
        } else {
          req.sendPort.send([ctx.bestmove, score]);
        }
        current = null;
      });
    });
  }).catchError((e) {
    // 引擎启动失败：向主 isolate 报错
    final errPort = ReceivePort();
    mainPort.send(errPort.sendPort);
    errPort.listen((msg) {
      if (msg is _GoRequest) {
        // 无引擎时的随机走法回退
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
