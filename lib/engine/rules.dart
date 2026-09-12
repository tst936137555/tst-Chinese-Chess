/// 中国象棋棋规核心：棋盘表示、FEN 解析、走法生成、胜负判定。
///
/// 坐标系统：file 0-8 从左到右（红方视角），rank 0-9 从上到下（rank 0 为黑方底线）。
/// 走法格式采用 UCI 风格，如 h2e2。
library;

import 'dart:typed_data';

/// Zobrist 局面哈希随机表：固定种子的 xorshift64* 生成，
/// 同一棋盘 + 行棋方状态在任何进程/平台下哈希恒定。
/// 供 [Board.makeMove]/[Board.undoMove] 以 O(1) 增量维护局面键，
/// 重复局面检测不再全量解析 FEN（64 位空间内单局碰撞概率可忽略）。
final class _Zobrist {
  /// 键数量：7 类棋子 × 2 色 × 90 格，末位 1 个为行棋方键
  static final Int64List _keys = _build();

  /// 行棋方键：红方行棋时异或之（与 [Board._computeHash] 约定一致）
  static final int sideKey = _keys[_keys.length - 1];

  /// 棋子位于 (file, rank) 的键
  static int key(Piece p, int file, int rank) {
    final typeIdx = switch (p.type) {
      PieceType.king => 0,
      PieceType.advisor => 1,
      PieceType.elephant => 2,
      PieceType.horse => 3,
      PieceType.rook => 4,
      PieceType.cannon => 5,
      PieceType.pawn => 6,
    };
    return _keys[(typeIdx * 2 + (p.isRed ? 0 : 1)) * 90 + rank * 9 + file];
  }

  static Int64List _build() {
    final t = Int64List(14 * 90 + 1);
    var s = 0x9E3779B97F4A7C15; // 黄金分割常数种子
    for (var i = 0; i < t.length; i++) {
      s ^= s >> 12;
      s ^= s << 25;
      s ^= s >> 27;
      t[i] = s * 0x2545F4914F6CDD1D;
    }
    return t;
  }
}

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

  /// UCI 走法格式：纵线 a-i、横线数字 0-9，共 4 字符（如 "a0i9"）。
  /// 严格校验拒绝任何越界/错位字符，杜绝 "j1i1" 之类被静默映射到棋盘内的错误位置。
  static final RegExp _uciPattern = RegExp(r'^[a-i][0-9][a-i][0-9]$');

  /// 从 UCI 字符串（如 "a0a1"）解析；格式不合法时抛 [FormatException]
  factory Move.fromUci(String uci) {
    if (!_uciPattern.hasMatch(uci)) {
      throw FormatException('无效的 UCI 走法: $uci');
    }
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

  /// 半回合计数：自最近一次吃子以来的半回合数（自然限着用，吃子清零）
  int _halfmoveClock = 0;

  /// 回合数（黑方走完后 +1，标准 FEN 字段）
  int _fullmoveNumber = 1;

  /// 局面 Zobrist 哈希（棋盘 + 行棋方，不含着法计数）。
  /// makeMove/undoMove 增量维护，_loadFen 全量重算，undoMove 不逆推半回合计数。
  int _hash = 0;

  /// 局面哈希（供重复局面检测 O(1) 取键）
  int get positionHash => _hash;

  /// 半回合计数（自最近一次吃子以来的半回合数）
  int get halfmoveClock => _halfmoveClock;

  /// 自然限着：双方连续 60 回合（120 半回合）无吃子判和（中象规则自然限着）
  static const naturalDrawHalfmoves = 120;

  /// 是否已达自然限着（60 回合无吃子）
  bool get isNaturalDraw => _halfmoveClock >= naturalDrawHalfmoves;

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
    sb.write(' ${redToMove ? 'w' : 'b'} - - $_halfmoveClock $_fullmoveNumber');
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
    _halfmoveClock = other._halfmoveClock;
    _fullmoveNumber = other._fullmoveNumber;
    _hash = other._hash;
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
    // 着法计数字段（半回合数 / 回合数）：缺失或非法时取默认值，
    // 兼容旧存档中恒为 "0 1" 的历史 FEN
    _halfmoveClock = parts.length > 4 ? int.tryParse(parts[4]) ?? 0 : 0;
    _fullmoveNumber = parts.length > 5 ? int.tryParse(parts[5]) ?? 1 : 1;
    _hash = _computeHash();
  }

  /// 全量计算局面 Zobrist 哈希（_loadFen 用；增量维护见 makeMove/undoMove）
  int _computeHash() {
    var h = 0;
    for (int rank = 0; rank < 10; rank++) {
      for (int file = 0; file < 9; file++) {
        final p = _squares[rank * 9 + file];
        if (p != null) h ^= _Zobrist.key(p, file, rank);
      }
    }
    if (redToMove) h ^= _Zobrist.sideKey;
    return h;
  }

  /// 执行走法（须为合法走法）。
  /// 同步增量维护 Zobrist 哈希与半回合计数（吃子清零，其余 +1）。
  void makeMove(Move m) {
    final p = pieceAt(m.fromFile, m.fromRank);
    final captured = pieceAt(m.toFile, m.toRank);
    if (p != null) {
      _hash ^= _Zobrist.key(p, m.fromFile, m.fromRank);
      _hash ^= _Zobrist.key(p, m.toFile, m.toRank);
    }
    if (captured != null) {
      _hash ^= _Zobrist.key(captured, m.toFile, m.toRank);
      _halfmoveClock = 0;
    } else {
      _halfmoveClock++;
    }
    _set(m.toFile, m.toRank, p);
    _set(m.fromFile, m.fromRank, null);
    redToMove = !redToMove;
    _hash ^= _Zobrist.sideKey;
    if (redToMove) _fullmoveNumber++; // 黑方走完，回合数 +1
  }

  /// 撤销走法并还原被吃棋子。
  /// Zobrist 哈希与回合数精确还原；半回合计数无法增量逆推
  /// （走子前值未知），不在此还原——生产对局路径经 FEN 重建棋盘，
  /// 合法性探测（isLegal）自行保存/还原计数。
  void undoMove(Move m, Piece? captured) {
    final p = pieceAt(m.toFile, m.toRank)!;
    // 与 makeMove 的增量更新逐项异或（异或满足交换律，顺序无关）
    _hash ^= _Zobrist.key(p, m.toFile, m.toRank);
    _hash ^= _Zobrist.key(p, m.fromFile, m.fromRank);
    if (captured != null) {
      _hash ^= _Zobrist.key(captured, m.toFile, m.toRank);
    }
    _set(m.fromFile, m.fromRank, p);
    _set(m.toFile, m.toRank, captured);
    redToMove = !redToMove;
    _hash ^= _Zobrist.sideKey;
    if (!redToMove) _fullmoveNumber--; // 撤销的是黑方走法
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

  /// 走法是否合法（不送将）。
  /// 探测用 makeMove/undoMove 会推进半回合计数，这里保存/还原，
  /// 保证合法走法生成不污染自然限着计数。
  bool isLegal(Move m) {
    final p = pieceAt(m.fromFile, m.fromRank);
    if (p == null || p.isRed != redToMove) return false;
    if (!_pieceMoves(m.fromFile, m.fromRank).contains(m)) return false;
    final clockBefore = _halfmoveClock;
    final captured = pieceAt(m.toFile, m.toRank);
    makeMove(m);
    final bad = _inCheck(this, !redToMove); // 检查走棋方（已翻转）是否被将
    undoMove(m, captured);
    _halfmoveClock = clockBefore;
    return !bad;
  }

  /// 所有合法走法。
  /// pseudoMoves 生成的走法天然通过来源校验，这里直接逐个
  /// make/undo 探测送将即可，省去 isLegal 内对每个走法重新
  /// 生成整子走法列表的冗余计算（约减 60% 走法生成开销）；
  /// isLegal 保留给外部传入走法（如引擎 UCI 回包）时验证使用。
  List<Move> legalMoves() {
    final result = <Move>[];
    for (final m in pseudoMoves()) {
      final clockBefore = _halfmoveClock;
      final captured = pieceAt(m.toFile, m.toRank);
      makeMove(m);
      final bad = _inCheck(this, !redToMove); // 检查走棋方（已翻转）是否被将
      undoMove(m, captured);
      _halfmoveClock = clockBefore;
      if (!bad) result.add(m);
    }
    return result;
  }

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
/// 键须唯一标识「棋盘 + 行棋方」组合（Zobrist 哈希或 FEN 前两段皆可，
/// 不含着法计数等无关字段）。[givesChecks][k] 表示第 k 步（0 基）走完后
/// 对方是否被将军。
///
/// 同一局面出现 3 次即触发：取最后一个完整重复周期分析——
/// 周期内若一方所有着法均为将军而对方并非如此，则该方长将判负；
/// 双方均长将（或均非长将）判和。未重复 3 次返回 null（对局继续）。
GameStatus? repetitionStatus(
    List<Object> positionKeys, List<bool> givesChecks) {
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
int repetitionCount(List<Object> positionKeys) {
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
