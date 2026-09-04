/// 对局存档：终局自动归档，供复盘棋谱使用。
library;

import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

/// 最多保留的对局数
const int kMaxArchivedGames = 50;

/// 一局已结束的对局
class ArchivedGame {
  const ArchivedGame({
    required this.time,
    required this.userRed,
    required this.levelName,
    required this.result,
    required this.history,
  });

  final DateTime time;
  /// 用户是否执红
  final bool userRed;
  final String levelName;
  /// redWin / blackWin / draw
  final String result;
  /// 走法列表：{uci, captured, notation, fen}
  final List<Map<String, dynamic>> history;

  String get resultLabel {
    switch (result) {
      case 'redWin':
        return userRed ? '胜' : '负';
      case 'blackWin':
        return userRed ? '负' : '胜';
      default:
        return '和';
    }
  }

  /// 棋谱标题：【对局时间-玩家执红、执黑-当局胜负情况】
  String get title {
    String two(int v) => v.toString().padLeft(2, '0');
    final t = time;
    final timeStr =
        '${t.year}-${two(t.month)}-${two(t.day)} ${two(t.hour)}:${two(t.minute)}';
    final side = userRed ? '执红' : '执黑';
    final outcome = switch (result) {
      'redWin' => '红方胜',
      'blackWin' => '黑方胜',
      _ => '平局',
    };
    return '【$timeStr-$side-$outcome】';
  }

  Map<String, dynamic> toJson() => {
        'time': time.millisecondsSinceEpoch,
        'userRed': userRed,
        'levelName': levelName,
        'result': result,
        'history': history,
      };

  static ArchivedGame fromJson(Map<String, dynamic> json) => ArchivedGame(
        time: DateTime.fromMillisecondsSinceEpoch(json['time'] as int),
        userRed: json['userRed'] as bool? ?? true,
        levelName: json['levelName'] as String? ?? '',
        result: json['result'] as String? ?? 'draw',
        history: (json['history'] as List? ?? [])
            .map((e) => Map<String, dynamic>.from(e as Map))
            .toList(),
      );
}

/// 存档服务（基于 SharedPreferences）
class GameArchive {
  static const _key = 'game_archive';

  /// 读取全部存档（新的在前）
  static Future<List<ArchivedGame>> loadAll(SharedPreferences prefs) async {
    try {
      final raw = prefs.getString(_key);
      if (raw == null || raw.isEmpty) return [];
      final list = jsonDecode(raw) as List;
      return list
          .map((e) => ArchivedGame.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (_) {
      return [];
    }
  }

  /// 追加一局（插到最前，超出上限丢弃最旧的）
  static Future<void> add(SharedPreferences prefs, ArchivedGame game) async {
    try {
      final all = await loadAll(prefs);
      all.insert(0, game);
      if (all.length > kMaxArchivedGames) {
        all.removeRange(kMaxArchivedGames, all.length);
      }
      await prefs.setString(
          _key, jsonEncode(all.map((g) => g.toJson()).toList()));
    } catch (_) {}
  }

  static Future<void> clear(SharedPreferences prefs) async {
    await prefs.remove(_key);
  }
}
