// 存档服务测试：50 局裁剪、收藏保护、序列化往返
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
  test('超出 100 局时移除最旧的未收藏对局', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    for (var i = 0; i < 105; i++) {
      await GameArchive.add(prefs, _game(i));
    }
    final all = await GameArchive.loadAll(prefs);
    expect(all.length, 100);
    // 最新在前；最旧的 5 局（i=0..4）被移除
    expect(
        all.first.time, DateTime(2026, 1, 1).add(const Duration(minutes: 104)));
    expect(
        all.last.time, DateTime(2026, 1, 1).add(const Duration(minutes: 5)));
  });

  test('收藏对局不占名额、不会被自动移除', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    // 最旧的 3 局收藏
    await GameArchive.add(prefs, _game(0, favorite: true));
    await GameArchive.add(prefs, _game(1, favorite: true));
    await GameArchive.add(prefs, _game(2, favorite: true));
    for (var i = 3; i < 110; i++) {
      await GameArchive.add(prefs, _game(i));
    }
    final all = await GameArchive.loadAll(prefs);
    // 3 局收藏 + 100 局未收藏
    expect(all.where((g) => g.favorite).length, 3);
    expect(all.where((g) => !g.favorite).length, 100);
    // 最旧的收藏（i=0）仍在
    expect(all.any((g) => g.favorite && g.time.minute == 0), isTrue);
  });

  test('收藏状态序列化往返一致，saveAll 可切换', () async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await GameArchive.add(prefs, _game(1));
    await GameArchive.add(prefs, _game(2, favorite: true));
    var all = await GameArchive.loadAll(prefs);
    expect(all.length, 2);
    expect(all[0].favorite, isTrue);
    expect(all[1].favorite, isFalse);

    // 切换收藏并整体保存
    await GameArchive.saveAll(prefs, [
      all[0].withFavorite(false),
      all[1].withFavorite(false),
    ]);
    all = await GameArchive.loadAll(prefs);
    expect(all.every((g) => !g.favorite), isTrue);
  });

  test('单条损坏记录被跳过，其余棋谱保留', () async {
    SharedPreferences.setMockInitialValues({
      'game_archive':
          '[{"time":1,"history":[]},"oops",{"time":2,"history":[]}]',
    });
    final prefs = await SharedPreferences.getInstance();
    final all = await GameArchive.loadAll(prefs);
    expect(all.length, 2);
  });
}
