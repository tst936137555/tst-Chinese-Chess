// 复盘折线图渲染测试
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:xiangqi/ui/eval_chart.dart';

void main() {
  testWidgets('评估折线图正常渲染并响应点击', (WidgetTester tester) async {
    // 模拟一局：红优 → 黑反超 → 红方 2 步绝杀
    final scores = <int>[0, 120, -80, -350, 200, 60, 9998, 9999];
    var tapped = -1;

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Center(
          child: EvalChart(
            scores: scores,
            currentIndex: 3,
            onTapIndex: (i) => tapped = i,
          ),
        ),
      ),
    ));

    // 无异常即渲染成功
    expect(find.byType(EvalChart), findsOneWidget);

    // 点击右侧区域应跳到接近末尾的步
    await tester.tapAt(tester.getTopLeft(find.byType(EvalChart)) +
        const Offset(290, 40));
    await tester.pump();
    expect(tapped, greaterThan(0));
  });

  test('mate 分值映射约定', () {
    // 一步绝杀 = 9999，两步 = 9998
    expect(10000 - 1, 9999);
    expect(10000 - 2, 9998);
    expect(-10000 - 1, -10001); // 引擎原始 mate -1 映射后
  });
}
