/// 中文纵线记谱法：如「炮二平五」「马8进7」
library;

import 'rules.dart';

/// 红方纵线用中文数字（从红方视角右起一~九），黑方用阿拉伯数字（从黑方视角右起 1~9）
String _fileDigit(int file, bool red) {
  if (red) {
    const cn = ['九', '八', '七', '六', '五', '四', '三', '二', '一'];
    return cn[file];
  } else {
    // 黑方 1~9 从黑方视角右起，即红方视角 file 8 -> 1
    return '${8 - file + 1}';
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
  final fromDigit = _fileDigit(m.fromFile, red);
  final toDigit = _fileDigit(m.toFile, red);

  String action;
  String target;

  if (m.fromFile == m.toFile) {
    // 同一纵线：进/退 + 步数
    final steps = (m.toRank - m.fromRank).abs();
    if (p.type == PieceType.pawn || p.type == PieceType.king || p.type == PieceType.advisor || p.type == PieceType.elephant) {
      // 直行子用步数
      final up = red ? m.toRank < m.fromRank : m.toRank > m.fromRank;
      action = up ? '进' : '退';
      target = red
          ? const ['一', '二', '三', '四', '五', '六', '七', '八', '九'][steps - 1]
          : '$steps';
    } else {
      final up = red ? m.toRank < m.fromRank : m.toRank > m.fromRank;
      action = up ? '进' : '退';
      target = red
          ? const ['一', '二', '三', '四', '五', '六', '七', '八', '九'][steps - 1]
          : '$steps';
    }
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
  return '$name$fromDigit$action$target';
}
