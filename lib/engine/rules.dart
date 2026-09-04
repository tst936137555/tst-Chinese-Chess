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
    _loadFen(_startFen);
  }

  Board.cloneFrom(Board other) {
    for (int i = 0; i < 90; i++) {
      _squares[i] = other._squares[i];
    }
    redToMove = other.redToMove;
  }

  static const _startFen =
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
      // 黑卒攻击红帅
      final p1 = kr + 1 <= 9 ? board.pieceAt(kf, kr + 1) : null;
      if (p1 != null && !p1.isRed && p1.type == PieceType.pawn) return true;
      if (kr >= 5) {
        // 帅在黑方半场，黑卒可横吃
        for (final df in const [-1, 1]) {
          final f = kf + df;
          if (f < 0 || f > 8) continue;
          final p = board.pieceAt(f, kr);
          if (p != null && !p.isRed && p.type == PieceType.pawn) return true;
        }
      }
    } else {
      // 红兵攻击黑将
      final p1 = kr - 1 >= 0 ? board.pieceAt(kf, kr - 1) : null;
      if (p1 != null && p1.isRed && p1.type == PieceType.pawn) return true;
      if (kr <= 4) {
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

  /// 双方是否照面（非法局面）
  bool kingsFacing() {
    final rk = findKing(true);
    final bk = findKing(false);
    if (rk == null || bk == null) return false;
    if (rk.$1 != bk.$1) return false;
    for (int r = bk.$2 + 1; r < rk.$2; r++) {
      if (pieceAt(rk.$1, r) != null) return false;
    }
    return true;
  }

  /// 将死 / 困毙判定（在当前方走之前调用）
  GameStatus statusAfterMove() {
    if (legalMoves().isEmpty) {
      // 无子可动：被将军则将死，否则困毙，均为对方胜
      return redToMove ? GameStatus.blackWin : GameStatus.redWin;
    }
    return GameStatus.playing;
  }

  /// 简单长将检测：检查最近 24 步内是否出现同一局面 3 次
  static GameStatus repetitionStatus(List<String> historyFens) {
    if (historyFens.length < 12) return GameStatus.playing;
    final recent = historyFens.sublist(historyFens.length - 24);
    final last = recent.last;
    final count = recent.where((f) => f == last).length;
    if (count >= 3) return GameStatus.draw;
    return GameStatus.playing;
  }
}
