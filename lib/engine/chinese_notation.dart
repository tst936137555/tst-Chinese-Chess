/// 中文纵线记谱法：如「炮二平五」「马8进7」
library;

import 'rules.dart';

/// 红方纵线用中文数字（从红方视角右起一~九），黑方用阿拉伯数字（从黑方视角右起 1~9）
String _fileDigit(int file, bool red) {
  if (red) {
    const cn = ['九', '八', '七', '六', '五', '四', '三', '二', '一'];
    return cn[file];
  } else {
    // 黑方 1~9 从黑方视角右起（黑方右侧即红方视角 file 0 一侧）：
    // file 0 -> 1 ... file 8 -> 9。
    // 标准对照：h9g7 = 马8进7、b9c7 = 马2进3、车9平8 = i9h9。
    return '${file + 1}';
  }
}

const _redNames = {
  PieceType.king: '帅',
  PieceType.advisor: '仕',
  PieceType.elephant: '相',
  PieceType.horse: '马',
  PieceType.rook: '车',
  PieceType.cannon: '炮',
  PieceType.pawn: '兵',
};

const _blackNames = {
  PieceType.king: '将',
  PieceType.advisor: '士',
  PieceType.elephant: '象',
  PieceType.horse: '马',
  PieceType.rook: '车',
  PieceType.cannon: '炮',
  PieceType.pawn: '卒',
};

/// 生成一步棋的中文记谱。
/// [board] 为走这步棋之前的局面。
String moveToChinese(Board board, Move m) {
  final p = board.pieceAt(m.fromFile, m.fromRank)!;
  final red = p.isRed;
  final name = (red ? _redNames : _blackNames)[p.type]!;
  var fromDigit = _fileDigit(m.fromFile, red);
  final toDigit = _fileDigit(m.toFile, red);

  // 同纵线同名棋子消歧：加前缀（前/中/后，4 个及以上仅可能出现于兵，
  // 按 xqbase 规范用一~五代替），此时省略起始纵线数字。
  // 仕/相不需要前缀：同线时进退方向天然区分（如仕六进五/仕六退五）。
  String prefix = '';
  if (p.type != PieceType.advisor && p.type != PieceType.elephant) {
    final ranks = <int>[];
    for (int rank = 0; rank < 10; rank++) {
      final q = board.pieceAt(m.fromFile, rank);
      if (q != null && q.isRed == red && q.type == p.type) ranks.add(rank);
    }
    if (ranks.length >= 2) {
      // 靠近对方的一侧为"前"：红方 rank 小在前（已按升序收集），
      // 黑方 rank 大在前（需反转）。
      if (!red) {
        for (int i = 0, j = ranks.length - 1; i < j; i++, j--) {
          final t = ranks[i];
          ranks[i] = ranks[j];
          ranks[j] = t;
        }
      }
      final idx = ranks.indexOf(m.fromRank);
      final tags = switch (ranks.length) {
        2 => const ['前', '后'],
        3 => const ['前', '中', '后'],
        _ => const ['一', '二', '三', '四', '五'],
      };
      prefix = tags[idx];
      fromDigit = '';
    }
  }

  String action;
  String target;

  if (m.fromFile == m.toFile) {
    // 同一纵线：进/退 + 步数
    final steps = (m.toRank - m.fromRank).abs();
    final up = red ? m.toRank < m.fromRank : m.toRank > m.fromRank;
    action = up ? '进' : '退';
    target = red
        ? const ['一', '二', '三', '四', '五', '六', '七', '八', '九'][steps - 1]
        : '$steps';
  } else {
    final up = red ? m.toRank < m.fromRank : m.toRank > m.fromRank;
    if (m.toRank == m.fromRank) {
      action = '平';
      target = toDigit;
    } else {
      action = up ? '进' : '退';
      if (p.type == PieceType.horse ||
          p.type == PieceType.advisor ||
          p.type == PieceType.elephant) {
        // 斜行子用目标纵线
        target = toDigit;
      } else {
        // 车炮兵横走（不应出现进退）——此处按平处理
        action = '平';
        target = toDigit;
      }
    }
  }
  return '$prefix$name$fromDigit$action$target';
}
