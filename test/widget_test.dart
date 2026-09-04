// 主界面冒烟测试：验证应用可启动并显示棋盘界面。
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:xiangqi/main.dart';

void main() {
  testWidgets('应用启动显示棋盘界面', (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await tester.pumpWidget(XiangqiApp(prefs: prefs));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('中国象棋'), findsWidgets);
  });
}
