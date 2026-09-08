/// 对局存档：终局自动归档，供复盘棋谱使用。
library;

import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

/// 最多保留的未收藏对局数
const int kMaxArchivedGames = 100;

/// 一局已结束的对局
class ArchivedGame {
  const ArchivedGame({
    required this.time,
    required this.userRed,
    required this.levelName,
    required this.result,
    required this.history,
    this.favorite = false,
  });

  final DateTime time;
  /// 用户是否执红
  final bool userRed;
  final String levelName;
  /// redWin / blackWin / draw
  final String result;
  /// 走法列表：{uci, captured, notation, fen}
  final List<Map<String, dynamic>> history;

  /// 是否已收藏：置顶显示，且不占 100 局名额、不会被自动移除
  final bool favorite;

  /// 复制并修改收藏状态
  ArchivedGame withFavorite(bool favorite) => ArchivedGame(
        time: time,
        userRed: userRed,
        levelName: levelName,
        result: result,
        history: history,
        favorite: favorite,
      );

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
        'favorite': favorite,
      };

  static ArchivedGame fromJson(Map<String, dynamic> json) => ArchivedGame(
        time: DateTime.fromMillisecondsSinceEpoch(json['time'] as int),
        userRed: json['userRed'] as bool? ?? true,
        levelName: json['levelName'] as String? ?? '',
        result: json['result'] as String? ?? 'draw',
        history: (json['history'] as List? ?? [])
            .map((e) => Map<String, dynamic>.from(e as Map))
            .toList(),
        favorite: json['favorite'] as bool? ?? false,
      );
}

/// 存档服务（基于 SharedPreferences）
class GameArchive {
  static const _key = 'game_archive';

  /// 读取全部存档（新的在前）。
  /// 单条记录损坏时跳过该条，保留其余棋谱；整体结构损坏返回空列表。
  static Future<List<ArchivedGame>> loadAll(SharedPreferences prefs) async {
    List<dynamic> list;
    try {
      final raw = prefs.getString(_key);
      if (raw == null || raw.isEmpty) return [];
      list = jsonDecode(raw) as List;
    } catch (_) {
      return [];
    }
    final games = <ArchivedGame>[];
    for (final e in list) {
      try {
        games.add(
            ArchivedGame.fromJson(Map<String, dynamic>.from(e as Map)));
      } catch (_) {
        // 单条损坏：跳过，不影响其余棋谱
      }
    }
    return games;
  }

  /// 追加一局（插到最前）。
  /// 未收藏对局最多保留 100 局：超出时从最旧的非收藏对局开始移除；
  /// 收藏对局不占名额，不会被自动移除。
  static Future<void> add(SharedPreferences prefs, ArchivedGame game) async {
    try {
      final all = await loadAll(prefs);
      all.insert(0, game);
      var kept = 0;
      final keptGames = <ArchivedGame>[];
      for (final g in all) {
        if (g.favorite) {
          keptGames.add(g);
        } else if (kept < kMaxArchivedGames) {
          kept++;
          keptGames.add(g);
        }
      }
      await prefs.setString(
          _key, jsonEncode(keptGames.map((g) => g.toJson()).toList()));
    } catch (_) {}
  }

  /// 整体覆写保存（收藏切换用；不做裁剪，调用方保证列表已含全部保留项）
  static Future<void> saveAll(
      SharedPreferences prefs, List<ArchivedGame> games) async {
    try {
      await prefs.setString(
          _key, jsonEncode(games.map((g) => g.toJson()).toList()));
    } catch (_) {}
  }

  static Future<void> clear(SharedPreferences prefs) async {
    await prefs.remove(_key);
  }
}
