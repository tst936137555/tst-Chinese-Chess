/// 中国象棋棋规核心：棋盘表示、FEN 解析、走法生成、胜负判定。
///
/// 坐标系统：file 0-8 从左到右（红方视角），rank 0-9 从上到下（rank 0 为黑方底线）。
/// 走法格式采用 UCI 风格，如 h2e2。
library;

/// 棋子类型
enum PieceType {
  king('k'),
  advisor('a'),
  elephant('b'),
  horse('n'),
  rook('r'),
  cannon('c'),
  pawn('p');

  const PieceType(this.letter);
  final String letter;
}

/// 棋子（颜色 + 类型）
class Piece {
  const Piece(this.isRed, this.type);
  final bool isRed;
  final PieceType type;

  /// FEN 字符：红方大写、黑方小写
  String get fenChar => isRed ? type.letter.toUpperCase() : type.letter;

  @override
  bool operator ==(Object other) => other is Piece && other.isRed == isRed && other.type == type;

  @override
  int get hashCode => Object.hash(isRed, type);

  @override
  String toString() => fenChar;
}

/// 一步走棋
class Move {
  const Move(this.fromFile, this.fromRank, this.toFile, this.toRank);

  final int fromFile;
  final int fromRank;
  final int toFile;
  final int toRank;

  /// UCI 格式字符串，如 h2e2
  String get uci {
    String f(int i) => String.fromCharCode('a'.codeUnitAt(0) + i);
    return '${f(fromFile)}${9 - fromRank}${f(toFile)}${9 - toRank}';
  }

  /// 从 UCI 字符串（如 "a0a1"）解析
  factory Move.fromUci(String uci) {
    return Move(
      uci.codeUnitAt(0) - 'a'.codeUnitAt(0),
      9 - int.parse(uci[1]),
      uci.codeUnitAt(2) - 'a'.codeUnitAt(0),
      9 - int.parse(uci[3]),
    );
  }

  @override
  bool operator ==(Object other) =>
      other is Move &&
      other.fromFile == fromFile &&
      other.fromRank == fromRank &&
      other.toFile == toFile &&
      other.toRank == toRank;

  @override
  int get hashCode => Object.hash(fromFile, fromRank, toFile, toRank);

  @override
  String toString() => uci;
}

/// 对局状态结果
enum GameStatus { playing, redWin, blackWin, draw }

/// 棋盘与规则实现
class Board {
  /// 90 格棋盘，下标 = rank * 9 + file
  final List<Piece?> _squares = List.filled(90, null);

  /// 轮到红方走则为 true
  bool redToMove = true;

  /// 局面 FEN
  String get fen {
    final sb = StringBuffer();
    for (int rank = 0; rank < 10; rank++) {
      int empty = 0;
      for (int file = 0; file < 9; file++) {
        final p = _squares[rank * 9 + file];
        if (p == null) {
          empty++;
        } else {
          if (empty > 0) {
            sb.write(empty);
            empty = 0;
          }
          sb.write(p.fenChar);
        }
      }
      if (empty > 0) sb.write(empty);
      if (rank < 9) sb.write('/');
    }
    sb.write(' ${redToMove ? 'w' : 'b'} - - 0 1');
    return sb.toString();
  }

  Piece? pieceAt(int file, int rank) => _squares[rank * 9 + file];

  void _set(int file, int rank, Piece? p) => _squares[rank * 9 + file] = p;

  /// 标准初始局面
  Board() {
    _loadFen(startFen);
  }

  Board.cloneFrom(Board other) {
    for (int i = 0; i < 90; i++) {
      _squares[i] = other._squares[i];
    }
    redToMove = other.redToMove;
  }

  /// 标准初始局面 FEN（公开供重复判定等使用）
  static const startFen =
      'rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1';

  /// 从 FEN 载入局面
  factory Board.fromFen(String fen) {
    final b = Board();
    b._loadFen(fen);
    return b;
  }

  void _loadFen(String fen) {
    _squares.fillRange(0, 90, null);
    final parts = fen.trim().split(RegExp(r'\s+'));
    final ranks = parts[0].split('/');
    if (ranks.length != 10) throw ArgumentError('FEN 需 10 行: $fen');
    for (int rank = 0; rank < 10; rank++) {
      int file = 0;
      for (final ch in ranks[rank].split('')) {
        if (RegExp(r'\d').hasMatch(ch)) {
          file += int.parse(ch);
        } else {
          final isRed = ch == ch.toUpperCase();
          final type = PieceType.values.firstWhere(
            (t) => t.letter == ch.toLowerCase(),
            orElse: () => throw ArgumentError('非法棋子字符: $ch'),
          );
          if (file > 8) throw ArgumentError('FEN 行超长: ${ranks[rank]}');
          _set(file, rank, Piece(isRed, type));
          file++;
        }
      }
      if (file != 9) throw ArgumentError('FEN 行长度错误: ${ranks[rank]}');
    }
    redToMove = parts.length < 2 || parts[1] == 'w';
  }

  /// 执行走法（须为合法走法）
  void makeMove(Move m) {
    final p = pieceAt(m.fromFile, m.fromRank);
    _set(m.toFile, m.toRank, p);
    _set(m.fromFile, m.fromRank, null);
    redToMove = !redToMove;
  }

  /// 撤销走法并还原被吃棋子
  void undoMove(Move m, Piece? captured) {
    final p = pieceAt(m.toFile, m.toRank)!;
    _set(m.fromFile, m.fromRank, p);
    _set(m.toFile, m.toRank, captured);
    redToMove = !redToMove;
  }

  /// 查找指定某方将/帅的位置，返回 (file, rank)
  (int, int)? findKing(bool red) {
    final lo = red ? 7 : 0;
    final hi = red ? 9 : 2;
    for (int rank = lo; rank <= hi; rank++) {
      for (int file = 3; file <= 5; file++) {
        final p = pieceAt(file, rank);
        if (p != null && p.type == PieceType.king && p.isRed == red) {
          return (file, rank);
        }
      }
    }
    return null;
  }

  /// 是否被对方将军
  bool _inCheck(Board board, bool red) {
    final kingPos = board.findKing(red);
    if (kingPos == null) return false;
    final (kf, kr) = kingPos;

    // 车/将（同列直视）与炮：沿四个方向扫描
    for (final (df, dr) in const [(1, 0), (-1, 0), (0, 1), (0, -1)]) {
      int f = kf + df, r = kr + dr;
      int blockers = 0; // 首个棋子后的遮挡数
      Piece? first;
      while (f >= 0 && f < 9 && r >= 0 && r < 10) {
        final p = board.pieceAt(f, r);
        if (p != null) {
          if (first == null) {
            first = p;
            if (p.isRed != red &&
                (p.type == PieceType.rook ||
                    (p.type == PieceType.king && df == 0))) {
              return true; // 车或将帅照面（同列直视）
            }
          } else {
            blockers++;
            if (blockers == 1 &&
                p.isRed != red &&
                p.type == PieceType.cannon) {
              return true; // 炮隔一子打将
            }
            break;
          }
        }
        f += df;
        r += dr;
      }
    }

    // 马：8 个马位（hf/hr 为马腿相对将的位置，紧邻马、朝将方向长轴）
    for (final (df, dr, hf, hr) in const [
      (2, 1, 1, 1), (2, -1, 1, -1), (-2, 1, -1, 1), (-2, -1, -1, -1),
      (1, 2, 1, 1), (-1, 2, -1, 1), (1, -2, 1, -1), (-1, -2, -1, -1),
    ]) {
      final f = kf + df, r = kr + dr;
      if (f < 0 || f > 8 || r < 0 || r > 9) continue;
      final p = board.pieceAt(f, r);
      if (p != null && p.isRed != red && p.type == PieceType.horse) {
        final leg = board.pieceAt(kf + hf, kr + hr);
        if (leg == null) return true;
      }
    }

    // 兵：红兵向上（rank-1），黑卒向下（rank+1），过河后可横走
    if (red) {
      // 黑卒向下走，正面攻击红帅的卒位于 kr-1
      final p1 = kr - 1 >= 0 ? board.pieceAt(kf, kr - 1) : null;
      if (p1 != null && !p1.isRed && p1.type == PieceType.pawn) return true;
      if (kr >= 5) {
        // 卒位于 kr 行，过河（rank>=5）后可横吃
        for (final df in const [-1, 1]) {
          final f = kf + df;
          if (f < 0 || f > 8) continue;
          final p = board.pieceAt(f, kr);
          if (p != null && !p.isRed && p.type == PieceType.pawn) return true;
        }
      }
    } else {
      // 红兵向上走，正面攻击黑将的兵位于 kr+1
      final p1 = kr + 1 <= 9 ? board.pieceAt(kf, kr + 1) : null;
      if (p1 != null && p1.isRed && p1.type == PieceType.pawn) return true;
      if (kr <= 4) {
        // 兵位于 kr 行，过河（rank<=4）后可横吃
        for (final df in const [-1, 1]) {
          final f = kf + df;
          if (f < 0 || f > 8) continue;
          final p = board.pieceAt(f, kr);
          if (p != null && p.isRed && p.type == PieceType.pawn) return true;
        }
      }
    }
    return false;
  }

  /// 当前走棋方是否被将军
  bool get inCheck => _inCheck(this, redToMove);

  /// 在九宫内
  static bool _inPalace(int file, int rank, bool red) {
    if (file < 3 || file > 5) return false;
    return red ? rank >= 7 : rank <= 2;
  }

  /// 己方半场
  static bool _ownHalf(int rank, bool red) => red ? rank >= 5 : rank <= 4;

  bool _canAttack(int toFile, int toRank, bool red) {
    final target = pieceAt(toFile, toRank);
    return target == null || target.isRed != red;
  }

  /// 生成某棋子的伪合法走法（不考虑送将）
  List<Move> _pieceMoves(int file, int rank) {
    final p = pieceAt(file, rank)!;
    final red = p.isRed;
    final moves = <Move>[];

    void add(int f, int r) {
      if (f >= 0 && f < 9 && r >= 0 && r < 10 && _canAttack(f, r, red)) {
        moves.add(Move(file, rank, f, r));
      }
    }

    switch (p.type) {
      case PieceType.king:
        for (final (df, dr) in const [(1, 0), (-1, 0), (0, 1), (0, -1)]) {
          final f = file + df, r = rank + dr;
          if (f < 0 || f > 8 || r < 0 || r > 9) continue;
          if (_inPalace(f, r, red)) add(f, r);
        }
      case PieceType.advisor:
        for (final (df, dr) in const [(1, 1), (1, -1), (-1, 1), (-1, -1)]) {
          final f = file + df, r = rank + dr;
          if (f < 0 || f > 8 || r < 0 || r > 9) continue;
          if (_inPalace(f, r, red)) add(f, r);
        }
      case PieceType.elephant:
        for (final (df, dr) in const [(2, 2), (2, -2), (-2, 2), (-2, -2)]) {
          final f = file + df, r = rank + dr;
          if (f < 0 || f > 8 || r < 0 || r > 9) continue;
          if (_ownHalf(r, red) && pieceAt(file + df ~/ 2, rank + dr ~/ 2) == null) {
            add(f, r);
          }
        }
      case PieceType.horse:
        for (final (df, dr, hf, hr) in const [
          (2, 1, 1, 0), (2, -1, 1, 0), (-2, 1, -1, 0), (-2, -1, -1, 0),
          (1, 2, 0, 1), (-1, 2, 0, 1), (1, -2, 0, -1), (-1, -2, 0, -1),
        ]) {
          final f = file + df, r = rank + dr;
          if (f < 0 || f > 8 || r < 0 || r > 9) continue;
          if (pieceAt(file + hf, rank + hr) == null) add(f, r);
        }
      case PieceType.rook:
        for (final (df, dr) in const [(1, 0), (-1, 0), (0, 1), (0, -1)]) {
          int f = file + df, r = rank + dr;
          while (f >= 0 && f < 9 && r >= 0 && r < 10) {
            final t = pieceAt(f, r);
            if (t == null) {
              moves.add(Move(file, rank, f, r));
            } else {
              if (t.isRed != red) moves.add(Move(file, rank, f, r));
              break;
            }
            f += df;
            r += dr;
          }
        }
      case PieceType.cannon:
        for (final (df, dr) in const [(1, 0), (-1, 0), (0, 1), (0, -1)]) {
          int f = file + df, r = rank + dr;
          bool jumped = false;
          while (f >= 0 && f < 9 && r >= 0 && r < 10) {
            final t = pieceAt(f, r);
            if (!jumped) {
              if (t == null) {
                moves.add(Move(file, rank, f, r));
              } else {
                jumped = true;
              }
            } else {
              if (t != null) {
                if (t.isRed != red) moves.add(Move(file, rank, f, r));
                break;
              }
            }
            f += df;
            r += dr;
          }
        }
      case PieceType.pawn:
        final forward = red ? -1 : 1;
        add(file, rank + forward);
        final crossedRiver = red ? rank <= 4 : rank >= 5;
        if (crossedRiver) {
          add(file - 1, rank);
          add(file + 1, rank);
        }
    }
    return moves;
  }

  /// 生成所有伪合法走法
  List<Move> pseudoMoves({bool? forRed}) {
    final red = forRed ?? redToMove;
    final result = <Move>[];
    for (int rank = 0; rank < 10; rank++) {
      for (int file = 0; file < 9; file++) {
        final p = pieceAt(file, rank);
        if (p != null && p.isRed == red) {
          result.addAll(_pieceMoves(file, rank));
        }
      }
    }
    return result;
  }

  /// 走法是否合法（不送将）
  bool isLegal(Move m) {
    final p = pieceAt(m.fromFile, m.fromRank);
    if (p == null || p.isRed != redToMove) return false;
    if (!_pieceMoves(m.fromFile, m.fromRank).contains(m)) return false;
    final captured = pieceAt(m.toFile, m.toRank);
    makeMove(m);
    final bad = _inCheck(this, !redToMove); // 检查走棋方（已翻转）是否被将
    undoMove(m, captured);
    return !bad;
  }

  /// 所有合法走法
  List<Move> legalMoves() =>
      pseudoMoves().where((m) => isLegal(m)).toList();

  /// 将死 / 困毙判定（在当前方走之前调用）
  GameStatus statusAfterMove() {
    if (legalMoves().isEmpty) {
      // 无子可动：被将军则将死，否则困毙，均为对方胜
      return redToMove ? GameStatus.blackWin : GameStatus.redWin;
    }
    return GameStatus.playing;
  }
}

/// 重复局面判和与长将判负。
///
/// [positionKeys] 为局面键序列：下标 0 = 初始局面，k = 走完第 k 步后；
/// 键须为「棋盘 FEN + 行棋方」两段（不含着法计数等无关字段）。
/// [givesChecks][k] 表示第 k 步（0 基）走完后对方是否被将军。
///
/// 同一局面出现 3 次即触发：取最后一个完整重复周期分析——
/// 周期内若一方所有着法均为将军而对方并非如此，则该方长将判负；
/// 双方均长将（或均非长将）判和。未重复 3 次返回 null（对局继续）。
GameStatus? repetitionStatus(
    List<String> positionKeys, List<bool> givesChecks) {
  if (positionKeys.length < 5) return null; // 3 次同局面至少需 4 步
  final last = positionKeys.last;
  final occurrences = <int>[];
  for (var i = 0; i < positionKeys.length; i++) {
    if (positionKeys[i] == last) occurrences.add(i);
  }
  if (occurrences.length < 3) return null;

  // 最后一个完整周期：局面 o2 -> o3，即 0 基第 o2 .. o3-1 步
  final o2 = occurrences[occurrences.length - 2];
  final o3 = occurrences.last;
  return switch (longCheckSide(givesChecks, o2, o3)) {
    'r' => GameStatus.blackWin, // 红方长将
    'b' => GameStatus.redWin, // 黑方长将
    _ => GameStatus.draw,
  };
}

/// 当前行棋后局面（键序列最后一项）的出现次数，供「距判和还差一次」预警。
int repetitionCount(List<String> positionKeys) {
  if (positionKeys.isEmpty) return 0;
  final last = positionKeys.last;
  var n = 0;
  for (final k in positionKeys) {
    if (k == last) n++;
  }
  return n;
}

/// 判断 0 基第 [o2] .. [o3]-1 步构成的周期内，是否为单方长将
/// （该方所有着法均为将军而对方并非如此）。
///
/// 返回 'r'（红方长将）/ 'b'（黑方长将）/ null（双方均长将或均非长将）。
String? longCheckSide(List<bool> givesChecks, int o2, int o3) {
  var redAllCheck = true;
  var blackAllCheck = true;
  for (var k = o2; k < o3; k++) {
    if (givesChecks[k]) continue;
    if (k.isEven) {
      redAllCheck = false; // 红方永远走偶数步（红先行）
    } else {
      blackAllCheck = false;
    }
  }
  if (redAllCheck && !blackAllCheck) return 'r';
  if (blackAllCheck && !redAllCheck) return 'b';
  return null;
}
