/// PGN 导出：把归档棋谱序列化为标准 PGN 文本（象棋扩展）。
///
/// 设计要点：
/// - 走法文本复用归档时已生成的中文纵线记谱（象棋 PGN 的事实惯例，
///   参考 XQWizard/东萍的导出风格）；单条记谱缺失时回退 UCI 坐标。
/// - 标签使用 PGN 标准七标签中适用的部分 + `XQ` 前缀自定义标签
///   （执子方/难度）。PGN 规范要求解析器忽略未知标签，
///   因此自定义标签对其他工具无副作用，未来加字段也不破坏旧文件。
/// - 对局恒从标准初始局面开始，按象棋 PGN 惯例省略 FEN 标签。
/// - 走法区按 PGN 规范折行（≤80 列），行尾以 Result 令牌收束。
library;

import 'game_archive.dart';

/// 把一局归档棋谱导出为 PGN 文本
String archivedGameToPgn(ArchivedGame game) {
  final t = game.time;
  String two(int v) => v.toString().padLeft(2, '0');
  final date = '${t.year}.${two(t.month)}.${two(t.day)}';
  final aiName = game.levelName.isEmpty ? '皮卡鱼' : '皮卡鱼(${game.levelName})';

  final headers = [
    '[Event "中国象棋对局"]',
    '[Site "本地"]',
    '[Date "$date"]',
    '[Red "${_escapeTag(game.userRed ? '玩家' : aiName)}"]',
    '[Black "${_escapeTag(game.userRed ? aiName : '玩家')}"]',
    '[Result "${_pgnResult(game.result)}"]',
    '[XQUserRed "${game.userRed ? 'yes' : 'no'}"]',
    '[XQLevel "${_escapeTag(game.levelName)}"]',
  ];

  // 走法令牌：整回合计数 "N. 红着 黑着"，行尾追加结果令牌
  final tokens = <String>[];
  final history = game.history;
  for (var i = 0; i < history.length; i += 2) {
    tokens.add('${i ~/ 2 + 1}.');
    tokens.add(_moveText(history[i]));
    if (i + 1 < history.length) tokens.add(_moveText(history[i + 1]));
  }
  tokens.add(_pgnResult(game.result));
  final moveLines = _wrapTokens(tokens);

  return [...headers, '', ...moveLines].join('\n');
}

/// 对局结果 → PGN Result 标准值
String _pgnResult(String result) => switch (result) {
      'redWin' => '1-0',
      'blackWin' => '0-1',
      _ => '1/2-1/2',
    };

/// 走法文本：优先中文记谱，其次 UCI 坐标，兜底占位符
String _moveText(Map<String, dynamic> e) {
  final notation = (e['notation'] as String?)?.trim();
  if (notation != null && notation.isNotEmpty) return notation;
  final uci = (e['uci'] as String?)?.trim();
  if (uci != null && uci.isNotEmpty) return uci;
  return '?';
}

/// 标签值转义：PGN 规范要求值内的双引号与反斜杠转义
String _escapeTag(String v) =>
    v.replaceAll('\\', '\\\\').replaceAll('"', '\\"');

/// 把令牌流折行：单行 ≤80 列（PGN 规范上限），空格连接
List<String> _wrapTokens(List<String> tokens) {
  const maxLen = 80;
  final lines = <String>[];
  var cur = '';
  for (final token in tokens) {
    if (cur.isEmpty) {
      cur = token;
    } else if (cur.length + 1 + token.length <= maxLen) {
      cur = '$cur $token';
    } else {
      lines.add(cur);
      cur = token;
    }
  }
  if (cur.isNotEmpty) lines.add(cur);
  return lines;
}
