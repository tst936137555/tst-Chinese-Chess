/// 应用外壳：屏幕常亮路由观察器与 MaterialApp 配置。
library;

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import 'ui/home_screen.dart';
import 'ui/theme.dart';

/// 屏幕常亮路由观察器：对局页 / 复盘列表 / 复盘分析位于栈顶时保持屏幕常亮，
/// 离开这些页面（返回主页等）自动恢复正常熄屏策略。
final wakeLockRouteObserver = WakeLockRouteObserver();

class WakeLockRouteObserver extends RouteObserver<PageRoute<dynamic>> {
  static const _wakelockRoutes = {'/game', '/archive', '/review'};

  void _sync(Route<dynamic>? route) {
    final name = route?.settings.name;
    if (name != null && _wakelockRoutes.contains(name)) {
      WakelockPlus.enable();
    } else {
      WakelockPlus.disable();
    }
  }

  @override
  void didPush(Route<dynamic> route, Route<dynamic>? previousRoute) {
    super.didPush(route, previousRoute);
    _sync(route);
  }

  @override
  void didPop(Route<dynamic> route, Route<dynamic>? previousRoute) {
    super.didPop(route, previousRoute);
    _sync(previousRoute);
  }

  @override
  void didReplace({Route<dynamic>? newRoute, Route<dynamic>? oldRoute}) {
    super.didReplace(newRoute: newRoute, oldRoute: oldRoute);
    _sync(newRoute);
  }

  @override
  void didRemove(Route<dynamic> route, Route<dynamic>? previousRoute) {
    super.didRemove(route, previousRoute);
    _sync(previousRoute);
  }
}

class XiangqiApp extends StatelessWidget {
  const XiangqiApp({super.key, required this.prefs});

  final SharedPreferences prefs;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '中国象棋',
      debugShowCheckedModeBanner: false,
      theme: xiangqiTheme(),
      // 锁定应用内字体：不随系统字体大小缩放，UI 按固定设计尺寸渲染；
      // 桌面端竖屏适配：宽窗口/最大化时所有页面与弹窗以 520 逻辑像素
      // 列宽居中呈现，不随窗口拉伸（列外填宣纸底色与页面无缝衔接）
      builder: (context, child) {
        return MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: TextScaler.noScaling),
          child: ColoredBox(
            color: Theme.of(context).scaffoldBackgroundColor,
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 520),
                child: child!,
              ),
            ),
          ),
        );
      },
      navigatorObservers: [wakeLockRouteObserver],
      home: HomePage(prefs: prefs),
    );
  }
}
