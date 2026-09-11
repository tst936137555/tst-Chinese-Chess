// 引擎 isolate 侧实现：UCI 协议消息、请求上下文与会话主体。
//
// 本文件是 pikafish.dart 的 part（共享库级导入与私有成员）：
// - 协议消息：主 isolate 与引擎 isolate 之间的握手/请求/错误报文
// - 会话主体：启动引擎传输、完成 UCI 握手、串行处理走棋请求
//   （看门狗/崩溃兜底应答/MultiPV 解析与削弱选路）
part of 'pikafish.dart';

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
Future<void> _engineIsolateEntry(List<Object> args) async {
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
