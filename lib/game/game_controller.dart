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
    this.givesCheck = false,
  });

  final Move move;
  /// 被吃棋子（FEN 字符），供吃子动画使用
  final String? capturedPiece;
  final String notation;
  final String fenAfter;

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

/// 对局控制器（ChangeNotifier）
class GameController extends ChangeNotifier {
  GameController({
    required this.engine,
    required this._prefs,
    DifficultyLevel? initialLevel,
    this.archiveFile,
  })  : _level = initialLevel ?? DifficultyLevel.medium {
    if (initialLevel == null) {
      _loadSettings();
    } else {
      _saveSettings();
    }
  }

  final EngineClient engine;
  final SharedPreferences _prefs;

  /// 存档文件（测试注入临时文件用；null 时用应用支持目录默认文件）
  final File? archiveFile;

  /// 页面销毁后不再处理异步结果
  bool disposed = false;

  @override
  void dispose() {
    disposed = true;
    super.dispose();
  }

  Board _board = Board();
  List<HistoryEntry> _history = [];
  GameStatus _status = GameStatus.playing;
  DifficultyLevel _level = DifficultyLevel.medium;
  bool _loading = false;

  /// 用户执红（先手）
  bool userPlaysRed = true;

  /// AI 是否正在思考
  bool thinking = false;

  /// 引擎评估（红方视角厘兵值）
  int engineScore = 0;

  /// 局内醒目提示（重复局面 / 长将预警），每次局面变化后在 _updateStatus 重算
  String? ruleNotice;

  /// 终局原因说明（三次重复判和 / 长将判负），无则为 null
  String? endReason;

  /// 引擎故障提示（不可用/超时等）：与规则预警同区域展示，
  /// 引擎调用成功后自动清除，新对局时重置
  String? engineNotice;

  /// 棋谱归档失败提示：对局结束时写入存档失败则展示（读写错误不再静默）
  String? archiveNotice;

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
    if (thinking || _status != GameStatus.playing || !isUserTurn) return;
    hinting = true;
    _hints = const [];
    notifyListeners();
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
      if (disposed ||
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
      engineNotice = '引擎不可用，无法获取提示';
    } catch (e) {
      debugPrint('提示获取失败: $e');
    } finally {
      hinting = false;
      if (!disposed) notifyListeners();
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

  /// 是否正在获取提示
  bool hinting = false;

  /// 开始新对局；[userRed] 用户是否执红先行
  void newGame({bool? userRed}) {
    if (userRed != null) userPlaysRed = userRed;
    _board = Board();
    _history = [];
    _status = GameStatus.playing;
    engineScore = 0;
    thinking = false;
    ruleNotice = null;
    endReason = null;
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
    if (_history.isEmpty) {
      _status = GameStatus.playing;
      return;
    }
    final nextMoves = _board.legalMoves();
    if (nextMoves.isEmpty) {
      _status = _board.redToMove ? GameStatus.blackWin : GameStatus.redWin;
      return;
    }
    // 重复局面判和 / 长将判负（局面 = 棋盘 + 行棋方）。
    // 序列含初始局面，避免"绕回开局局面"的循环漏判。
    final keys = <String>[_posKey(Board.startFen)];
    final checks = <bool>[];
    for (final e in _history) {
      keys.add(_posKey(e.fenAfter));
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
    // 预警：当前局面第 2 次出现（距判和 / 判负生效还差一次重复）
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
    }
  }

  /// 局面键：棋盘 FEN + 行棋方（不含着法计数等无关字段）
  static String _posKey(String fen) => fen.split(' ').take(2).join(' ');

  /// 若轮到 AI 且对局进行中，请求引擎走棋
  Future<void> _maybeEngineMove() async {
    if (_status != GameStatus.playing) return;
    if (_board.redToMove == userPlaysRed) return;
    // 若已有一次引擎思考在跑（如恢复对局 + 新开局连续触发），复用等待
    if (_thinkToken case final token) {
      await token;
      if (disposed || _status != GameStatus.playing) return;
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
    notifyListeners();
    try {
      final result = await engine.think(Board.cloneFrom(_board), _level);
      engineNotice = null;
      engineScore = result.scoreCp;
      if (!disposed &&
          _status == GameStatus.playing &&
          _board.redToMove != userPlaysRed &&
          _board.fen == fenAtRequest &&
          _board.isLegal(result.move)) {
        _applyMove(result.move);
      }
    } on EngineUnavailableException catch (e) {
      // 引擎不可用：明确告知用户，不静默卡住，也绝不伪造走法
      debugPrint('引擎错误: $e');
      engineNotice = '引擎不可用，AI 暂停走棋';
    } catch (e) {
      debugPrint('引擎错误: $e');
    } finally {
      thinking = false;
      if (!disposed) notifyListeners();
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

  /// 结束对局：引擎分析当前局势，按分差判定胜负
  /// 分差 600 以内为平局，某方超过 600 则该方获胜
  /// （600 ≈ 皮卡鱼子力尺度下净多一马/炮；绝杀分 ±9000+ 必然判胜）。
  Future<void> endGameByScore() async {
    if (_status != GameStatus.playing) return;
    if (_history.isEmpty) {
      // 一步未走直接结束：无局势可评，视为平局
      _status = GameStatus.draw;
      unawaited(_archiveGame());
      notifyListeners();
      _saveState();
      return;
    }
    ending = true;
    notifyListeners();
    try {
      final result = await engine.analyze(Board.cloneFrom(_board),
          depth: 12, movetimeMs: 2000);
      engineScore = result.scoreCp;
      if (result.scoreCp > 600) {
        _status = GameStatus.redWin;
      } else if (result.scoreCp < -600) {
        _status = GameStatus.blackWin;
      } else {
        _status = GameStatus.draw;
      }
      unawaited(_archiveGame());
    } catch (e) {
      debugPrint('结束分析失败: $e');
      _status = GameStatus.draw;
      unawaited(_archiveGame());
    } finally {
      ending = false;
      if (!disposed) notifyListeners();
      _saveState();
    }
  }

  /// 是否正在做结束局势分析
  bool ending = false;

  void _undoOne() {
    if (_history.isEmpty) return;
    final entry = _history.removeLast();
    // 从历史 FEN 恢复
    if (_history.isEmpty) {
      _board = Board();
    } else {
      _board = Board.fromFen(_history.last.fenAfter);
    }
    debugPrint('悔棋: ${entry.notation}');
  }

  /// 归档对局（对局结束时）
  Future<void> _archiveGame() async {
    if (_history.isEmpty) return;
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
          history: _history.map((e) => {
                'uci': e.move.uci,
                'captured': e.capturedPiece,
                'notation': e.notation,
                'fen': e.fenAfter,
              }).toList(),
        ));
    if (!ok && !disposed) {
      archiveNotice = '棋谱保存失败';
      notifyListeners();
    }
  }

  /// 立即保存当前对局状态（生命周期兜底：切后台/进程终止前调用）
  Future<void> saveNow() async {
    // 页面已销毁时状态不再有效，跳过
    if (disposed) return;
    await _saveState();
  }

  /// 保存当前局面（自动保存）
  Future<void> _saveState() async {
    try {
      // 对局已结束：棋局不可续玩，清除存档（结果已归档到复盘棋谱）
      if (_status != GameStatus.playing) {
        await _prefs.remove('saved_game');
        return;
      }
      await _prefs.setString('saved_game', jsonEncode({
        'history': _history.map((e) => {
          'uci': e.move.uci,
          'captured': e.capturedPiece,
          'notation': e.notation,
          'fen': e.fenAfter,
        }).toList(),
        'userRed': userPlaysRed,
        'levelName': _level.name,
        'status': _status.index,
      }));
    } catch (_) {}
  }

  /// 恢复上次对局
  /// 失败时棋盘重置为初始局面并返回 false，难度与执子设置保持原值
  Future<bool> restoreGame() async {
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
      _board = Board();
      _history = [];
      _status = GameStatus.playing;
      final hist = data['history'] as List;
      for (final e in hist) {
        final uci = e['uci'] as String;
        if (uci.length < 4) {
          _board = Board();
          _history = [];
          _status = GameStatus.playing;
          return false;
        }
        final m = Move(
          uci.codeUnitAt(0) - 'a'.codeUnitAt(0),
          9 - int.parse(uci[1]),
          uci.codeUnitAt(2) - 'a'.codeUnitAt(0),
          9 - int.parse(uci[3]),
        );
        if (!_board.isLegal(m)) {
          // 数据损坏时放弃恢复
          _board = Board();
          _history = [];
          _status = GameStatus.playing;
          return false;
        }
        _applyMove(m, persist: false);
      }
      // 状态以重放重算为准（_applyMove → _updateStatus 已按终局与重复/长将规则重算），
      // 不再用存档 status 覆盖，避免损坏数据下覆盖出与实际局面不一致的结果
      userPlaysRed = savedUserRed;
      _level = savedLevel;
      notifyListeners();
      _maybeEngineMove();
      return true;
    } catch (_) {
      // 数据损坏：丢弃恢复结果，避免留下走了一半的残缺局面
      _board = Board();
      _history = [];
      _status = GameStatus.playing;
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
      await _prefs.setString('level', _level.name);
      await _prefs.setBool('userRed', userPlaysRed);
      await _saveState();
    } catch (_) {}
  }
}
