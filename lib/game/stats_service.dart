/// 设置与战绩持久化
library;

import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import '../engine/pikafish.dart';

/// 对局统计
class Stats {
  const Stats({this.wins = 0, this.losses = 0, this.draws = 0});
  final int wins;
  final int losses;
  final int draws;

  Stats copyWith({int? wins, int? losses, int? draws}) => Stats(
        wins: wins ?? this.wins,
        losses: losses ?? this.losses,
        draws: draws ?? this.draws,
      );

  factory Stats.fromJsonString(String raw) {
    final data = jsonDecode(raw) as Map<String, dynamic>;
    return Stats(
      wins: data['w'] as int? ?? 0,
      losses: data['l'] as int? ?? 0,
      draws: data['d'] as int? ?? 0,
    );
  }

  String toJsonString() => jsonEncode({'w': wins, 'l': losses, 'd': draws});
}

/// 战绩记录服务
class StatsService {
  static const _key = 'stats';
  Stats _stats = const Stats();
  Stats get stats => _stats;

  final Map<String, Stats> _byLevel = {};
  Stats statsForLevel(String levelName) => _byLevel[levelName] ?? const Stats();

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_key);
    if (raw != null) {
      _stats = Stats.fromJsonString(raw);
    }
    // 按难度分别记录
    for (final level in DifficultyLevel.all) {
      final r = prefs.getString('stats_${level.name}');
      if (r != null) {
        _byLevel[level.name] = Stats.fromJsonString(r);
      }
    }
  }

  Future<void> record(GameOutcome outcome, String levelName) async {
    final prefs = await SharedPreferences.getInstance();
    _stats = _stats.copyWith(
      wins: outcome == GameOutcome.win ? _stats.wins + 1 : null,
      losses: outcome == GameOutcome.loss ? _stats.losses + 1 : null,
      draws: outcome == GameOutcome.draw ? _stats.draws + 1 : null,
    );
    await prefs.setString(_key, _stats.toJsonString());

    final cur = _byLevel[levelName] ?? const Stats();
    _byLevel[levelName] = cur.copyWith(
      wins: outcome == GameOutcome.win ? cur.wins + 1 : null,
      losses: outcome == GameOutcome.loss ? cur.losses + 1 : null,
      draws: outcome == GameOutcome.draw ? cur.draws + 1 : null,
    );
    await prefs.setString(
        'stats_$levelName', _byLevel[levelName]!.toJsonString());
  }
}

enum GameOutcome { win, loss, draw }
