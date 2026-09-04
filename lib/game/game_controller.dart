/// 对局控制器：管理棋局状态、悔棋、保存恢复、AI 走棋调度。
library;

import 'dart:async';
import 'dart:convert';

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
  });

  final Move move;
  /// 被吃棋子（FEN 字符），供吃子动画使用
  final String? capturedPiece;
  final String notation;
  final String fenAfter;

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
    required SharedPreferences prefs,
    DifficultyLevel? initialLevel,
  })  : _prefs = prefs,
        _level = initialLevel ?? DifficultyLevel.medium {
    if (initialLevel == null) _loadSettings();
    else _saveSettings();
  }

  final PikafishEngine engine;
  final SharedPreferences _prefs;

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
    try {
      final result = await engine.analyze(
        Board.cloneFrom(_board),
        depth: 12,
        multiPv: 2,
      );
      final moves = <Move>[];
      for (final pv in result.pvList.take(2)) {
        try {
          moves.add(Move.fromUci(pv.move));
        } catch (_) {}
      }
      _hints = moves;
    } catch (e) {
      debugPrint('提示获取失败: $e');
    } finally {
      hinting = false;
      notifyListeners();
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
  void _applyMove(Move m) {
    final captured = _board.pieceAt(m.toFile, m.toRank);
    final notation = moveToChinese(_board, m);
    _board.makeMove(m);
    _history.add(HistoryEntry(
      move: m,
      capturedPiece: captured?.fenChar,
      notation: notation,
      fenAfter: _board.fen,
    ));
    _updateStatus();
    _saveState();
    if (_status != GameStatus.playing) {
      _archiveGame();
    }
  }

  void _updateStatus() {
    if (_history.isEmpty) {
      _status = GameStatus.playing;
      return;
    }
    final nextMoves = _board.legalMoves();
    if (nextMoves.isEmpty) {
      _status = _board.redToMove ? GameStatus.blackWin : GameStatus.redWin;
      return;
    }
    // 简单重复局面判和
    final fens = _history.map((e) => e.fenAfter.split(' ').first).toList();
    final last = fens.last;
    final count = fens.where((f) => f == last).length;
    _status = count >= 3 ? GameStatus.draw : GameStatus.playing;
  }

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
    thinking = true;
    notifyListeners();
    try {
      final result = await engine.think(Board.cloneFrom(_board), _level);
      engineScore = result.scoreCp;
      if (!disposed &&
          _status == GameStatus.playing &&
          _board.redToMove != userPlaysRed &&
          _board.isLegal(result.move)) {
        _applyMove(result.move);
      }
    } catch (e) {
      debugPrint('引擎错误: $e');
    } finally {
      thinking = false;
      if (!disposed) notifyListeners();
    }
  }

  /// 悔棋：撤销用户与 AI 各一步
  void undo() {
    if (_history.isEmpty || thinking) return;
    _undoOne();
    if (_history.isNotEmpty && _board.redToMove != userPlaysRed) {
      _undoOne();
    }
    _status = GameStatus.playing;
    notifyListeners();
    _saveState();
  }

  /// 结束对局：引擎分析当前局势，按分差判定胜负
  /// 分差 1000 以内为平局，某方超过 1000 则该方获胜。
  Future<void> endGameByScore() async {
    if (_status != GameStatus.playing) return;
    if (_history.isEmpty) {
      // 一步未走直接结束：无局势可评，视为平局
      _status = GameStatus.draw;
      _archiveGame();
      notifyListeners();
      _saveState();
      return;
    }
    ending = true;
    notifyListeners();
    try {
      final result = await engine.analyze(Board.cloneFrom(_board), depth: 12);
      engineScore = result.scoreCp;
      if (result.scoreCp > 1000) {
        _status = GameStatus.redWin;
      } else if (result.scoreCp < -1000) {
        _status = GameStatus.blackWin;
      } else {
        _status = GameStatus.draw;
      }
      _archiveGame();
    } catch (e) {
      debugPrint('结束分析失败: $e');
      _status = GameStatus.draw;
      _archiveGame();
    } finally {
      ending = false;
      notifyListeners();
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
  void _archiveGame() {
    if (_history.isEmpty) return;
    GameArchive.add(_prefs, ArchivedGame(
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
  }

  /// 保存当前局面（自动保存）
  Future<void> _saveState() async {
    try {
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
  Future<bool> restoreGame() async {
    try {
      _loading = true;
      notifyListeners();
      final raw = _prefs.getString('saved_game');
      if (raw == null || raw.isEmpty) return false;
      final data = jsonDecode(raw) as Map<String, dynamic>;
      userPlaysRed = data['userRed'] as bool? ?? true;
      _level = DifficultyLevel.all.firstWhere(
        (l) => l.name == data['levelName'],
        orElse: () => DifficultyLevel.medium,
      );
      _board = Board();
      _history = [];
      final hist = data['history'] as List;
      for (final e in hist) {
        final uci = e['uci'] as String;
        if (uci.length < 4) {
          _board = Board();
          _history = [];
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
          return false;
        }
        _applyMove(m);
      }
      final savedStatus = GameStatus.values[data['status'] as int? ?? 0];
      _status = _history.isEmpty ? GameStatus.playing : savedStatus;
      notifyListeners();
      _maybeEngineMove();
      return true;
    } catch (_) {
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
