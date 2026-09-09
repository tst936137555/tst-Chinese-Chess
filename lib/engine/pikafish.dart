/// 皮卡鱼 UCI 引擎封装：进程管理、启动握手、健康监测、崩溃恢复、走棋请求。
///
/// Android/macOS/Windows 经 Dart Isolate 内的 Process 与引擎以 UCI 行协议通信，
/// 通过 SendPort 将结果回传到主 isolate；
/// iOS 沙盒禁止子进程，改由进程内静态库 + FFI 传输（tool/setup_ios_engine.sh
/// 构建，未链接静态库时如实降级报错）。
///
/// 可靠性设计：
/// - 启动握手：isolate 完成 `uci`/`uciok` 与 `isready`/`readyok` 完整握手后
///   才向主 isolate 报告就绪；握手失败（进程起不来、提前退出、超时）报告失败。
/// - 健康通知：引擎进程退出（崩溃/被杀）通过健康端口通知主 isolate，
///   主 isolate 清空端口状态，下一次请求自动重启引擎（崩溃恢复）。
/// - 看门狗：isolate 侧对每个搜索设超时（时限 + move 判定挂死，杀掉进程触发重启恢
///   复，避免请求无限堆积。
/// - 失败如实传播：启动失败/崩溃/超时统一抛 [EngineUnavailableException]，
///   绝不以随机走法或 0 分伪造引擎结果。
///
/// 所有走棋/分析请求在主 isolate 侧串行排队，
/// 同一时刻只向引擎发送一个搜索指令，避免请求被丢弃。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:math' as math;


import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
// 原生内存分配器（dart:ffi 本体不含 malloc）
import 'package:ffi/ffi.dart' show malloc;

import 'rules.dart';

/// 引擎不可用异常：启动失败、进程崩溃、请求超时/无响应时抛出。
/// 调用方据此向用户呈现错误（提示/中止），而非用伪造结果冒充引擎输出。
class EngineUnavailableException implements Exception {
  const EngineUnavailableException(this.message);
  final String message;

  @override
  String toString() => '引擎不可用：$message';
}

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
      depth: 3,
      multiPv: 5,
      movetimeMs: 1000,
      toleranceCp: 300,
      minThinkMs: 500);
  static const easy = DifficultyLevel(
      name: '简单',
      depth: 5,
      multiPv: 4,
      movetimeMs: 1500,
      toleranceCp: 180,
      minThinkMs: 500);
  static const medium = DifficultyLevel(
      name: '中等',
      depth: 7,
      multiPv: 3,
      movetimeMs: 2000,
      toleranceCp: 100,
      minThinkMs: 600);
  static const hard = DifficultyLevel(
      name: '困难',
      depth: 9,
      multiPv: 2,
      movetimeMs: 2500,
      toleranceCp: 40,
      minThinkMs: 800);
  static const master = DifficultyLevel(
      name: '大师',
      // 深度 12 与提示/复盘分析同一评判标准；快设备 3s 内先触达时间上限，
      // 慢设备由深度 12 兜底（先到先停，跨设备表现一致）
      depth: 12,
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

/// isolate 就绪消息：UCI 握手完成后发回请求端口
/// （健康通知端口由主 isolate 预先创建并经启动参数传入，不经此消息）
class _IsolateReady {
  _IsolateReady(this.requestsPort);
  final SendPort requestsPort;
}

/// isolate 启动失败消息（进程起不来 / 握手超时 / 握手阶段退出）
class _IsolateFailed {
  _IsolateFailed(this.reason);
  final String reason;
}

/// 引擎进程退出通知（崩溃/被杀），主 isolate 据此清空状态、下次请求自动重启
class _EngineExited {
  _EngineExited(this.exitCode);
  final int exitCode;
}

/// 单个请求的失败应答（主 isolate 解码后转为 EngineUnavailableException）
class _RequestError {
  _RequestError(this.message);
  final String message;
}

/// 引擎客户端抽象：GameController/ReviewController 依赖此接口编程，
/// 测试注入伪造实现即可覆盖 AI 调度与故障路径，无需真实引擎二进制。
abstract interface class EngineClient {
  /// 启动引擎（幂等；崩溃后调用会重新拉起）
  Future<void> start();

  /// 请求引擎走棋
  Future<EngineResult> think(Board board, DifficultyLevel level);

  /// 请求引擎分析（[multiPv] > 1 时返回多路候选；[movetimeMs] > 0 时深度/时间双限）
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
    int movetimeMs = 0,
  });

  /// 释放资源
  void dispose();
}

/// 皮卡鱼引擎管理类
class PikafishEngine implements EngineClient {
  PikafishEngine._();
  static final PikafishEngine instance = PikafishEngine._();

  /// 测试专用：非空时跳过 Isolate.spawn，以该工厂创建内存引擎传输并
  /// 在当前 isolate 运行会话（握手/看门狗/崩溃恢复协议与生产完全一致）。
  /// 生产代码永远为 null。
  @visibleForTesting
  EngineIoFactory? debugIoFactory;

  /// 测试专用：引擎会话是否就绪。崩溃/退出并经健康通知清空状态后为 false，
  /// 用于验证"崩溃 → 主侧感知"契约（生产恢复路径为下次请求自动重启）。
  @visibleForTesting
  bool get debugEngineReady => _engineSendPort != null;

  Isolate? _isolate;
  SendPort? _engineSendPort;
  ReceivePort? _healthPort;

  /// 进行中的启动（并发请求合并为一次 spawn + 握手）
  Future<void>? _starting;

  /// 连续启动失败次数：达到上限后快速失败，
  /// 避免引擎损坏时每个请求都完整等待握手超时（请求堆积的根源之一）
  static const _maxStartFailures = 3;
  int _startFailures = 0;

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

  /// 启动引擎进程（幂等）。
  /// 已就绪直接返回；崩溃后（端口已清空）调用会重新拉起引擎（崩溃恢复入口）。
  @override
  Future<void> start() async {
    if (_engineSendPort != null) return;
    final starting = _starting;
    if (starting != null) return starting;
    if (_startFailures >= _maxStartFailures) {
      throw const EngineUnavailableException(
          '引擎连续启动失败，已停止重试（请检查引擎文件后重启应用）');
    }
    final fut = _doStart();
    _starting = fut;
    try {
      await fut;
    } finally {
      _starting = null;
    }
  }

  Future<void> _doStart() async {
    String exePath;
    String nnuePath;
    try {
      exePath = await _resolveEngineExecutable();
      nnuePath = await _copyNnueToTmp();
    } catch (e) {
      _startFailures++;
      throw EngineUnavailableException('引擎初始化失败: $e');
    }

    final readyPort = ReceivePort();
    final errorPort = ReceivePort();
    // 健康端口由主 isolate 持有，句柄传入引擎 isolate：
    // 引擎进程退出（崩溃/被杀）经它通知主 isolate（此前 isolate 自建端口
    // 的 sendPort 无法跨端口投递到主侧，通知会静默丢失）
    final healthPort = ReceivePort();
    Isolate? isolate;
    try {
      final handshake = Completer<Object>();
      readyPort.listen((m) {
        if (!handshake.isCompleted) handshake.complete(m as Object);
      });
      errorPort.listen((m) {
        if (!handshake.isCompleted) {
          handshake.complete(_IsolateFailed('引擎 isolate 内部错误: $m'));
        }
      });

      final testFactory = debugIoFactory;
      if (testFactory != null) {
        // 测试模式：会话在当前 isolate 内运行（跳过 Isolate.spawn），
        // 失败同样经 _IsolateFailed 消息通知主侧
        unawaited(() async {
          try {
            await _runEngineSession(testFactory, readyPort.sendPort, nnuePath,
                () {}, healthPort.sendPort);
          } catch (e) {
            if (!handshake.isCompleted) {
              handshake.complete(_IsolateFailed('测试会话异常: $e'));
            }
          }
        }());
      } else {
        isolate = await Isolate.spawn(
          _engineIsolateEntry,
          [readyPort.sendPort, exePath, nnuePath, healthPort.sendPort],
          debugName: 'pikafish',
          onError: errorPort.sendPort,
          errorsAreFatal: true,
        );
      }

      // 等待握手结果：就绪消息（含端口）或失败消息，二者先到者生效；
      // isolate 内部错误也转为失败，避免傻等超时
      final msg = await handshake.future.timeout(
        const Duration(seconds: 30),
        onTimeout: () => _IsolateFailed('引擎启动握手超时（30 秒无响应）'),
      );

      if (msg is _IsolateFailed) {
        throw EngineUnavailableException(msg.reason);
      }
      final ready = msg as _IsolateReady;

      _isolate = isolate;
      _engineSendPort = ready.requestsPort;
      // 健康监测：引擎进程退出（崩溃/被杀）→ 清空状态，下次请求自动重启
      _healthPort = healthPort;
      _healthPort!.listen((m) {
        if (m is _EngineExited) {
          debugPrint('[Pikafish] 引擎进程退出 exit=${m.exitCode}，下次请求自动重启');
          _resetIsolate();
        }
      });
      _startFailures = 0;
    } catch (e) {
      isolate?.kill(priority: Isolate.immediate);
      // 启动失败且健康端口未被采纳时关闭，避免泄漏
      if (_healthPort != healthPort) healthPort.close();
      _startFailures++;
      throw e is EngineUnavailableException
          ? e
          : EngineUnavailableException('引擎启动失败: $e');
    } finally {
      readyPort.close();
      errorPort.close();
    }
  }

  /// 清空 isolate 相关状态（崩溃恢复 / dispose 共用）。
  /// 之后调用 [start] 会重新 spawn 引擎。
  void _resetIsolate() {
    _engineSendPort = null;
    _healthPort?.close();
    _healthPort = null;
    _isolate?.kill(priority: Isolate.immediate);
    _isolate = null;
  }

  /// 请求级崩溃恢复：请求过程中引擎崩溃/挂死时，重置并重启引擎后重试一次。
  /// 重试仍失败则向上抛 [EngineUnavailableException]（调用方呈现错误，不伪造结果）。
  Future<T> _withRecovery<T>(Future<T> Function() action) async {
    try {
      return await action();
    } on EngineUnavailableException {
      debugPrint('[Pikafish] 请求失败，重置引擎并重试一次（崩溃恢复）');
      _resetIsolate();
      await start();
      return action();
    }
  }

  /// 发送一次走棋/分析请求并等待应答。
  /// 引擎侧故障以 [_RequestError] 应答，此处统一转为 [EngineUnavailableException]。
  Future<Object> _request({
    required String fen,
    required DifficultyLevel level,
    bool analysis = false,
    int analysisDepth = 12,
    int multiPv = 1,
    int analysisMovetimeMs = 0,
    required Duration timeout,
  }) async {
    final port = _engineSendPort;
    if (port == null) {
      throw const EngineUnavailableException('引擎未就绪');
    }
    final response = ReceivePort();
    try {
      port.send(_GoRequest(
        sendPort: response.sendPort,
        fen: fen,
        level: level,
        analysis: analysis,
        analysisDepth: analysisDepth,
        multiPv: multiPv,
        analysisMovetimeMs: analysisMovetimeMs,
      ));
      final result = await response.first.timeout(timeout, onTimeout: () {
        throw EngineUnavailableException('引擎请求超时（${timeout.inSeconds}s 无应答）');
      });
      if (result is _RequestError) {
        throw EngineUnavailableException(result.message);
      }
      return result as Object;
    } finally {
      // 超时/异常时也必须关闭，否则 ReceivePort 泄漏
      response.close();
    }
  }

  /// 解析平台对应的引擎可执行文件路径
  Future<String> _resolveEngineExecutable() async {
    if (Platform.isAndroid) {
      final path = await const MethodChannel('tst_xiangqi/engine')
          .invokeMethod<String>('getEnginePath');
      return path!;
    }
    if (Platform.isIOS) {
      // iOS 沙盒不允许派生子进程：改用进程内静态库 + FFI（传输层 _FfiIo）。
      // 静态库由 tool/setup_ios_engine.sh 在 macOS 构建并链入主可执行文件；
      // 未链接时如实降级报错（不伪造结果）。
      if (_FfiIo.supported) return _ffiMarker;
      throw UnsupportedError(
          'iOS 引擎静态库未链接（请在 macOS 运行 tool/setup_ios_engine.sh）');
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

  /// 请求引擎走棋（排队串行执行）。
  /// 引擎不可用（启动失败/崩溃且无法恢复）时抛 [EngineUnavailableException]。
  @override
  Future<EngineResult> think(Board board, DifficultyLevel level) {
    return _enqueue(() async {
      // 启动失败不重试（环境性问题重试无意义），直接抛给调用方
      await start();
      // 请求过程中崩溃/无响应：重置引擎重试一次（崩溃恢复）
      return _withRecovery(() async {
        final sw = Stopwatch()..start();
        final result = await _request(
          fen: board.fen,
          level: level,
          // 主侧超时须晚于 isolate 侧看门狗（时限+8s），
          // 让挂死请求优先走"报错+重启引擎"路径，此处只是最终防线
          timeout: Duration(milliseconds: level.movetimeMs + 12000),
        ) as List;
        // 低档深度上限极浅（实测几十毫秒完成），补足最少思考时间，
        // 让落子节奏自然（观感上"引擎在思考"而非瞬间应答）
        final remain = level.minThinkMs - sw.elapsedMilliseconds;
        if (remain > 0) {
          await Future<void>.delayed(Duration(milliseconds: remain));
        }
        final uci = result[0] as String;
        final Move m;
        try {
          m = Move.fromUci(uci);
        } catch (_) {
          throw EngineUnavailableException('引擎返回无效走法: $uci');
        }
        return EngineResult(move: m, scoreCp: result[1] as int);
      });
    });
  }

  /// 请求引擎分析（复盘用，满强度）。请求会排队串行执行。
  /// [multiPv] > 1 时返回多路最佳走法（提示功能用）。
  /// [movetimeMs] > 0 时为深度/时间双限（先到先停），兜底慢设备延迟。
  /// 引擎不可用（启动失败/崩溃且无法恢复）时抛 [EngineUnavailableException]。
  @override
  Future<AnalysisResult> analyze(
    Board board, {
    int depth = 12,
    int multiPv = 1,
    int movetimeMs = 0,
  }) {
    return _enqueue(() async {
      await start();
      return _withRecovery(() async {
        final result = await _request(
          fen: board.fen,
          level: DifficultyLevel.master,
          analysis: true,
          analysisDepth: depth,
          multiPv: multiPv,
          analysisMovetimeMs: movetimeMs,
          timeout: Duration(
              milliseconds:
                  movetimeMs > 0 ? movetimeMs + 12000 : 150000),
        ) as List;
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
    });
  }

  @override
  void dispose() {
    // 先尝试优雅退出，再强制清理；启动失败计数复位，允许再次 start
    _engineSendPort?.send('quit');
    _resetIsolate();
    _startFailures = 0;
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

/// isolate 内单个请求的上下文
class _RequestContext {
  _RequestContext(this.req);
  final _GoRequest req;
  /// 各 MultiPV 路的评分（行棋方视角；键 1 = 最佳）
  final Map<int, int> scores = {};
  /// 各 MultiPV 路的主变化
  final Map<int, List<String>> pvs = {};
  String bestmove = '0000';
  /// 是否已向主 isolate 发送应答（结果或错误）
  bool done = false;
}

/// 引擎 isolate 入口：按平台选择传输层（子进程 / iOS FFI），运行引擎会话。
Future<void> _engineIsolateEntry(List args) async {
  final mainPort = args[0] as SendPort;
  final exePath = args[1] as String;
  final healthOut = args[3] as SendPort;
  final EngineIoFactory ioFactory = exePath == _ffiMarker
      ? _FfiIo.start
      : (
          {required void Function(String line) onLine,
          required void Function(int code) onExit}) =>
          _ProcessIo.start(exePath, onLine: onLine, onExit: onExit);
  await _runEngineSession(
    ioFactory,
    mainPort,
    args[2] as String,
    () => Isolate.current.kill(priority: Isolate.immediate),
    healthOut,
  );
}

/// 引擎会话主体：启动引擎传输、完成 UCI 握手，随后串行处理走棋请求。
/// 握手完成前不向主 isolate 报就绪；进程崩溃经 [healthOut] 通知主 isolate。
///
/// [fatalExit] 为会话致命退出（请求已兜底应答后的收尾）：
/// 生产环境结束引擎 isolate；测试在当前 isolate 内驱动时为 no-op。
Future<void> _runEngineSession(
  EngineIoFactory ioFactory,
  SendPort mainPort,
  String nnuePath,
  void Function() fatalExit,
  SendPort healthOut,
) async {
  final rng = math.Random();

  /// 启动失败：通知主 isolate 后结束 isolate（幂等）
  bool failed = false;
  void fail(String reason) {
    if (failed) return;
    failed = true;
    debugPrint('[Pikafish] 启动失败: $reason');
    mainPort.send(_IsolateFailed(reason));
  }

  bool dead = false;
  final handshakeComplete = Completer<void>();
  final uciok = Completer<void>();
  final readyok = Completer<void>();

  // 服务阶段共享状态（前置声明：统一退出回调 onEngineExit 需跨阶段访问）
  final requests = ReceivePort();
  _RequestContext? current;

  /// 引擎忙时挂起的新请求（仅主侧超时提前放行时才会出现，上限防堆积）
  final pending = <_GoRequest>[];
  Timer? watchdog;

  void cancelWatchdog() {
    watchdog?.cancel();
    watchdog = null;
  }

  late EngineIo io;

  void send(String s) {
    if (dead) return;
    io.send(s);
  }

  // 当前阶段的引擎输出行处理器：先握手，握手完成后切换为搜索结果路由
  void Function(String line)? onLine;
  void routeLine(String line) {
    final h = onLine;
    if (h != null) h(line);
  }

  /// 引擎退出统一处理（子进程退出 / 进程内引擎线程结束）：
  /// 握手阶段 = 启动失败；服务阶段 = 通知主 isolate 崩溃恢复并兜底应答。
  ///
  /// dead 已置位（quit / 看门狗主动杀进程）时不再重复通知主 isolate、
  /// 不重复收尾，但仍为尚未应答的挂起请求补发错误，避免其傻等主侧超时。
  void onEngineExit(int code) {
    cancelWatchdog();
    final wasDead = dead;
    dead = true;
    if (!handshakeComplete.isCompleted) {
      fail('引擎在握手阶段退出（exit=$code）');
      return;
    }
    if (!wasDead) {
      debugPrint('[Pikafish] 引擎进程退出 exit=$code');
      // 先通知主 isolate（崩溃恢复），再兜底应答，最后结束 isolate
      healthOut.send(_EngineExited(code));
    }
    final err = _RequestError('引擎进程已退出（exit=$code）');
    final ctx = current;
    if (ctx != null && !ctx.done) {
      ctx.done = true;
      ctx.req.sendPort.send(err);
    }
    current = null;
    while (pending.isNotEmpty) {
      pending.removeAt(0).sendPort.send(err);
    }
    if (!wasDead) {
      requests.close();
      fatalExit();
    }
  }

  try {
    io = await ioFactory(onLine: routeLine, onExit: onEngineExit);
  } catch (e) {
    fail('引擎启动失败: $e');
    return;
  }

  onLine = (line) {
    if (line == 'uciok' && !uciok.isCompleted) uciok.complete();
    if (line == 'readyok' && !readyok.isCompleted) readyok.complete();
  };

  try {
    send('uci');
    await uciok.future.timeout(const Duration(seconds: 15),
        onTimeout: () => throw TimeoutException('等待 uciok 超时'));
    send('setoption name EvalFile value $nnuePath');
    // 多线程搜索：单线程会浪费多核算力，大师/分析档的满强度依赖足够算力
    // （留 1 核给主 isolate/UI，最多 8 线程防低配设备过热）
    final threads = math.max(1, math.min(8, Platform.numberOfProcessors - 1));
    send('setoption name Threads value $threads');
    send('setoption name Hash value 128');
    send('isready');
    await readyok.future.timeout(const Duration(seconds: 15),
        onTimeout: () => throw TimeoutException('等待 readyok 超时'));
  } catch (e) {
    dead = true;
    io.kill();
    fail('UCI 握手失败: $e');
    return;
  }
  handshakeComplete.complete();

  // ---- 阶段二：服务请求（requests/current/pending/watchdog 已在启动
  // 前前置声明，供统一退出回调 onEngineExit 跨阶段访问）----
  mainPort.send(_IsolateReady(requests.sendPort));

  /// 引擎故障（无响应/挂死）：终止引擎触发重启恢复，
  /// 由统一的 onEngineExit 回调负责通知主 isolate 并兜底应答剩余请求
  void killEngine(String reason) {
    if (dead) return;
    debugPrint('[Pikafish] $reason，终止引擎');
    dead = true;
    io.kill();
  }

  /// 处理一条走棋/分析请求：发送局面与搜索指令，并启动看门狗
  void handleRequest(_GoRequest req) {
    final ctx = _RequestContext(req);
    current = ctx;

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

    // 看门狗：超过搜索时限 + 8s 仍未收到 bestmove → 判定引擎挂死。
    // 皮卡鱼自行掐表、到点即输出 bestmove（误差毫秒级），8s 余量已足够
    // 覆盖慢设备调度抖动；该请求以错误应答并杀掉进程，
    // 避免 current 卡死导致后续请求无限堆积。
    final movetimeMs = req.analysis
        ? (req.analysisMovetimeMs > 0 ? req.analysisMovetimeMs : 120000)
        : req.level.movetimeMs;
    watchdog = Timer(Duration(milliseconds: movetimeMs + 8000), () {
      if (current != ctx || ctx.done) return;
      cancelWatchdog();
      ctx.done = true;
      current = null;
      req.sendPort.send(_RequestError('引擎无响应（看门狗超时）'));
      killEngine('引擎看门狗超时（未收到 bestmove）');
    });
  }

  /// 完成当前请求：把搜索结果发给主 isolate，然后继续处理队列
  void finishCurrent() {
    final ctx = current;
    if (ctx == null || ctx.done) return;
    cancelWatchdog();
    ctx.done = true;
    current = null;

    final req = ctx.req;
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
    } else if (req.level.fullStrength) {
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
        if (best == null ||
            best - (ctx.scores[i] ?? 0) <= req.level.toleranceCp) {
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
    // 旧请求结束后，若队列中还有新请求则继续处理
    if (pending.isNotEmpty) {
      handleRequest(pending.removeAt(0));
    }
  }

  /// 搜索结果行路由（服务阶段）
  void handleEngineLine(String line) {
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
                RegExp(r'multipv (\d+)').firstMatch(line)?.group(1) ?? '1') ??
            1;
        ctx.scores[pvIdx] = score;
      }
      // 主变化（最后一条 info 的 pv 最深）
      final pvMatch = RegExp(r' pv ((?:\S+ )*\S+)').firstMatch(line);
      if (pvMatch != null) {
        final pvIdx = int.tryParse(
                RegExp(r'multipv (\d+)').firstMatch(line)?.group(1) ?? '1') ??
            1;
        ctx.pvs[pvIdx] = pvMatch.group(1)!.split(' ');
      }
    } else if (line.startsWith('bestmove')) {
      final parts = line.split(' ');
      ctx.bestmove = parts.length > 1 ? parts[1] : '0000';
      finishCurrent();
    }
  }

  onLine = handleEngineLine;

  // 服务阶段引擎退出：由统一的 onEngineExit 处理（通知主 isolate 触发重启
  // 恢复，所有等待请求以错误应答，见启动前的前置声明）

  requests.listen((msg) {
    if (msg == 'quit') {
      dead = true;
      io.kill();
      requests.close();
      fatalExit();
      return;
    }
    if (dead) {
      // 进程已死但退出回调尚未跑完：直接以错误应答
      (msg as _GoRequest).sendPort.send(_RequestError('引擎进程已退出'));
      return;
    }
    final req = msg as _GoRequest;
    if (current != null && !current!.done) {
      // 引擎仍在搜索（主侧超时后会提前放行新请求）：
      // 令引擎尽快结束当前搜索，新请求排队待 bestmove 后接管
      send('stop');
      if (pending.length < 8) {
        pending.add(req);
      } else {
        req.sendPort.send(_RequestError('引擎请求积压，已拒绝'));
      }
      return;
    }
    handleRequest(req);
  });
}

// =============================================================================
// 传输层：子进程传输（Android/macOS/Windows）与进程内 FFI 传输（iOS）
//
// 两者以相同抽象对接引擎会话的 UCI 行协议处理，
// 握手/看门狗/请求路由/MultiPV 解析完全不感知传输差异。
// =============================================================================

/// iOS 进程内引擎标记：exePath 位不承载路径，改选 FFI 传输层。
const String _ffiMarker = '__ffi__';

/// 引擎传输层工厂：生产为进程/FFI 实现，测试可注入内存伪造实现
/// （伪造实现随 [PikafishEngine.debugIoFactory] 注入，无需真实引擎二进制）。
typedef EngineIoFactory = Future<EngineIo> Function({
  required void Function(String line) onLine,
  required void Function(int code) onExit,
});

/// 引擎 I/O 抽象：发送 UCI 命令行 + 终止引擎。
/// 输出行与退出事件在构造时（各 start 工厂）以回调注册。
abstract class EngineIo {
  void send(String line);
  void kill();
}

/// 子进程传输：Android（jniLibs 解压路径）/ macOS / Windows。
class _ProcessIo implements EngineIo {
  _ProcessIo._(this._proc);

  final Process _proc;

  static Future<_ProcessIo> start(
    String exePath, {
    required void Function(String line) onLine,
    required void Function(int code) onExit,
  }) async {
    final proc = await Process.start(exePath, []);
    proc.stdout
        .transform(const Utf8Decoder())
        .transform(const LineSplitter())
        .listen(onLine);
    proc.exitCode.then(onExit);
    return _ProcessIo._(proc);
  }

  @override
  void send(String line) {
    try {
      _proc.stdin.writeln(line);
    } catch (_) {
      // 进程已退出：写入失败，由退出回调走失败/崩溃通道
    }
  }

  @override
  void kill() => _proc.kill();
}

/// 进程内 FFI 传输（iOS）：引擎静态库经 tool/setup_ios_engine.sh 链入
/// Runner 主可执行文件，C 适配层（ios/EngineShim/）在独立原生线程上驱动
/// Stockfish::main（上游 -DUNIVERSAL_BINARY 收编的原 main）并把
/// stdin/stdout 重定向为行队列。
///
/// - 输出行经 NativeCallable.listener 投递：回调来自引擎原生线程（非
///   isolate 线程），listener 跨线程转投 isolate 事件循环，行处理逻辑
///   与子进程模式完全一致。
/// - 'quit' 由适配层注入引擎输入队列优雅收线；失去进程隔离是已知代价
///   （引擎线程真挂死时无法强杀，见 iOS FFI 可行性评估）。
/// - NativeCallable 不显式 close：close 后原生线程仍回调属未定义行为，
///   保持引用至 isolate 终止，由运行时统一回收。
class _FfiIo implements EngineIo {
  _FfiIo._(this._b, this._lineCb, this._exitCb);

  final _FfiBindings _b;

  // ignore: unused_field — 存活引用（防垃圾回收/误报，见类注释）
  final NativeCallable<_SfLineCbC> _lineCb;

  // ignore: unused_field — 存活引用（防垃圾回收/误报，见类注释）
  final NativeCallable<_SfExitCbC> _exitCb;

  /// 静态库是否已链入主可执行文件（符号探测；未链接时 iOS 优雅降级）
  static bool get supported => _FfiBindings.instance != null;

  static Future<_FfiIo> start({
    required void Function(String line) onLine,
    required void Function(int code) onExit,
  }) async {
    final b = _FfiBindings.instance;
    if (b == null) {
      throw StateError('引擎静态库未链接（缺少 sf_start 等符号）');
    }
    // 行缓冲由适配层 malloc 分配，Dart 复制字符串后立即释放
    final lineCb = NativeCallable<_SfLineCbC>.listener((Pointer<Uint8> p) {
      final line = _readCString(p);
      b.free(p.cast<Void>());
      onLine(line);
    });
    final exitCb = NativeCallable<_SfExitCbC>.listener(onExit);
    final rc = b.start(lineCb.nativeFunction, exitCb.nativeFunction);
    if (rc != 0) {
      lineCb.close();
      exitCb.close();
      throw StateError('sf_start 失败（rc=$rc，上一轮引擎可能仍在运行）');
    }
    return _FfiIo._(b, lineCb, exitCb);
  }

  static String _readCString(Pointer<Uint8> p) {
    var len = 0;
    while (p[len] != 0) {
      len++;
    }
    return utf8.decode(p.asTypedList(len), allowMalformed: true);
  }

  @override
  void send(String line) {
    final bytes = utf8.encode(line);
    final p = malloc<Uint8>(bytes.length + 1);
    try {
      p.asTypedList(bytes.length).setAll(0, bytes);
      p[bytes.length] = 0;
      _b.send(p);
    } finally {
      malloc.free(p);
    }
  }

  @override
  void kill() {
    // 优雅收线：注入 quit 令 UCI 主循环返回；2s 内未结束则由适配层放弃
    _b.stop();
  }
}

/// C 适配层绑定（接口见 ios/EngineShim/pikafish_shim.h）。
class _FfiBindings {
  _FfiBindings._(this.start, this.send, this.stop, this.free);

  final _SfStartD start;
  final _SfSendD send;
  final _SfStopD stop;
  final _SfFreeD free;

  static _FfiBindings? _cache;

  /// 探测并缓存绑定；符号缺失（静态库未链接）返回 null，绝不抛出。
  static _FfiBindings? get instance {
    final cached = _cache;
    if (cached != null) return cached;
    try {
      // 静态链接：符号位于主可执行文件（Flutter 官方 FFI 静态链接方式，
      // 配合 Xcode OTHER_LDFLAGS 的 -Wl,-force_load 防止链接器裁剪）
      final lib = DynamicLibrary.executable();
      return _cache = _FfiBindings._(
        lib.lookupFunction<_SfStartC, _SfStartD>('sf_start'),
        lib.lookupFunction<_SfSendC, _SfSendD>('sf_send'),
        lib.lookupFunction<_SfStopC, _SfStopD>('sf_stop'),
        lib.lookupFunction<_SfFreeC, _SfFreeD>('sf_free'),
      );
    } catch (_) {
      return null;
    }
  }
}

// —— C 适配层签名（与 pikafish_shim.h 一一对应）——
typedef _SfLineCbC = Void Function(Pointer<Uint8> line);
typedef _SfExitCbC = Void Function(Int32 code);
typedef _SfStartC = Int32 Function(
    Pointer<NativeFunction<_SfLineCbC>> onLine,
    Pointer<NativeFunction<_SfExitCbC>> onExit);
typedef _SfStartD = int Function(
    Pointer<NativeFunction<_SfLineCbC>> onLine,
    Pointer<NativeFunction<_SfExitCbC>> onExit);
typedef _SfSendC = Int32 Function(Pointer<Uint8> line);
typedef _SfSendD = int Function(Pointer<Uint8> line);
typedef _SfStopC = Void Function();
typedef _SfStopD = void Function();
typedef _SfFreeC = Void Function(Pointer<Void> ptr);
typedef _SfFreeD = void Function(Pointer<Void> ptr);
