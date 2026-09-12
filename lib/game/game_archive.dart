/// 对局存档：终局自动归档，供复盘棋谱使用。
/// 存储于应用支持目录的 JSON 文件（经 path_provider 解析），
/// 旧版本 SharedPreferences 中的棋谱在启动时一次性迁移（见 [GameArchive.migrateFromPrefs]）。
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// 最多保留的未收藏对局数
const int kMaxArchivedGames = 100;

/// 最多保留的收藏对局数（收藏同样有上限，超出时从最旧的收藏开始移除）
const int kMaxFavoriteGames = 50;

/// 一局已结束的对局
class ArchivedGame {
  const ArchivedGame({
    required this.time,
    required this.userRed,
    required this.levelName,
    required this.result,
    required this.history,
    this.favorite = false,
    this.startFen,
    this.mode = 'normal',
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

  /// 起始局面 FEN：null = 标准开局；复盘续下为自定义局面，
  /// 复盘重放以此为基准
  final String? startFen;

  /// 对局来源模式：normal / review（复盘续下），
  /// 标题前缀（（复盘））由此生成；旧档缺省为 normal
  final String mode;

  /// 复制并修改收藏状态
  ArchivedGame withFavorite(bool favorite) => ArchivedGame(
        time: time,
        userRed: userRed,
        levelName: levelName,
        result: result,
        history: history,
        favorite: favorite,
        startFen: startFen,
        mode: mode,
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

  /// 棋谱标题：【对局时间-玩家执红、执黑-当局胜负情况】；
  /// 非普通对局在原题前加模式名（（复盘））
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
    final modePrefix = switch (mode) {
      'review' => '（复盘）',
      _ => '',
    };
    return '$modePrefix【$timeStr-$side-$outcome】';
  }

  Map<String, dynamic> toJson() => {
        'time': time.millisecondsSinceEpoch,
        'userRed': userRed,
        'levelName': levelName,
        'result': result,
        'history': history,
        'favorite': favorite,
        'startFen': startFen,
        'mode': mode,
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
        startFen: json['startFen'] as String?,
        mode: json['mode'] as String? ?? 'normal',
      );
}

/// 存档服务（JSON 文件存储）。
/// 所有方法显式传入目标 [File]：调用方经 [defaultArchiveFile] 获取默认文件，
/// 测试可直接传入临时目录文件，不依赖平台通道。
class GameArchive {
  static const _prefsKey = 'game_archive';
  static const _fileName = 'game_archive.json';

  /// 读-改-写串行锁：并发 add / 收藏切换 / 清空 / 迁移经 [_locked]
  /// 排队串行，整体不可分割，避免并发追加各自读到旧快照互相覆盖丢局。
  static Future<void> _writeLock = Future.value();

  /// 在写锁内执行 [action]（非重入：闭包内不得再调用拿锁的方法）
  static Future<T> _locked<T>(Future<T> Function() action) {
    final next = _writeLock.then<T>((_) => action());
    _writeLock = next.then<void>((_) {}, onError: (_) {});
    return next;
  }

  /// 默认存档文件路径的解析缓存（平台通道只调一次）
  static Future<File>? _fileFuture;

  /// 默认存档文件：应用支持目录下 game_archive.json
  static Future<File> defaultArchiveFile() =>
      _fileFuture ??= _resolveDefaultFile();

  static Future<File> _resolveDefaultFile() async {
    final dir = await getApplicationSupportDirectory();
    return File('${dir.path}${Platform.pathSeparator}$_fileName');
  }

  /// 读取全部存档（新的在前）。
  /// 文件不存在返回空列表；单条记录损坏时跳过该条，保留其余棋谱；
  /// 整体结构损坏返回空列表（调用方可用"文件存在且非空但结果为空"感知并提示）。
  static Future<List<ArchivedGame>> loadAll(File file) async {
    String raw;
    try {
      if (!await file.exists()) return [];
      raw = await file.readAsString();
      if (raw.isEmpty) return [];
      final list = jsonDecode(raw) as List;
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
    } catch (e) {
      debugPrint('棋谱读取失败: $e');
      return [];
    }
  }

  /// 追加一局（插到最前）。
  /// 未收藏对局最多 100 局、收藏对局最多 50 局：各自超出时从最旧开始移除。
  /// 返回 false = 写入失败（调用方负责向用户提示）。
  static Future<bool> add(File file, ArchivedGame game) =>
      _locked(() async {
        final all = await loadAll(file);
        all.insert(0, game);
        return _trimAndSave(file, all);
      });

  /// 整体覆写保存（收藏切换用；不做裁剪，调用方保证列表合法）。
  /// 返回 false = 写入失败（调用方负责向用户提示）。
  static Future<bool> saveAll(File file, List<ArchivedGame> games) =>
      _locked(() => _writeLocked(file, games));

  /// 清空存档（删除存档文件）。
  /// 返回 false = 删除失败（调用方负责向用户提示）。
  static Future<bool> clear(File file) => _locked(() async {
        try {
          if (await file.exists()) await file.delete();
          // 顺带清理残留的临时文件，保持目录干净
          await _deleteQuietly(File('${file.path}.tmp'));
          return true;
        } catch (e) {
          debugPrint('棋谱清空失败: $e');
          return false;
        }
      });

  /// 一次性迁移：旧版 SharedPreferences 中的整体棋谱迁入 [target] 文件。
  /// 返回迁移条数；
  /// 0 = 无旧数据或目标文件已存在（幂等跳过）；
  /// -1 = 旧数据不可恢复损坏（已清除 prefs 键，避免每次启动重复解析）；
  /// -2 = 写入文件失败（保留 prefs 键，下次启动重试）。
  static Future<int> migrateFromPrefs(SharedPreferences prefs,
          {File? target}) =>
      _locked(() => _migrateLocked(prefs, target));

  /// 迁移主体（须在写锁内调用）：目标文件存在性检查与写入同锁，
  /// 避免与并发归档交错（检查时不存在、写入前被归档建立文件而被覆盖）。
  static Future<int> _migrateLocked(
      SharedPreferences prefs, File? target) async {
    final file = target ?? await defaultArchiveFile();
    if (await file.exists()) return 0;
    final raw = prefs.getString(_prefsKey);
    if (raw == null || raw.isEmpty) return 0;
    List<dynamic> list;
    try {
      list = jsonDecode(raw) as List;
    } catch (_) {
      // 旧数据已不可读：清键防止每次启动重复尝试
      await prefs.remove(_prefsKey);
      return -1;
    }
    if (list.isEmpty) {
      await prefs.remove(_prefsKey);
      return 0;
    }
    final games = <ArchivedGame>[];
    for (final e in list) {
      try {
        games.add(ArchivedGame.fromJson(Map<String, dynamic>.from(e as Map)));
      } catch (_) {
        // 单条损坏：跳过
      }
    }
    // 写文件全部成功后才清 prefs 键，任何失败保留旧数据下次重试
    if (!await _writeLocked(file, games)) return -2;
    await prefs.remove(_prefsKey);
    return games.length;
  }

  /// 双上限裁剪后保存
  static Future<bool> _trimAndSave(File file, List<ArchivedGame> all) {
    var plain = 0;
    var fav = 0;
    final kept = <ArchivedGame>[];
    for (final g in all) {
      if (g.favorite) {
        if (fav < kMaxFavoriteGames) {
          fav++;
          kept.add(g);
        }
      } else if (plain < kMaxArchivedGames) {
        plain++;
        kept.add(g);
      }
    }
    return _writeLocked(file, kept);
  }

  /// 锁内切换收藏：重读文件最新内容定位条目改写，不依赖界面持有的旧快照
  /// （避免归档页打开期间新终局归档落盘后被旧快照覆写丢失）。
  /// 条目以 [keyOf]（time+userRed）定位；返回切换后的最新完整列表，
  /// 保存失败返回 null（调用方回滚界面并提示）。
  static Future<List<ArchivedGame>?> toggleFavorite(
      File file, ArchivedGame game) {
    final key = keyOf(game);
    return _locked(() async {
      final all = await loadAll(file);
      final idx = all.indexWhere((g) => keyOf(g) == key);
      if (idx < 0) return null;
      if (!game.favorite && _favCount(all) >= kMaxFavoriteGames) {
        // 切为收藏将超上限：拒绝，返回当前列表供界面刷新
        return all;
      }
      final updated = all[idx].withFavorite(!game.favorite);
      all[idx] = updated;
      if (!await _writeLocked(file, all)) return null;
      return all;
    });
  }

  /// 条目身份键：归档时间 + 执子方（同一时刻仅一局在录，不冲突）
  static (int, bool) keyOf(ArchivedGame g) => (g.time.millisecondsSinceEpoch, g.userRed);

  /// 锁内删除单条棋谱：重读文件最新内容按 [keyOf] 定位后移除，
  /// 不依赖界面持有的旧快照（同 [toggleFavorite]，
  /// 避免归档页打开期间新终局归档落盘后被旧快照覆写丢失）。
  /// 条目已不存在时视为成功（幂等），返回当前最新完整列表；
  /// 返回 null = 写入失败（调用方回滚界面并提示）。
  static Future<List<ArchivedGame>?> remove(File file, ArchivedGame game) {
    final key = keyOf(game);
    return _locked(() async {
      final all = await loadAll(file);
      final idx = all.indexWhere((g) => keyOf(g) == key);
      if (idx >= 0) {
        all.removeAt(idx);
        if (!await _writeLocked(file, all)) return null;
      }
      return all;
    });
  }

  static int _favCount(List<ArchivedGame> all) {
    var n = 0;
    for (final g in all) {
      if (g.favorite) n++;
    }
    return n;
  }

  /// 原子写入（须在写锁内调用）：临时文件 → 回读校验 → 改名替换。
  /// 返回 false = 任一步失败（旧文件保持完好，临时文件尽力清理）。
  static Future<bool> _writeLocked(File file, List<ArchivedGame> games) async {
    final tmp = File('${file.path}.tmp');
    try {
      await file.parent.create(recursive: true);
      final json = jsonEncode(games.map((g) => g.toJson()).toList());
      await tmp.writeAsString(json, flush: true);
      // 回读校验，避免半截文件替换旧档
      if (await tmp.readAsString() != json) {
        await _deleteQuietly(tmp);
        return false;
      }
      // 同目录 rename：原子替换既有文件
      await tmp.rename(file.path);
      return true;
    } catch (e) {
      debugPrint('棋谱写入失败: $e');
      await _deleteQuietly(tmp);
      return false;
    }
  }

  /// 静默删除（不存在 / 已被外部清理 / 收尾竞态均视为成功）
  static Future<void> _deleteQuietly(File f) async {
    try {
      if (await f.exists()) await f.delete();
    } catch (_) {}
  }
}
