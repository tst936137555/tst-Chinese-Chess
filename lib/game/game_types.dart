/// 对局数据类型：历史记录条目与对局来源模式。
library;

import '../engine/rules.dart';

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
