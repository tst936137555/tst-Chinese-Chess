/// 评估折线图：x 轴步数、y 轴评分（红方视角，正分上方红色、负分下方黑色）。
///
/// y 轴采用分段线性刻度：|分| ≤ 500 区间占图高一半（精细），
/// 500 ~ 10000 区间占另一半（绝杀顶到边缘）。胜负分上限 ±10000，
/// mate N 步 = 10000 - N（一步绝杀 9999）。
library;

import 'package:flutter/material.dart';

/// 评估折线图组件
class EvalChart extends StatelessWidget {
  const EvalChart({
    super.key,
    required this.scores,
    this.currentIndex = 0,
    this.onTapIndex,
    this.height = 108,
  });

  /// 评分序列：索引 0 = 初始局面，i = 走完第 i 步
  final List<int> scores;
  /// 当前浏览步数
  final int currentIndex;
  /// 点击某步
  final ValueChanged<int>? onTapIndex;
  final double height;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: height,
      child: LayoutBuilder(builder: (context, constraints) {
        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapUp: (d) {
            final tap = onTapIndex;
            if (tap == null || scores.length < 2) return;
            final w = constraints.maxWidth;
            final i = ((d.localPosition.dx / w) * (scores.length - 1)).round();
            tap(i.clamp(0, scores.length - 1));
          },
          child: CustomPaint(
            size: Size(constraints.maxWidth, height),
            painter: _EvalChartPainter(
              scores: scores,
              currentIndex: currentIndex,
            ),
          ),
        );
      }),
    );
  }
}

class _EvalChartPainter extends CustomPainter {
  _EvalChartPainter({required this.scores, required this.currentIndex});

  final List<int> scores;
  final int currentIndex;

  static const double padLeft = 36;
  static const double padRight = 8;
  static const double padTop = 6;
  static const double padBottom = 14;

  /// 分段刻度：低段边界（±500 以内占半幅）
  static const double lowSeg = 500;
  static const double maxScore = 10000;

  /// 绝杀判定线（±9999 为一步杀）
  static const int mateLine = 9000;

  @override
  void paint(Canvas canvas, Size size) {
    final n = scores.length;
    final plotLeft = padLeft;
    final plotRight = size.width - padRight;
    final plotTop = padTop;
    final plotBottom = size.height - padBottom;
    final plotW = plotRight - plotLeft;
    final plotH = plotBottom - plotTop;
    final cy = plotTop + plotH / 2;

    double xOf(int i) =>
        n <= 1 ? plotLeft : plotLeft + i / (n - 1) * plotW;

    /// |分| → 0..1 归一化（分段线性）
    double norm(double v) {
      if (v <= lowSeg) return v / lowSeg * 0.5;
      return 0.5 + (v - lowSeg) / (maxScore - lowSeg) * 0.5;
    }

    double yOf(int s) {
      final v = norm(s.abs().clamp(0, maxScore).toDouble());
      return cy - (s >= 0 ? v : -v) * (plotH / 2);
    }

    _drawAxes(canvas, plotLeft, plotRight, cy, plotTop, plotBottom);
    _drawCurve(canvas, xOf, yOf, plotTop, cy, plotBottom);
    _drawPoints(canvas, xOf, yOf);
    _drawLabels(canvas, size, xOf, plotLeft, plotRight, plotBottom, cy);
  }

  /// 轴与刻度线
  void _drawAxes(Canvas canvas, double left, double right, double cy,
      double top, double bottom) {
    // 上下绝杀色带（红方绝杀区 / 黑方绝杀区）
    final bandH = (bottom - top) * 0.06;
    canvas.drawRect(
      Rect.fromLTWH(left, top, right - left, bandH),
      Paint()..color = const Color(0xFFB03020).withValues(alpha: 0.10),
    );
    canvas.drawRect(
      Rect.fromLTWH(left, bottom - bandH, right - left, bandH),
      Paint()..color = const Color(0xFF222222).withValues(alpha: 0.10),
    );

    // 0 轴（实线）
    canvas.drawLine(
      Offset(left, cy),
      Offset(right, cy),
      Paint()
        ..color = Colors.grey.withValues(alpha: 0.7)
        ..strokeWidth = 1,
    );
    // ±500 虚线
    final dash = Paint()
      ..color = Colors.grey.withValues(alpha: 0.4)
      ..strokeWidth = 0.8;
    for (final y in [cy - (cy - top) * 0.5, cy + (bottom - cy) * 0.5]) {
      _dashedLine(canvas, Offset(left, y), Offset(right, y), dash);
    }
  }

  void _dashedLine(Canvas canvas, Offset from, Offset to, Paint paint) {
    const dashLen = 4.0;
    const gap = 3.0;
    var dx = 0.0;
    final total = to.dx - from.dx;
    while (dx < total) {
      canvas.drawLine(
        Offset(from.dx + dx, from.dy),
        Offset(from.dx + (dx + dashLen).clamp(0, total), to.dy),
        paint,
      );
      dx += dashLen + gap;
    }
  }

  /// 折线：上半裁剪画红、下半裁剪画黑；附带淡色区域填充
  void _drawCurve(Canvas canvas, double Function(int) xOf, double Function(int) yOf,
      double top, double cy, double bottom) {
    if (scores.length < 2) return;

    final path = Path()..moveTo(xOf(0), yOf(scores[0]));
    for (var i = 1; i < scores.length; i++) {
      path.lineTo(xOf(i), yOf(scores[i]));
    }

    // 区域填充路径（到中线闭合）
    final fill = Path.from(path)
      ..lineTo(xOf(scores.length - 1), cy)
      ..lineTo(xOf(0), cy)
      ..close();

    // 上半：红
    canvas.save();
    canvas.clipRect(Rect.fromLTRB(0, top - 2, double.infinity, cy));
    canvas.drawPath(
        fill, Paint()..color = const Color(0xFFB03020).withValues(alpha: 0.18));
    canvas.drawPath(
      path,
      Paint()
        ..color = const Color(0xFFB03020)
        ..strokeWidth = 2
        ..style = PaintingStyle.stroke
        ..strokeJoin = StrokeJoin.round,
    );
    canvas.restore();

    // 下半：黑
    canvas.save();
    canvas.clipRect(Rect.fromLTRB(0, cy, double.infinity, bottom + 2));
    canvas.drawPath(
        fill, Paint()..color = const Color(0xFF222222).withValues(alpha: 0.18));
    canvas.drawPath(
      path,
      Paint()
        ..color = const Color(0xFF222222)
        ..strokeWidth = 2
        ..style = PaintingStyle.stroke
        ..strokeJoin = StrokeJoin.round,
    );
    canvas.restore();
  }

  /// 数据点与当前指示
  void _drawPoints(Canvas canvas, double Function(int) xOf, double Function(int) yOf) {
    for (var i = 0; i < scores.length; i++) {
      final s = scores[i];
      final c = Offset(xOf(i), yOf(s));
      final isCurrent = i == currentIndex;
      final isMate = s.abs() >= mateLine;
      final color = s >= 0 ? const Color(0xFFB03020) : const Color(0xFF222222);

      if (isCurrent) {
        // 当前步：白底大点 + 光圈
        canvas.drawCircle(
          c,
          5.5,
          Paint()..color = color.withValues(alpha: 0.25),
        );
        canvas.drawCircle(c, 3.5, Paint()..color = Colors.white);
        canvas.drawCircle(
          c,
          3.5,
          Paint()
            ..color = color
            ..style = PaintingStyle.stroke
            ..strokeWidth = 2,
        );
      } else {
        // 普通点；绝杀点画成星形大小（更大更醒目）
        canvas.drawCircle(
          c,
          isMate ? 3.2 : 2.2,
          Paint()..color = color,
        );
        if (isMate) {
          canvas.drawCircle(
            c,
            3.2,
            Paint()
              ..color = Colors.white
              ..style = PaintingStyle.stroke
              ..strokeWidth = 1,
          );
        }
      }
    }
  }

  /// 轴标签
  void _drawLabels(Canvas canvas, Size size, double Function(int) xOf,
      double left, double right, double bottom, double cy) {
    TextPainter tp(String s, Color c, double size, {bool bold = false}) =>
        TextPainter(
          text: TextSpan(
            text: s,
            style: TextStyle(
                color: c, fontSize: size, fontWeight: bold ? FontWeight.w700 : null),
          ),
          textDirection: TextDirection.ltr,
        )..layout();

    // y 轴刻度：±500 / 0
    final y500 = tp('+500', Colors.grey, 9)..layout();
    y500.paint(canvas, Offset(left - y500.width - 3, cy - (bottom - cy) * 0.5 - y500.height / 2));
    final ym500 = tp('-500', Colors.grey, 9)..layout();
    ym500.paint(canvas, Offset(left - ym500.width - 3, cy + (bottom - cy) * 0.5 - ym500.height / 2));
    final y0 = tp('0', Colors.grey, 9)..layout();
    y0.paint(canvas, Offset(left - y0.width - 3, cy - y0.height / 2));

    // x 轴：首/中/尾步数
    final n = scores.length;
    void xLabel(int i) {
      final t = tp('$i', Colors.grey, 9)..layout();
      final x = (xOf(i) - t.width / 2).clamp(0.0, size.width - t.width);
      t.paint(canvas, Offset(x, bottom + 3));
    }

    xLabel(0);
    if (n - 1 >= 8) xLabel((n - 1) ~/ 2);
    if (n >= 2) xLabel(n - 1);
  }

  @override
  bool shouldRepaint(_EvalChartPainter old) =>
      old.scores != scores || old.currentIndex != currentIndex;
}
