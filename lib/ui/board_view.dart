/// 棋盘绘制与交互组件
library;

import 'package:flutter/material.dart';

import '../engine/rules.dart';
import '../game/review_controller.dart';

/// 棋子显示名（红/黑）
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

/// 棋盘视图组件
///
/// 通过 [onTapSquare] 通知点击事件（file, rank）。
/// [animatingMove] 与 [animationProgress]（0~1）驱动走子动画，
/// [capturedPiece] 用于动画期间渐隐显示被吃棋子。
class BoardView extends StatelessWidget {
  const BoardView({
    super.key,
    required this.board,
    required this.onTapSquare,
    required this.flipBoard,
    this.selected,
    this.legalTargets = const [],
    this.lastMove,
    this.checkPos,
    this.animatingMove,
    this.animationProgress,
    this.capturedPiece,
    this.suggestedMove,
    this.suggestedMoves = const [],
    this.quality,
  });
  final Board board;
  final void Function(int file, int rank) onTapSquare;
  final bool flipBoard;
  final (int, int)? selected;
  final List<(int, int)> legalTargets;
  final Move? lastMove;
  final (int, int)? checkPos;
  final Move? animatingMove;
  final double? animationProgress;
  final Piece? capturedPiece;
  /// 引擎建议走法（复盘时绘制绿色箭头）
  final Move? suggestedMove;
  /// 多个建议走法（提示功能：最优①绿色、次优②蓝色，编号标于箭杆中点）
  final List<Move> suggestedMoves;
  /// 走法质量（复盘时在目标棋子右上角绘制角标）
  final MoveQuality? quality;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, constraints) {
      final size = constraints.biggest;
      // 9 列 10 行，网格线间距
      final cell = size.width / 10;
      final boardHeight = cell * 11;
      return GestureDetector(
        onTapUp: (details) {
          final pos = details.localPosition;
          // 反算网格坐标：交点 (f, r) 绘制于 ((f+1)*cell, (r+1)*cell)
          int file = (pos.dx / cell - 1).round();
          int rank = (pos.dy / cell - 1).round();
          if (file < 0 || file > 8 || rank < 0 || rank > 9) return;
          if (flipBoard) {
            file = 8 - file;
            rank = 9 - rank;
          }
          onTapSquare(file, rank);
        },
        child: SizedBox(
          width: size.width,
          height: boardHeight,
          child: CustomPaint(
            painter: _BoardPainter(
              board: board,
              flip: flipBoard,
              selected: selected,
              legalTargets: legalTargets,
              lastMove: lastMove,
              checkPos: checkPos,
              animatingMove: animatingMove,
              animationProgress: animationProgress,
              capturedPiece: capturedPiece,
              suggestedMove: suggestedMove,
              suggestedMoves: suggestedMoves,
              quality: quality,
              cell: cell,
            ),
          ),
        ),
      );
    });
  }
}

class _BoardPainter extends CustomPainter {
  _BoardPainter({
    required this.board,
    required this.flip,
    required this.selected,
    required this.legalTargets,
    required this.lastMove,
    required this.checkPos,
    required this.animatingMove,
    required this.animationProgress,
    required this.capturedPiece,
    required this.suggestedMove,
    required this.suggestedMoves,
    required this.quality,
    required this.cell,
  });
  final Board board;
  final bool flip;
  final (int, int)? selected;
  final List<(int, int)> legalTargets;
  final Move? lastMove;
  final (int, int)? checkPos;
  final Move? animatingMove;
  final double? animationProgress;
  final Piece? capturedPiece;
  final Move? suggestedMove;
  final List<Move> suggestedMoves;
  final MoveQuality? quality;
  final double cell;

  /// 屏幕坐标（file/rank -> 像素中心点）
  Offset point(int file, int rank) {
    final f = flip ? 8 - file : file;
    final r = flip ? 9 - rank : rank;
    return Offset((f + 1) * cell, (r + 1) * cell);
  }

  @override
  void paint(Canvas canvas, Size size) {
    _drawBackground(canvas, size);
    _drawGrid(canvas);
    _drawMarkings(canvas);
    _drawHighlights(canvas);
    _drawPieces(canvas);
    _drawSuggestion(canvas);
    _drawQualityBadge(canvas);
  }

  /// 引擎建议走法箭头（复盘单箭头 / 提示双箭头，绘制于棋子上层）
  void _drawSuggestion(Canvas canvas) {
    if (suggestedMoves.isNotEmpty) {
      // 提示：最优①（绿色）、次优②（蓝色），编号标在箭杆中点
      for (var i = 0; i < suggestedMoves.length && i < 2; i++) {
        final color = i == 0
            ? const Color(0xFF2E7D32) // 绿
            : const Color(0xFF1565C0); // 蓝
        final m = suggestedMoves[i];
        _drawArrow(canvas, m, color);
        _drawHintBadge(canvas, m, '${i + 1}', color);
      }
      return;
    }
    final m = suggestedMove;
    if (m != null) _drawArrow(canvas, m, const Color(0xFF2E7D32));
  }

  void _drawArrow(Canvas canvas, Move m, Color color) {
    final from = point(m.fromFile, m.fromRank);
    final to = point(m.toFile, m.toRank);
    final paint = Paint()
      ..color = color.withValues(alpha: 0.75)
      ..strokeWidth = cell * 0.12
      ..strokeCap = StrokeCap.round
      ..style = PaintingStyle.stroke;
    // 缩短箭头避免覆盖棋子圆心
    final dir = (to - from) / (to - from).distance;
    final start = from + dir * cell * 0.46;
    final end = to - dir * cell * 0.30;
    canvas.drawLine(start, end, paint);
    // 箭头头部
    final arrowPaint = Paint()
      ..color = color.withValues(alpha: 0.85)
      ..style = PaintingStyle.fill;
    final perpendicular = Offset(-dir.dy, dir.dx);
    final headBase = end - dir * cell * 0.22;
    final headPath = Path()
      ..moveTo(end.dx, end.dy)
      ..lineTo(headBase.dx + perpendicular.dx * cell * 0.14,
          headBase.dy + perpendicular.dy * cell * 0.14)
      ..lineTo(headBase.dx - perpendicular.dx * cell * 0.14,
          headBase.dy - perpendicular.dy * cell * 0.14)
      ..close();
    canvas.drawPath(headPath, arrowPaint);
  }

  /// 提示编号角标（①/②）：绘制于箭杆中点，白边彩底圆牌
  void _drawHintBadge(Canvas canvas, Move m, String label, Color color) {
    final from = point(m.fromFile, m.fromRank);
    final to = point(m.toFile, m.toRank);
    final dir = (to - from) / (to - from).distance;
    final start = from + dir * cell * 0.46;
    final end = to - dir * cell * 0.30;
    final center = Offset.lerp(start, end, 0.45)!;

    final radius = cell * 0.19;
    // 白色描边 + 彩色底，确保在棋盘/棋子上都清晰
    canvas.drawCircle(center, radius + 1.5, Paint()..color = Colors.white);
    canvas.drawCircle(center, radius, Paint()..color = color);

    final tp = TextPainter(
      text: TextSpan(
        text: label,
        style: TextStyle(
          color: Colors.white,
          fontSize: radius * 1.3,
          fontWeight: FontWeight.w700,
          height: 1.0,
        ),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    tp.paint(canvas, center - Offset(tp.width / 2, tp.height / 2));
  }

  /// 走法质量角标（复盘：目标棋子右上角单字角标）
  void _drawQualityBadge(Canvas canvas) {
    final q = quality;
    final m = lastMove;
    if (q == null || m == null) return;
    final to = point(m.toFile, m.toRank);

    // 角标圆形背景：明显大于普通落点标记
    final radius = cell * 0.26;
    final center = to + Offset(cell * 0.34, -cell * 0.34);
    // 白色描边 + 彩色底，确保在棋盘/棋子上都清晰
    canvas.drawCircle(
      center,
      radius + 1.5,
      Paint()..color = Colors.white,
    );
    canvas.drawCircle(
      center,
      radius,
      Paint()..color = q.color,
    );

    // 角标单字（优/良/平/差/错）
    final tp = TextPainter(
      text: TextSpan(
        text: q.badge,
        style: TextStyle(
          color: Colors.white,
          fontSize: radius * 1.35,
          fontWeight: FontWeight.w700,
        ),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    tp.paint(canvas, center - Offset(tp.width / 2, tp.height / 2));
  }

  void _drawBackground(Canvas canvas, Size size) {
    final rect = Rect.fromLTWH(0, 0, size.width, size.height);
    canvas.drawRect(rect, Paint()..color = const Color(0xFFE8C88A));
    // 边框
    final border = Paint()
      ..color = const Color(0xFF5B3A1E)
      ..strokeWidth = 2
      ..style = PaintingStyle.stroke;
    final margin = cell * 0.5;
    canvas.drawRect(
      Rect.fromLTWH(margin, margin, size.width - margin * 2, size.height - margin * 2),
      border,
    );
  }

  void _drawGrid(Canvas canvas) {
    final paint = Paint()
      ..color = const Color(0xFF5B3A1E)
      ..strokeWidth = 1.2;
    // 横线 10 条
    for (int r = 0; r < 10; r++) {
      canvas.drawLine(point(0, r), point(8, r), paint);
    }
    // 竖线 9 条（中间楚河汉界断开）
    for (int f = 0; f < 9; f++) {
      if (f == 0 || f == 8) {
        canvas.drawLine(point(f, 0), point(f, 9), paint);
      } else {
        canvas.drawLine(point(f, 0), point(f, 4), paint);
        canvas.drawLine(point(f, 5), point(f, 9), paint);
      }
    }
    // 九宫斜线
    for (final (fr, tr) in const [(0, 2), (2, 0)]) {
      canvas.drawLine(point(3 + 0, fr), point(5, tr), paint);
      canvas.drawLine(point(5, fr), point(3, tr), paint);
    }
  }

  void _drawMarkings(Canvas canvas) {
    // 楚河汉界
    final tp = TextPainter(
      text: const TextSpan(
        text: '楚 河          汉 界',
        style: TextStyle(
          color: Color(0xFF5B3A1E),
          fontSize: 20,
          fontWeight: FontWeight.bold,
          letterSpacing: 6,
        ),
      ),
      textDirection: TextDirection.ltr,
    );
    tp.layout();
    tp.paint(canvas, Offset((cell * 10 - tp.width) / 2, cell * 5.5 - tp.height / 2));

    // 纵线号：红方一~九（右侧）、黑方 1~9（左侧）
    const redDigits = ['一', '二', '三', '四', '五', '六', '七', '八', '九'];
    TextPainter tpHelper(String s, Color c, double size) => TextPainter(
          text: TextSpan(text: s, style: TextStyle(color: c, fontSize: size)),
          textDirection: TextDirection.ltr,
        )..layout();

    for (int f = 0; f < 9; f++) {
      // 底部红方数字（红方视角 file 8 为"一"）
      final redIdx = 8 - f;
      final t1 = tpHelper(redDigits[redIdx], const Color(0xFF8B2F1F), 13);
      t1.paint(canvas, point(f, 9) + Offset(-t1.width / 2, cell * 0.62));
      // 顶部黑方数字
      final t2 = tpHelper('${f + 1}', const Color(0xFF333333), 12);
      t2.paint(canvas, point(f, 0) + Offset(-t2.width / 2, -cell * 0.62 - t2.height));
    }
  }

  void _drawHighlights(Canvas canvas) {
    // 最后一步标记
    if (lastMove != null) {
      final paint = Paint()
        ..color = Colors.orange.withValues(alpha: 0.35)
        ..style = PaintingStyle.stroke
        ..strokeWidth = 3;
      for (final p in [point(lastMove!.fromFile, lastMove!.fromRank), point(lastMove!.toFile, lastMove!.toRank)]) {
        canvas.drawCircle(p, cell * 0.44, paint);
      }
    }
    // 被将军标记
    if (checkPos != null) {
      final paint = Paint()
        ..color = Colors.red.withValues(alpha: 0.75)
        ..style = PaintingStyle.stroke
        ..strokeWidth = 4;
      canvas.drawCircle(point(checkPos!.$1, checkPos!.$2), cell * 0.46, paint);
    }
    // 选中与可走点
    if (selected != null) {
      canvas.drawCircle(
        point(selected!.$1, selected!.$2),
        cell * 0.44,
        Paint()
          ..color = Colors.green.withValues(alpha: 0.5)
          ..style = PaintingStyle.stroke
          ..strokeWidth = 3,
      );
    }
    for (final t in legalTargets) {
      final hasPiece = board.pieceAt(t.$1, t.$2) != null;
      if (hasPiece) {
        canvas.drawCircle(
          point(t.$1, t.$2),
          cell * 0.42,
          Paint()
            ..color = Colors.redAccent.withValues(alpha: 0.55)
            ..style = PaintingStyle.stroke
            ..strokeWidth = 3,
        );
      } else {
        canvas.drawCircle(
          point(t.$1, t.$2),
          cell * 0.12,
          Paint()..color = Colors.green.withValues(alpha: 0.6),
        );
      }
    }
  }

  void _drawPieces(Canvas canvas) {
    final anim = animatingMove;
    final animating =
        anim != null && animationProgress != null && animationProgress! < 1.0;

    // 被吃棋子：在目标点渐隐
    if (animating && capturedPiece != null) {
      final p = capturedPiece!;
      _drawPiece(
        canvas,
        point(anim.toFile, anim.toRank),
        p,
        cell,
        opacity: (1.0 - animationProgress!) * 0.6,
      );
    }

    for (int rank = 0; rank < 10; rank++) {
      for (int file = 0; file < 9; file++) {
        final p = board.pieceAt(file, rank);
        if (p == null) continue;
        // 动画期间：跳过移动中的棋子（稍后在插值位置绘制）
        if (animating && file == anim.toFile && rank == anim.toRank) {
          continue;
        }
        _drawPiece(canvas, point(file, rank), p, cell);
      }
    }

    // 移动中的棋子：从起点到终点插值
    if (animating) {
      final p = board.pieceAt(anim.toFile, anim.toRank);
      if (p != null) {
        final t = Curves.easeInOutCubic.transform(animationProgress!);
        final from = point(anim.fromFile, anim.fromRank);
        final to = point(anim.toFile, anim.toRank);
        _drawPiece(canvas, Offset.lerp(from, to, t)!, p, cell);
      }
    }
  }

  static void _drawPiece(Canvas canvas, Offset center, Piece p, double cell,
      {double opacity = 1.0}) {
    final radius = cell * 0.42;
    // 阴影
    canvas.drawCircle(
      center.translate(1.5, 2),
      radius,
      Paint()..color = Colors.black.withValues(alpha: 0.25 * opacity),
    );
    // 底色
    canvas.drawCircle(
        center, radius, Paint()..color = const Color(0xFFF3E1B8).withValues(alpha: opacity));
    // 边框
    final ringColor = (p.isRed ? const Color(0xFFB03020) : const Color(0xFF222222))
        .withValues(alpha: opacity);
    canvas.drawCircle(
      center,
      radius,
      Paint()
        ..color = ringColor
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2,
    );
    canvas.drawCircle(
      center,
      radius * 0.84,
      Paint()
        ..color = ringColor
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1,
    );
    // 文字
    final name = (p.isRed ? _redNames : _blackNames)[p.type]!;
    final tp = TextPainter(
      text: TextSpan(
        text: name,
        style: TextStyle(
          color: (p.isRed ? const Color(0xFFB03020) : const Color(0xFF222222))
              .withValues(alpha: opacity),
          fontSize: radius,
          fontWeight: FontWeight.bold,
        ),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    tp.paint(canvas, center - Offset(tp.width / 2, tp.height / 2));
  }

  @override
  bool shouldRepaint(covariant _BoardPainter old) => true;
}
