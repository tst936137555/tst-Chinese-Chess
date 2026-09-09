// 存档服务测试：文件存储、双上限裁剪、序列化往返、旧版数据迁移
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:tst_xiangqi/game/game_archive.dart';

ArchivedGame _game(int i, {bool favorite = false}) => ArchivedGame(
      time: DateTime(2026, 1, 1).add(Duration(minutes: i)),
      userRed: true,
      levelName: '中等',
      result: 'draw',
      history: const [],
      favorite: favorite,
    );

void main() {
  late Directory tmpDir;
  late File file;

  setUp(() async {
    tmpDir = await Directory.systemTemp.createTemp('xq_archive_test');
    file = File('${tmpDir.path}${Platform.pathSeparator}archive.json');
  });

  tearDown(() async {
    if (await tmpDir.exists()) {
      await tmpDir.delete(recursive: true);
    }
  });

  test('文件不存在时读取返回空列表', () async {
    expect(await GameArchive.loadAll(file), isEmpty);
  });

  test('超出 100 局时移除最旧的未收藏对局', () async {
    for (var i = 0; i < 105; i++) {
      expect(await GameArchive.add(file, _game(i)), isTrue);
    }
    final all = await GameArchive.loadAll(file);
    expect(all.length, 100);
    // 最新在前；最旧的 5 局（i=0..4）被移除
    expect(
        all.first.time, DateTime(2026, 1, 1).add(const Duration(minutes: 104)));
    expect(
        all.last.time, DateTime(2026, 1, 1).add(const Duration(minutes: 5)));
  });

  test('收藏不占未收藏名额，但自身最多 50 条（最旧收藏被移除）', () async {
    // 51 局收藏 + 100 局未收藏 → 收藏剩 50（最旧 1 局被移除），未收藏 100
    for (var i = 0; i < 51; i++) {
      expect(await GameArchive.add(file, _game(i, favorite: true)), isTrue);
    }
    for (var i = 100; i < 200; i++) {
      expect(await GameArchive.add(file, _game(i)), isTrue);
    }
    final all = await GameArchive.loadAll(file);
    expect(all.where((g) => g.favorite).length, 50);
    expect(all.where((g) => !g.favorite).length, 100);
    // 最旧的收藏（i=0）被移除，i=1 仍在
    expect(all.any((g) => g.favorite && g.time.minute == 0), isFalse);
    expect(all.any((g) => g.favorite && g.time.minute == 1), isTrue);
  });

  test('收藏状态序列化往返一致，saveAll 可切换', () async {
    expect(await GameArchive.add(file, _game(1)), isTrue);
    expect(await GameArchive.add(file, _game(2, favorite: true)), isTrue);
    var all = await GameArchive.loadAll(file);
    expect(all.length, 2);
    expect(all[0].favorite, isTrue);
    expect(all[1].favorite, isFalse);

    // 切换收藏并整体保存
    expect(
        await GameArchive.saveAll(file, [
          all[0].withFavorite(false),
          all[1].withFavorite(false),
        ]),
        isTrue);
    all = await GameArchive.loadAll(file);
    expect(all.every((g) => !g.favorite), isTrue);
  });

  test('单条损坏记录被跳过，其余棋谱保留', () async {
    await file.writeAsString(
        '[{"time":1,"history":[]},"oops",{"time":2,"history":[]}]');
    final all = await GameArchive.loadAll(file);
    expect(all.length, 2);
  });

  test('整体损坏返回空列表', () async {
    await file.writeAsString('not json');
    expect(await GameArchive.loadAll(file), isEmpty);
  });

  test('写入失败返回 false（临时文件路径被目录占用）', () async {
    await Directory('${file.path}.tmp').create();
    expect(await GameArchive.add(file, _game(1)), isFalse);
  });

  test('migrateFromPrefs：正常迁移后 prefs 键被清且文件内容一致', () async {
    SharedPreferences.setMockInitialValues({
      'game_archive': jsonEncode([
        _game(1).toJson(),
        _game(2, favorite: true).toJson(),
      ]),
    });
    final prefs = await SharedPreferences.getInstance();
    final n = await GameArchive.migrateFromPrefs(prefs, target: file);
    expect(n, 2);
    expect(prefs.getString('game_archive'), isNull);
    final all = await GameArchive.loadAll(file);
    expect(all.length, 2);
    // 迁移保持原列表顺序（旧→新）
    expect(all[1].favorite, isTrue);
  });

  test('migrateFromPrefs：目标文件已存在时幂等跳过', () async {
    await file.writeAsString('keep');
    SharedPreferences.setMockInitialValues({'game_archive': '[]'});
    final prefs = await SharedPreferences.getInstance();
    expect(await GameArchive.migrateFromPrefs(prefs, target: file), 0);
    expect(await file.readAsString(), 'keep');
  });

  test('migrateFromPrefs：旧数据为空数组时清除键并返回 0', () async {
    SharedPreferences.setMockInitialValues({'game_archive': '[]'});
    final prefs = await SharedPreferences.getInstance();
    expect(await GameArchive.migrateFromPrefs(prefs, target: file), 0);
    expect(prefs.getString('game_archive'), isNull);
    expect(file.existsSync(), isFalse);
  });

  test('migrateFromPrefs：旧数据不可恢复损坏时清除 prefs 键', () async {
    SharedPreferences.setMockInitialValues({'game_archive': '{broken'});
    final prefs = await SharedPreferences.getInstance();
    expect(await GameArchive.migrateFromPrefs(prefs, target: file), -1);
    expect(prefs.getString('game_archive'), isNull);
    expect(file.existsSync(), isFalse);
  });
}
