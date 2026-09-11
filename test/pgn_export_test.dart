// PGN 导出测试：标签、结果映射、走法文本、折行与转义
import 'package:flutter_test/flutter_test.dart';
import 'package:tst_xiangqi/game/game_archive.dart';
import 'package:tst_xiangqi/game/pgn_export.dart';

ArchivedGame _game({
  bool userRed = true,
  String result = 'redWin',
  String levelName = '入门',
  List<Map<String, dynamic>> history = const [],
}) =>
    ArchivedGame(
      time: DateTime(2026, 9, 11, 10, 30),
      userRed: userRed,
      levelName: levelName,
      result: result,
      history: history,
    );

Map<String, dynamic> _mv(String uci, String notation) =>
    {'uci': uci, 'captured': null, 'notation': notation, 'fen': 'ignored'};

void main() {
  group('PGN 导出', () {
    test('头部标签与走法文本（玩家执红）', () {
      final pgn = archivedGameToPgn(_game(history: [
        _mv('h2e2', '炮二平五'),
        _mv('h9g7', '马8进7'),
      ]));
      final lines = pgn.split('\n');
      expect(lines.take(8), [
        '[Event "中国象棋对局"]',
        '[Site "本地"]',
        '[Date "2026.09.11"]',
        '[Red "玩家"]',
        '[Black "皮卡鱼(入门)"]',
        '[Result "1-0"]',
        '[XQUserRed "yes"]',
        '[XQLevel "入门"]',
      ]);
      // 头部与走法区以空行分隔
      expect(lines[8], isEmpty);
      final moves = lines.sublist(9).join('\n');
      expect(moves, startsWith('1. 炮二平五 马8进7'));
      expect(moves, endsWith(' 1-0'));
    });

    test('玩家执黑：红黑名称互换，黑胜映射 0-1', () {
      final pgn = archivedGameToPgn(_game(
          userRed: false,
          result: 'blackWin',
          history: [_mv('h2e2', '炮二平五')]));
      expect(pgn, contains('[Red "皮卡鱼(入门)"]'));
      expect(pgn, contains('[Black "玩家"]'));
      expect(pgn, contains('[Result "0-1"]'));
      expect(pgn, contains('[XQUserRed "no"]'));
    });

    test('平局映射 1/2-1/2', () {
      final pgn = archivedGameToPgn(_game(result: 'draw'));
      expect(pgn, contains('[Result "1/2-1/2"]'));
      // 空走法列表：走法区仅剩结果令牌
      final movetext = pgn.split('\n\n')[1];
      expect(movetext, '1/2-1/2');
    });

    test('奇数步收尾：最后一节只有红着，结果令牌紧随其后', () {
      final pgn = archivedGameToPgn(_game(
          result: 'draw',
          history: [
            _mv('h2e2', '炮二平五'),
            _mv('h9g7', '马8进7'),
            _mv('h0g2', '马二进三'),
          ]));
      final moves = pgn.split('\n\n')[1];
      expect(moves, contains('2. 马二进三 1/2-1/2'));
    });

    test('记谱缺失时回退 UCI 坐标，全空回退 ?，仍可导出', () {
      final pgn = archivedGameToPgn(_game(history: [
        {'uci': 'h2e2', 'captured': null, 'fen': 'x'},
        {'captured': null, 'notation': '', 'fen': 'x'},
      ]));
      expect(pgn, contains('1. h2e2 ?'));
    });

    test('长局按 PGN 规范折行：所有行 ≤ 80 列', () {
      final history = List.generate(
          81,
          (i) => _mv(
              'a0a1',
              // 中文记谱 + 步号，构造足够长的行以触发折行
              '炮${i % 9 + 1}平${(i + 3) % 9 + 1}'));
      final pgn = archivedGameToPgn(_game(history: history));
      for (final line in pgn.split('\n')) {
        expect(line.length, lessThanOrEqualTo(80), reason: line);
      }
      expect(pgn.split('\n\n')[1], contains('41.'));
    });

    test('标签值转义：双引号与反斜杠', () {
      final pgn = archivedGameToPgn(_game(levelName: 'a"b\\c'));
      expect(pgn, contains(r'[XQLevel "a\"b\\c"]'));
    });
  });
}
