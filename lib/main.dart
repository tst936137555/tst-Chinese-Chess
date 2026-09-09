/// 中国象棋主入口：单机对弈（内置皮卡鱼引擎，无网络功能）。
///
/// 应用结构（拆分后）：
/// - app.dart：MaterialApp 配置与屏幕常亮路由观察器；
/// - ui/home_screen.dart：主界面（继续上局 / 新开局 / 复盘棋谱入口）；
/// - ui/game_screen.dart：对局界面（棋盘交互、走子动画、终局遮罩）；
/// - ui/announcement_dialog.dart：作者声明对话框；
/// - ui/review_screen.dart：复盘列表与复盘分析界面；
/// - game/：对局与复盘控制器、存档、音效；engine/：引擎对接与规则。
library;

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'app.dart';
import 'game/game_archive.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  // 旧版 SharedPreferences 棋谱一次性迁移到文件（幂等；失败保留旧数据下次重试）
  await GameArchive.migrateFromPrefs(prefs);
  runApp(XiangqiApp(prefs: prefs));
}
