/// 对局控制器：管理棋局状态、悔棋、保存恢复、AI 走棋调度。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../engine/chinese_notation.dart';
import '../engine/pikafish.dart';
import '../engine/rules.dart';
import 'game_archive.dart';

/// 历史记录条目
class HistoryEntry {
  const HistoryEntry({
    required this.move,
    required this.capturedPiece,
    required this.notation,
    required this.fenAfter,
    required this.posHash,
    this.givesCheck = false,
  });

  final Move move;
  /// 被吃棋子（FEN 字符），供吃子动画使用
  final String? capturedPiece;
  final String notation;
  final String fenAfter;

  /// 走完此步后的局面 Zobrist 哈希（棋盘 + 行棋方），
  /// 供重复局面检测 O(1) 取键，不再逐条解析 FEN
  final int posHash;

  /// 该步是否将军对方（供长将判定；存档回放时由 _applyMove 重算）
  final bool givesCheck;

  /// 被吃棋子的 Piece 对象（按 FEN 字符还原）
  Piece? get capturedPieceObj {
    final c = capturedPiece;
    if (c == null) return null;
    final isRed = c == c.toUpperCase();
    final type = PieceType.values.firstWhere(
      (t) => t.letter == c.toLowerCase(),
    );
    return Piece(isRed, type);
  }
}

/// 对局来源模式：普通开局 / 复盘续下。
/// 决定棋谱归档的标题前缀（（复盘））与偏好写入策略：
/// 仅普通模式才把难度/执子写入全局偏好，避免特殊模式污染默认设置。
enum GameMode {
  normal(''),
  review('复盘');

  const GameMode(this.label);
  /// 模式名（归档标题前缀用），普通模式为空串
  final String label;
}

/// 对局控制器（ChangeNotifier）
class GameController extends ChangeNotifier {
  GameController({
    required this.engine,
    required this._prefs,
    DifficultyLevel? initialLevel,
    this.archiveFile,
    GameMode mode = GameMode.normal,
  })  : _level = initialLevel ?? DifficultyLevel.medium,
        _mode = mode {
    if (initialLevel == null) {
      _loadSettings();
    } else if (mode == GameMode.normal) {
      // 非普通模式的指定难度不写入全局偏好（复盘续下沿用原设置）
      _saveSettings();
    }
    // 初始局面即重算合法走法缓存，保证 legalMoves 读取始终有效
    _updateStatus();
  }

  final EngineClient engine;
  final SharedPreferences _prefs;

  /// 存档文件（测试注入临时文件用；null 时用应用支持目录默认文件）
  final File? archiveFile;

  /// 对局来源模式（归档标题前缀与偏好写入策略，见 [GameMode]）
  GameMode _mode;
  GameMode get mode => _mode;

  /// 页面销毁后不再处理异步结果
  bool _disposed = false;

  /// 是否已销毁（外部只读，测试断言用）
  @visibleForTesting
  bool get disposed => _disposed;

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }

  Board _board = Board();
  List<HistoryEntry> _history = [];
  GameStatus _status = GameStatus.playing;
  DifficultyLevel _level = DifficultyLevel.medium;
  bool _loading = false;

  /// 本局起始局面 FEN（复盘续下为自定义局面，新开局为标准开局）
  String _startFen = Board.startFen;

  /// 本局起始局面的 Zobrist 哈希（重复局面判定基准，随 [_resetToStart] 更新）
  int _startPosHash = Board().positionHash;

  /// 本局起始局面 FEN（供复盘页沿用同一基准局面）
  String get startFen => _startFen;

  /// 用户执红（先手）
  bool userPlaysRed = true;

  /// 字段赋值并在值变化时通知监听者（已销毁不再通知）。
  /// 统一收口可变状态的对外赋值，杜绝"赋值后忘记 notifyListeners"造成 UI 不同步。
  void _setAndNotify<T>(T current, T next, void Function(T) assign) {
    if (current == next) return;
    assign(next);
    if (!_disposed) notifyListeners();
  }

  bool _thinking = false;

  /// AI 是否正在思考
  bool get thinking => _thinking;

  set thinking(bool v) => _setAndNotify(_thinking, v, (x) => _thinking = x);

  /// 局内醒目提示（重复局面 / 长将预警），每次局面变化后在 _updateStatus 重算
  String? _ruleNotice;

  String? get ruleNotice => _ruleNotice;

  set ruleNotice(String? v) =>
      _setAndNotify(_ruleNotice, v, (x) => _ruleNotice = x);

  /// 终局原因说明（三次重复判和 / 长将判负），无则为 null
  String? _endReason;

  String? get endReason => _endReason;

  set endReason(String? v) =>
      _setAndNotify(_endReason, v, (x) => _endReason = x);

  /// 引擎故障提示（不可用/超时等）：与规则预警同区域展示，
  /// 引擎调用成功后自动清除，新对局时重置
  String? _engineNotice;

  String? get engineNotice => _engineNotice;

  set engineNotice(String? v) =>
      _setAndNotify(_engineNotice, v, (x) => _engineNotice = x);

  String? _archiveNotice;

  /// 棋谱归档失败提示：对局结束时写入存档失败则展示（读写错误不再静默）
  String? get archiveNotice => _archiveNotice;

  set archiveNotice(String? v) =>
      _setAndNotify(_archiveNotice, v, (x) => _archiveNotice = x);

  /// 取走归档失败提示（取走即清除且不触发通知）：
  /// UI 收到该提示的通知后转 SnackBar 展示，清除本身无需再次通知重建。
  String? takeArchiveNotice() {
    final n = _archiveNotice;
    _archiveNotice = null;
    return n;
  }

  String? _saveNotice;

  /// 对局自动保存失败提示（切后台/退出后进度可能丢失，下次保存成功自动清除）
  String? get saveNotice => _saveNotice;

  void _setSaveNotice(String? v) =>
      _setAndNotify(_saveNotice, v, (x) => _saveNotice = x);

  /// 当前行棋方的全部合法走法（随局面在 _updateStatus 重算）。
  /// 供 UI 点选棋子时按起点过滤复用，避免每次点击全盘重算。
  List<Move> _legalMovesCache = const [];

  List<Move> get legalMoves => _legalMovesCache;

  /// 重置为本局起始局面（新局/恢复失败兜底），并重算合法走法缓存
  void _resetToStart() {
    _board = Board.fromFen(_startFen);
    // 重复局面判定基准随起始局面更新（复盘续下非标准开局）
    _startPosHash = _board.positionHash;
    _history = [];
    _status = GameStatus.playing;
    _updateStatus();
  }

  /// 正在恢复存档
  bool get loading => _loading;

  Board get board => _board;
  List<HistoryEntry> get history => _history;
  GameStatus get status => _status;
  DifficultyLevel get level => _level;
  bool get isUserTurn =>
      _status == GameStatus.playing && _board.redToMove == userPlaysRed;

  /// 最近一步（供 UI 高亮）
  Move? get lastMove => _history.isEmpty ? null : _history.last.move;

  /// 被将军的将/帅位置
  (int, int)? get checkPos {
    if (!_board.inCheck) return null;
    final k = _board.findKing(_board.redToMove);
    return k;
  }

  /// 是否应翻转棋盘（用户执黑时，黑在下）
  bool get flipBoard => !userPlaysRed;

  void setLevel(DifficultyLevel l) {
    _level = l;
    _saveSettings();
    notifyListeners();
  }

  /// 降低一档难度（已是最低档则忽略）
  void lowerLevel() {
    final idx = DifficultyLevel.all.indexOf(_level);
    if (idx > 0) setLevel(DifficultyLevel.all[idx - 1]);
  }

  /// 提升一档难度（已是最高档则忽略）
  void raiseLevel() {
    final idx = DifficultyLevel.all.indexOf(_level);
    if (idx < DifficultyLevel.all.length - 1) {
      setLevel(DifficultyLevel.all[idx + 1]);
    }
  }

  /// 提示：由最高难度引擎给出两个建议走法
  Future<void> hint() async {
    if (thinking || hinting || _status != GameStatus.playing || !isUserTurn) {
      return;
    }
    hinting = true;
    _hints = const [];
    // 记录请求时的局面：期间走子/悔棋导致局面变化（含悔棋后重走、长度不变）即作废
    final fenAtRequest = _history.isEmpty ? '' : _history.last.fenAfter;
    try {
      // 深度/时间双限：与大师档/复盘同一评判标准（深度 12），慢设备由 3s 时间上限兜底
      final result = await engine.analyze(
        Board.cloneFrom(_board),
        depth: 12,
        multiPv: 2,
        movetimeMs: 3000,
      );
      // 页面已销毁或局面已变化：过期建议直接丢弃
      if (_disposed ||
          (_history.isEmpty ? '' : _history.last.fenAfter) != fenAtRequest) {
        return;
      }
      final moves = <Move>[];
      for (final pv in result.pvList.take(2)) {
        try {
          moves.add(Move.fromUci(pv.move));
        } catch (_) {}
      }
      _hints = moves;
      engineNotice = null;
    } on EngineUnavailableException catch (e) {
      debugPrint('提示获取失败: $e');
      engineNotice = '无法获取提示：${e.message}';
    } catch (e) {
      debugPrint('提示获取失败: $e');
      // 非引擎异常同样透传，与 endGameByScore 的如实呈现策略一致
      engineNotice = '无法获取提示：$e';
    } finally {
      hinting = false;
    }
  }

  /// 清除提示（走子后调用）
  void clearHints() {
    if (_hints.isNotEmpty) {
      _hints = const [];
      notifyListeners();
    }
  }

  List<Move> _hints = const [];

  /// 当前提示的走法（最多两个）
  List<Move> get hints => _hints;

  bool _hinting = false;

  /// 是否正在获取提示
  bool get hinting => _hinting;

  set hinting(bool v) => _setAndNotify(_hinting, v, (x) => _hinting = x);

  /// 开始新对局；[userRed] 用户是否执红先行，
  /// [startFen] 自定义起始局面（复盘续下），[mode] 对局来源模式
  void newGame({bool? userRed, String? startFen, GameMode? mode}) {
    if (userRed != null) userPlaysRed = userRed;
    if (startFen != null) _startFen = startFen;
    if (mode != null) _mode = mode;
    _resetToStart();
    thinking = false;
    engineNotice = null;
    _hints = const [];
    notifyListeners();
    _saveSettings();
    _maybeEngineMove();
  }

  /// 用户尝试走棋（点击 from -> to）
  bool tryMove(Move m) {
    if (!isUserTurn || !_board.isLegal(m)) return false;
    clearHints();
    _applyMove(m);
    notifyListeners();
    _maybeEngineMove();
    return true;
  }

  /// 应用走法并更新历史与状态
  /// [persist] 为 false 时（恢复对局重放）不落盘、不归档
  void _applyMove(Move m, {bool persist = true}) {
    final captured = _board.pieceAt(m.toFile, m.toRank);
    final notation = moveToChinese(_board, m);
    _board.makeMove(m);
    // 走完后对方（当前行棋方）被将军 = 该步将军，供长将判定
    final givesCheck = _board.inCheck;
    _history.add(HistoryEntry(
      move: m,
      capturedPiece: captured?.fenChar,
      notation: notation,
      fenAfter: _board.fen,
      posHash: _board.positionHash,
      givesCheck: givesCheck,
    ));
    _updateStatus();
    if (!persist) return;
    _saveState();
    if (_status != GameStatus.playing) {
      unawaited(_archiveGame());
    }
  }

  void _updateStatus() {
    ruleNotice = null;
    endReason = null;
    // 合法走法在此统一重算并缓存（走子/悔棋/新局均会经过此处），
    // UI 点选棋子时复用，不再每次点击全盘重算
    final nextMoves = _board.legalMoves();
    _legalMovesCache = nextMoves;
    if (_history.isEmpty) {
      _status = GameStatus.playing;
      return;
    }
    if (nextMoves.isEmpty) {
      _status = _board.redToMove ? GameStatus.blackWin : GameStatus.redWin;
      return;
    }
    // 重复局面判和 / 长将判负：键为 Zobrist 哈希（棋盘 + 行棋方），
    // 逐条存于历史条目，随走法增量产生，不再全量解析 FEN。
    // 序列含本局起始局面（随 _resetToStart 更新，复盘续下非标准开局），
    // 避免"绕回开局局面"的循环漏判。
    final keys = <int>[_startPosHash];
    final checks = <bool>[];
    for (final e in _history) {
      keys.add(e.posHash);
      checks.add(e.givesCheck);
    }
    final rep = repetitionStatus(keys, checks);
    _status = rep ?? GameStatus.playing;
    if (rep != null) {
      // 判定生效：终局原因说明（结算弹窗展示）
      endReason = switch (rep) {
        GameStatus.draw => '三次重复局面，双方不变作和',
        GameStatus.blackWin => '红方长将，判负',
        _ => '黑方长将，判负',
      };
      return;
    }
    // 自然限着：双方连续 60 回合（120 半回合）无吃子，判和
    if (_board.isNaturalDraw) {
      _status = GameStatus.draw;
      endReason = '双方 60 回合未吃子，自然限着作和';
      return;
    }
    // 预警一：当前局面第 2 次出现（距判和 / 判负生效还差一次重复）
    final last = keys.last;
    var prev = -1; // 上一次出现的位置
    for (var i = 0; i < keys.length - 1; i++) {
      if (keys[i] == last) prev = i;
    }
    if (prev >= 0) {
      final side = longCheckSide(checks, prev, keys.length - 1);
      ruleNotice = switch (side) {
        'r' => '红方长将！再重复一次将判红方负',
        'b' => '黑方长将！再重复一次将判黑方负',
        _ => '局面已重复 2 次，再重复一次将判和',
      };
      return;
    }
    // 预警二：接近自然限着（≥50 回合未吃子）
    final rounds = _board.halfmoveClock ~/ 2;
    if (rounds >= 50) {
      ruleNotice = '双方已 $rounds 回合未吃子，累计 60 回合将判和';
    }
  }

  /// 若轮到 AI 且对局进行中，请求引擎走棋
  Future<void> _maybeEngineMove() async {
    if (_status != GameStatus.playing) return;
    if (_board.redToMove == userPlaysRed) return;
    // 若已有一次引擎思考在跑（如恢复对局 + 新开局连续触发），复用等待
    if (_thinkToken case final token) {
      await token;
      if (_disposed || _status != GameStatus.playing) return;
      if (_board.redToMove == userPlaysRed) return;
    }
    final future = _doThink();
    _thinkToken = future;
    await future;
    _thinkToken = null;
  }

  Future<void>? _thinkToken;

  Future<void> _doThink() async {
    // 记录请求时局面：期间新开局/恢复存档等导致局面变化即作废结果
    final fenAtRequest = _board.fen;
    thinking = true;
    try {
      final result = await engine.think(Board.cloneFrom(_board), _level);
      engineNotice = null;
      if (!_disposed &&
          _status == GameStatus.playing &&
          _board.redToMove != userPlaysRed &&
          _board.fen == fenAtRequest &&
          _board.isLegal(result.move)) {
        _applyMove(result.move);
      }
    } on EngineUnavailableException catch (e) {
      // 引擎不可用：透传具体原因（含 fail-fast 时的"请重启应用"引导），
      // 不静默卡住，也绝不伪造走法
      debugPrint('引擎错误: $e');
      engineNotice = 'AI 暂停走棋：${e.message}';
    } catch (e) {
      debugPrint('引擎错误: $e');
    } finally {
      thinking = false;
    }
  }

  /// 悔棋：撤销用户与 AI 各一步
  void undo() {
    if (_history.isEmpty || thinking || ending) return;
    _undoOne();
    if (_history.isNotEmpty && _board.redToMove != userPlaysRed) {
      _undoOne();
    }
    // 重算状态与规则提示（预警随局面回退自动清除）
    _updateStatus();
    notifyListeners();
    _saveState();
    // 撤销后若轮到 AI（如执黑时悔掉 AI 的开局首步），需重新触发引擎走棋
    _maybeEngineMove();
  }

  /// 结束判定的分差标准（厘兵）：某方超过该值判胜，以内为平局
  static const endScoreCp = 500;

  /// 结束前的局势预览：与结算同一标准分析当前局面（红方视角，厘兵）。
  /// 深度 12 + 3s 与大师档/复盘分析同一评判标准，慢设备由时间上限兜底。
  /// 一步未走（无局势可评）返回 0；失败时抛出原始异常由调用方展示。
  Future<int> analyzeEndingScore() async {
    if (_history.isEmpty) return 0;
    final result = await engine.analyze(Board.cloneFrom(_board),
        depth: 12, movetimeMs: 3000);
    return result.scoreCp;
  }

  /// 结束对局：按局势评分判定胜负
  /// 分差 [endScoreCp] 以内为平局，某方超过则该方获胜
  /// （绝杀分 ±9000+ 必然判胜）。
  /// [scoreCp] 为结束弹窗中已算好的局势评分，传入则跳过重复分析；
  /// 省略时现场分析（弹窗预览分析失败后的兜底路径）。
  Future<void> endGameByScore({int? scoreCp}) async {
    if (_status != GameStatus.playing || ending) return;
    if (_history.isEmpty) {
      // 一步未走直接结束：无局势可评，视为平局
      _status = GameStatus.draw;
      unawaited(_archiveGame());
      notifyListeners();
      await _saveState();
      return;
    }
    ending = true;
    try {
      final score = scoreCp ??
          (await engine.analyze(Board.cloneFrom(_board),
                  depth: 12, movetimeMs: 3000))
              .scoreCp;
      if (score > endScoreCp) {
        _status = GameStatus.redWin;
      } else if (score < -endScoreCp) {
        _status = GameStatus.blackWin;
      } else {
        _status = GameStatus.draw;
      }
      unawaited(_archiveGame());
    } catch (e) {
      debugPrint('结束分析失败: $e');
      // 如实告知：此平局是故障兜底，而非局势判定结果，并附具体原因
      // （引擎故障透传 message，含 fail-fast 时的"请重启应用"引导；
      // endReason 进结算弹窗，engineNotice 与其他引擎故障提示同横幅展示）
      final reason = e is EngineUnavailableException
          ? '引擎分析失败，按平局结算：${e.message}'
          : '分析异常，按平局结算：$e';
      endReason = reason;
      engineNotice = reason;
      _status = GameStatus.draw;
      unawaited(_archiveGame());
    } finally {
      ending = false;
      await _saveState();
    }
  }

  bool _ending = false;

  /// 是否正在做结束局势分析
  bool get ending => _ending;

  set ending(bool v) => _setAndNotify(_ending, v, (x) => _ending = x);

  void _undoOne() {
    if (_history.isEmpty) return;
    final entry = _history.removeLast();
    // 从历史 FEN 恢复（历史清空则回到本局起始局面，复盘续下非标准开局）
    if (_history.isEmpty) {
      _board = Board.fromFen(_startFen);
    } else {
      _board = Board.fromFen(_history.last.fenAfter);
    }
    debugPrint('悔棋: ${entry.notation}');
  }

  /// 在飞归档任务（终局 fire-and-forget 触发）。存档写入已全局串行，
  /// 后发起的归档必然晚于先者完成，追踪最近一次即可代表全部在飞归档。
  Future<void>? _archivePending;

  /// 等待在飞归档落盘完成（测试收尾 / 生命周期兜底用）
  Future<void> flushArchives() => _archivePending ?? Future<void>.value();

  /// 归档对局（对局结束时）
  Future<void> _archiveGame() {
    final op = _archiveGameNow();
    _archivePending = op;
    return op;
  }

  Future<void> _archiveGameNow() async {
    if (_history.isEmpty) return;
    try {
      final ok = await GameArchive.add(
          archiveFile ?? await GameArchive.defaultArchiveFile(),
          ArchivedGame(
            time: DateTime.now(),
            userRed: userPlaysRed,
            levelName: _level.name,
            result: switch (_status) {
              GameStatus.redWin => 'redWin',
              GameStatus.blackWin => 'blackWin',
              _ => 'draw',
            },
            // 仅存 uci/captured/notation：fen 可由 uci 序列重放推导，
            // 复盘端（_historyFromArchive）以重放为准逐步重算；
            // 自定义起始局面（复盘续下）必须存 startFen 作为重放基准
            startFen: _startFen == Board.startFen ? null : _startFen,
            mode: _mode.name,
            history: _history.map((e) => {
                  'uci': e.move.uci,
                  'captured': e.capturedPiece,
                  'notation': e.notation,
                }).toList(),
          ));
      if (!ok) {
        archiveNotice = '棋谱保存失败';
      }
    } catch (e) {
      // 归档是后台任务：异常只记录，绝不冒泡为未处理异步错误
      debugPrint('棋谱归档异常: $e');
    }
  }

  /// 立即保存当前对局状态（生命周期兜底：切后台/进程终止前调用），
  /// 并等待在飞归档落盘，避免归档被进程终止打断
  Future<void> saveNow() async {
    // 页面已销毁时状态不再有效，跳过
    if (_disposed) return;
    await _saveState();
    await flushArchives();
  }

  /// 保存当前局面（自动保存）
  Future<void> _saveState() async {
    try {
      // 对局已结束：棋局不可续玩，清除存档（结果已归档到复盘棋谱）
      if (_status != GameStatus.playing) {
        await _prefs.remove('saved_game');
        _setSaveNotice(null);
        return;
      }
      await _prefs.setString('saved_game', jsonEncode({
        // 仅存 uci 序列：captured/notation/fen/status 均可在恢复重放时
        // 经 _applyMove 全量重算（旧版冗余字段由重放端兼容读取）；
        // 自定义起始局面与模式随档保存（旧档缺失字段恢复为标准开局/普通模式）
        'history': [for (final e in _history) {'uci': e.move.uci}],
        'userRed': userPlaysRed,
        'levelName': _level.name,
        'startFen': _startFen,
        'mode': _mode.name,
      }));
      _setSaveNotice(null);
    } catch (e) {
      // 自动保存失败如实提示（旧版静默吞错，用户退出后进度无声丢失）
      debugPrint('对局保存失败: $e');
      _setSaveNotice('对局保存失败，进度可能丢失：$e');
    }
  }

  /// 恢复上次对局
  /// 失败时棋盘重置为原起始局面并返回 false，难度/执子/起始局面/模式保持原值
  Future<bool> restoreGame() async {
    // 失败回滚用：起始局面与模式在重放前就得位（_resetToStart 依赖），
    // 损坏时恢复原值，避免半截恢复污染控制器状态
    final prevStartFen = _startFen;
    final prevMode = _mode;
    try {
      _loading = true;
      notifyListeners();
      final raw = _prefs.getString('saved_game');
      if (raw == null || raw.isEmpty) return false;
      final data = jsonDecode(raw) as Map<String, dynamic>;
      // 先暂存，整局重放成功后再提交，避免损坏数据污染设置
      final savedUserRed = data['userRed'] as bool? ?? true;
      final savedLevel = DifficultyLevel.all.firstWhere(
        (l) => l.name == data['levelName'],
        orElse: () => DifficultyLevel.medium,
      );
      // 起始局面与模式随档恢复（旧档缺失时为标准开局/普通模式）
      _startFen = data['startFen'] as String? ?? Board.startFen;
      _mode = GameMode.values.firstWhere(
        (m) => m.name == data['mode'],
        orElse: () => GameMode.normal,
      );
      _resetToStart();
      final hist = data['history'] as List;
      for (final e in hist) {
        // Move.fromUci 严格校验格式，非法字符/越界直接抛 FormatException，
        // 由外层 catch 统一丢弃恢复（与旧手工解析的长度检查等效且更严）
        final m = Move.fromUci(e['uci'] as String);
        if (!_board.isLegal(m)) {
          // 数据损坏时放弃恢复
          _startFen = prevStartFen;
          _mode = prevMode;
          _resetToStart();
          return false;
        }
        _applyMove(m, persist: false);
      }
      // 状态以重放重算为准（_applyMove → _updateStatus 已按终局与重复/长将规则重算），
      // 不再用存档 status 覆盖，避免损坏数据下覆盖出与实际局面不一致的结果
      userPlaysRed = savedUserRed;
      _level = savedLevel;
      notifyListeners();
      // 故意不等 AI 思考完成：恢复流程以"局面已就位"为准即返回
      unawaited(_maybeEngineMove());
      return true;
    } catch (_) {
      // 数据损坏：丢弃恢复结果，避免留下走了一半的残缺局面
      _startFen = prevStartFen;
      _mode = prevMode;
      _resetToStart();
      return false;
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  Future<void> _loadSettings() async {
    try {
      final levelName = _prefs.getString('level');
      if (levelName != null) {
        for (final l in DifficultyLevel.all) {
          if (l.name == levelName) _level = l;
        }
      }
      userPlaysRed = _prefs.getBool('userRed') ?? true;
    } catch (_) {}
  }

  Future<void> _saveSettings() async {
    try {
      // 仅普通对弈写全局难度/执子偏好：复盘续下沿用原设置，
      // 不应覆盖用户普通对弈的默认选择
      if (_mode == GameMode.normal) {
        await _prefs.setString('level', _level.name);
        await _prefs.setBool('userRed', userPlaysRed);
      }
      await _saveState();
    } catch (_) {}
  }
}
