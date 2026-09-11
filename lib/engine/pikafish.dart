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

part 'pikafish_isolate.dart';
part 'pikafish_io.dart';

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
    // 测试工厂在解析前判定：内存传输不需要引擎可执行文件与 NNUE 权重，
    // 跳过平台相关路径解析（无引擎二进制的 Linux CI 可完整运行可靠性测试）
    final testFactory = debugIoFactory;
    String exePath;
    String nnuePath;
    if (testFactory != null) {
      exePath = 'debug';
      nnuePath = 'debug';
    } else {
      try {
        exePath = await _resolveEngineExecutable();
        nnuePath = await _resolveNnuePath();
      } catch (e) {
        _startFailures++;
        throw EngineUnavailableException('引擎初始化失败: $e');
      }
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

  /// NNUE 资产路径（避免散落的字符串字面量）
  static const _nnueAssetKey = 'assets/engine/pikafish.nnue';

  /// 解析 NNUE 权重路径（按平台择优，避免无谓的整块复制）：
  /// - 桌面/iOS：直读安装包内 flutter_assets 原文件（零复制、零峰值内存）；
  /// - Android：资产封在 APK 内，引擎子进程无法直读，走平台通道流式复制
  ///   （MainActivity 侧 64KB 分段写盘，文件名带 lastUpdateTime 指纹）；
  /// - 以上不可用时回退内存复制（[_copyNnueToTmp]），任何布局变化下仍可用。
  Future<String> _resolveNnuePath() async {
    if (Platform.isAndroid) {
      try {
        final path = await const MethodChannel('tst_xiangqi/engine')
            .invokeMethod<String>('getNnuePath');
        if (path != null && path.isNotEmpty) return path;
      } catch (e) {
        debugPrint('[Pikafish] NNUE 平台通道复制失败，回退内存复制: $e');
      }
    } else {
      final direct = _directAssetNnuePath();
      if (direct != null) return direct;
    }
    return _copyNnueToTmp();
  }

  /// 尝试定位安装包内 flutter_assets 的 NNUE 原文件（桌面 / iOS）。
  /// 路径候选兼容不同 Flutter 版本的 bundle 布局；均不存在返回 null。
  String? _directAssetNnuePath() {
    final exeDir = File(Platform.resolvedExecutable).parent.path;
    final List<String> candidates;
    if (Platform.isMacOS) {
      // Contents/MacOS/exe → Contents/Frameworks/App.framework/.../flutter_assets
      candidates = [
        '$exeDir/../Frameworks/App.framework/Versions/A/Resources/flutter_assets/$_nnueAssetKey',
        '$exeDir/../Frameworks/App.framework/Resources/flutter_assets/$_nnueAssetKey',
      ];
    } else if (Platform.isIOS) {
      // <Bundle>/exe → <Bundle>/Frameworks/App.framework/flutter_assets
      candidates = [
        '$exeDir/Frameworks/App.framework/flutter_assets/$_nnueAssetKey',
        '$exeDir/../Frameworks/App.framework/flutter_assets/$_nnueAssetKey',
      ];
    } else if (Platform.isWindows) {
      // <exe 目录>/data/flutter_assets
      candidates = [
        '$exeDir\\data\\flutter_assets\\$_nnueAssetKey',
        '$exeDir/data/flutter_assets/$_nnueAssetKey',
      ];
    } else {
      return null;
    }
    for (final c in candidates) {
      if (File(c).existsSync()) return c;
    }
    return null;
  }

  /// 回退路径：复制 NNUE 权重到引擎可访问的临时目录。
  ///
  /// 临时文件名携带构建指纹（主可执行文件的修改时间）：
  /// - 同一次安装内已落盘即直接复用，不再把 ~40MB 资产全量读入内存；
  /// - 应用升级（NNUE 随包更新）后指纹变化，必然重新复制，确保新权重生效；
  /// - 先写 .part 再改名：崩溃中断不会残留同名半截文件被误复用。
  Future<String> _copyNnueToTmp() async {
    final stamp = File(Platform.resolvedExecutable)
        .lastModifiedSync()
        .millisecondsSinceEpoch;
    final path =
        '${Directory.systemTemp.path}${Platform.pathSeparator}pikafish-$stamp.nnue';
    if (File(path).existsSync()) return path;
    final bytes = (await rootBundle.load(_nnueAssetKey)).buffer.asUint8List();
    final part = File('$path.part');
    await part.writeAsBytes(bytes, flush: true);
    await part.rename(path);
    _cleanupStaleNnue(File(path));
    return path;
  }

  /// 清理其他构建指纹的残留 NNUE 与未完成的 .part（尽力而为，失败不影响启动）
  static void _cleanupStaleNnue(File keep) {
    try {
      for (final entity in Directory.systemTemp.listSync()) {
        final name = entity.path.split(Platform.pathSeparator).last;
        final isNnue = name.startsWith('pikafish-') &&
            (name.endsWith('.nnue') || name.endsWith('.nnue.part'));
        if (entity is File && isNnue && entity.path != keep.path) {
          try {
            entity.deleteSync();
          } catch (_) {
            // 单个文件删除失败（被占用等）：跳过，下次启动再清
          }
        }
      }
    } catch (_) {}
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
        // 坐标合法但当前局面下不合法（引擎状态异常输出）同样拦截
        if (!board.isLegal(m)) {
          throw EngineUnavailableException('引擎返回非法走法: $uci');
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
