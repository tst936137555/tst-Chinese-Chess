// BoardView 渲染基线测试（Golden Test）：锁定棋盘绘制回归
// （棋子布局、选中/可走点、将军圈、提示箭头与角标、质量角标、棋盘翻转）。
//
// 运行前提：
// - 字体资产已就位（tool/fetch_engine.ps1 -Target core 下载霞鹜文楷）；
//   字体缺失时本组测试自动跳过（fresh clone 未 fetch 时不误报）。
// - Golden 基线跨平台渲染存在细微差异，基线统一在 Windows 上生成，
//   非 Windows 环境自动跳过（CI ubuntu 测试 job 不受影响；
//   windows-test job 以同平台运行基线比对）。
//
// 更新基线：flutter test --update-goldens test/golden_test.dart
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:tst_xiangqi/engine/rules.dart';
import 'package:tst_xiangqi/game/review_controller.dart';
import 'package:tst_xiangqi/ui/board_view.dart';

Future<void> main() async {
  TestWidgetsFlutterBinding.ensureInitialized();

  // 加载棋子字体（fonts: 声明的字体不进 asset bundle，须从仓库文件读入）
  var fontLoaded = false;
  try {
    final fontFile = File('assets/fonts/LXGWWenKai-Medium.ttf');
    if (fontFile.existsSync()) {
      final bytes = fontFile.readAsBytesSync();
      final loader = FontLoader('XqKai')
        ..addFont(Future.value(
            ByteData.view(bytes.buffer, bytes.offsetInBytes, bytes.length)));
      await loader.load();
      fontLoaded = true;
    }
  } catch (_) {
    fontLoaded = false;
  }

  final skipReason = !fontLoaded
      ? '字体资产缺失（先运行 tool/fetch_engine.ps1 -Target core）'
      : !Platform.isWindows
          ? 'Golden 基线仅在 Windows 平台比对'
          : null;

  /// 以固定尺寸（cell=45px）渲染 BoardView 并落图
  Future<void> pumpBoard(
    WidgetTester tester, {
    required Board board,
    bool flip = false,
    (int, int)? selected,
    List<(int, int)> targets = const [],
    Move? lastMove,
    (int, int)? checkPos,
    List<Move> hints = const [],
    Move? suggested,
    MoveQuality? quality,
  }) async {
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 450,
            height: 495,
            child: BoardView(
              board: board,
              onTapSquare: (_, _) {},
              flipBoard: flip,
              selected: selected,
              legalTargets: targets,
              lastMove: lastMove,
              checkPos: checkPos,
              suggestedMoves: hints,
              suggestedMove: suggested,
              quality: quality,
            ),
          ),
        ),
      ),
    ));
    await tester.pump();
  }

  group('BoardView Golden', () {
    testWidgets('初始局面', (tester) async {
    await pumpBoard(tester, board: Board());
    await expectLater(
        find.byType(BoardView), matchesGoldenFile('goldens/board_initial.png'));
  });

  testWidgets('选中与可走点、最后一步标记', (tester) async {
    final board = Board()..makeMove(Move.fromUci('h2e2')); // 炮二平五 + 黑马应着
    board.makeMove(Move.fromUci('h9g7'));
    // 选中红马 (7,9)，可走点取其合法走法
    final targets = board
        .legalMoves()
        .where((m) => m.fromFile == 7 && m.fromRank == 9)
        .map((m) => (m.toFile, m.toRank))
        .toList();
    await pumpBoard(
      tester,
      board: board,
      selected: (7, 9),
      targets: targets,
      lastMove: Move.fromUci('h9g7'),
    );
    await expectLater(find.byType(BoardView),
        matchesGoldenFile('goldens/board_selection.png'));
  });

  testWidgets('被将军标记', (tester) async {
    // 黑卒 d6 贴脸攻击红帅（红方行棋且被将军）
    final board = Board.fromFen('3k5/9/9/9/9/9/9/3pK4/9/9 w - - 0 1');
    await pumpBoard(
      tester,
      board: board,
      checkPos: board.findKing(true),
      lastMove: Move.fromUci('d7e7'),
    );
    await expectLater(find.byType(BoardView),
        matchesGoldenFile('goldens/board_check.png'));
  });

  testWidgets('提示双箭头与编号角标', (tester) async {
    await pumpBoard(
      tester,
      board: Board(),
      hints: [Move.fromUci('h2e2'), Move.fromUci('b2e2')],
    );
    await expectLater(find.byType(BoardView),
        matchesGoldenFile('goldens/board_hints.png'));
  });

  testWidgets('复盘：建议箭头与走法质量角标', (tester) async {
    final board = Board()..makeMove(Move.fromUci('h2e2'));
    await pumpBoard(
      tester,
      board: board,
      lastMove: Move.fromUci('h2e2'),
      suggested: Move.fromUci('b2e2'),
      quality: MoveQuality.mistake,
    );
    await expectLater(find.byType(BoardView),
        matchesGoldenFile('goldens/board_review.png'));
  });

  testWidgets('执黑翻转棋盘', (tester) async {
    final board = Board()..makeMove(Move.fromUci('h2e2'));
    board.makeMove(Move.fromUci('h9g7'));
    await pumpBoard(
      tester,
      board: board,
      flip: true,
      lastMove: Move.fromUci('h9g7'),
    );
    await expectLater(find.byType(BoardView),
        matchesGoldenFile('goldens/board_flipped.png'));
  });
  }, skip: skipReason);
}
