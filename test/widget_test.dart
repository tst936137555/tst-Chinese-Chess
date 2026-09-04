// 主界面冒烟测试：验证应用可启动并显示入口选择界面。
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:xiangqi/main.dart';

void main() {
  testWidgets('应用启动显示入口选择', (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await tester.pumpWidget(XiangqiApp(prefs: prefs));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('中国象棋'), findsWidgets);
    expect(find.text('新开局'), findsOneWidget);
    // 无存档时继续上局按钮不可用，但仍在
    expect(find.textContaining('继续上局'), findsOneWidget);
  });

  testWidgets('新开局流程：选难度后选执子', (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await tester.pumpWidget(XiangqiApp(prefs: prefs));
    await tester.pump();

    // 点击新开局
    await tester.tap(find.text('新开局'));
    await tester.pumpAndSettle();

    // 出现难度选择
    expect(find.text('选择难度'), findsOneWidget);
    await tester.tap(find.text('中等'));
    await tester.pumpAndSettle();

    // 出现执子选择
    expect(find.text('选择执子'), findsOneWidget);
    await tester.tap(find.text('执红'));
    // GamePage 异步初始化：pump 直到对局界面出现
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();

    // 进入对局界面（棋盘加载）
    expect(find.text('对局'), findsOneWidget);
  });
}
