import 'package:flutter_test/flutter_test.dart';
import 'package:tst_xiangqi/engine/rules.dart';
import 'package:tst_xiangqi/engine/chinese_notation.dart';

void main() {
  group('FEN 解析', () {
    test('初始局面', () {
      final b = Board();
      expect(b.pieceAt(4, 9)?.fenChar, 'K');
      expect(b.pieceAt(4, 0)?.fenChar, 'k');
      expect(b.pieceAt(0, 9)?.fenChar, 'R');
      expect(b.pieceAt(8, 0)?.fenChar, 'r');
      expect(b.redToMove, true);
    });

    test('FEN 往返一致', () {
      final b = Board();
      expect(b.fen, startsWith('rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w'));
      final b2 = Board.fromFen(b.fen);
      expect(b2.fen, b.fen);
    });

    test('非法 FEN 抛异常', () {
      expect(() => Board.fromFen('bad fen'), throwsArgumentError);
    });
  });

  group('走法生成 perft', () {
    int perft(Board board, int depth) {
      if (depth == 0) return 1;
      int nodes = 0;
      for (final m in board.legalMoves()) {
        final captured = board.pieceAt(m.toFile, m.toRank);
        board.makeMove(m);
        nodes += perft(board, depth - 1);
        board.undoMove(m, captured);
      }
      return nodes;
    }

    test('初始局面 perft(1) = 44', () {
      expect(perft(Board(), 1), 44);
    });

    test('初始局面 perft(2) = 1920', () {
      expect(perft(Board(), 2), 1920);
    });

    test('初始局面 perft(3) = 79666', () {
      expect(perft(Board(), 3), 79666);
    });

    test('初始局面 perft(4) = 3290240', () {
      expect(perft(Board(), 4), 3290240);
    }, timeout: const Timeout(Duration(minutes: 5)));
  });

  group('特殊规则', () {
    test('将帅照面为非法', () {
      // 红帅 e0 黑将 e9 中间无子，红车移开前不能照面
      final b = Board.fromFen(
          '4k4/9/9/9/9/9/9/9/9/3K1R3 w - - 0 1');
      // 红帅 d0 -> e0 会照面
      expect(b.isLegal(Move(3, 9, 4, 9)), isFalse);
    });

    test('蹩马腿', () {
      // 马 b0，马腿 c0 有子则不能跳到 d1... 用初始局面验证马二进三
      final b = Board.fromFen('rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1');
      // 红马 b0（file 1, rank 9）马二进三 -> (2, 7)
      expect(b.isLegal(Move(1, 9, 2, 7)), isTrue);
      // 马二退一不可（出界方向不对），改测车不可斜走
      expect(b.isLegal(Move(0, 9, 1, 8)), isFalse);
    });

    test('相不过河', () {
      final b = Board.fromFen('rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1');
      // 红相 c0 不可到 河对岸
      // 相 (2,9) 只能到 (0,7) 和 (4,7)
      final moves = b.legalMoves().where((m) => m.fromFile == 2 && m.fromRank == 9).toList();
      expect(moves.length, 2);
    });

    test('送将非法', () {
      // 黑车吃红帅受保护
      final b = Board.fromFen('4k4/9/9/9/9/9/9/9/4r4/3K5 w - - 0 1');
      // 红帅 d0 -> e0 会送将（黑车 e1 控制 e 线）？e1->e0 后红帅在 e0 被黑将照面也非法
      expect(b.isLegal(Move(3, 9, 4, 9)), isFalse);
    });

    test('将死判定', () {
      // 双车闷杀：黑将 (4,0)，红车 (0,0) 沿底线将军、红车 (8,1) 封锁第 1 横线
      final b = Board.fromFen('R3k4/8R/9/9/9/9/9/9/9/9 b - - 0 1');
      // 黑方被将死，无子可动 → 红方胜
      expect(b.statusAfterMove(), GameStatus.redWin);
    });
  });

  group('将军检测', () {
    test('黑卒正面攻击红帅', () {
      // 黑卒 (4,6) 向下走可吃红帅 (4,7)
      final b = Board.fromFen('3k5/9/9/9/9/9/4p4/4K4/9/9 w - - 0 1');
      expect(b.inCheck, isTrue);
    });

    test('红兵正面攻击黑将', () {
      // 红兵 (4,3) 向上走可吃黑将 (4,2)
      final b = Board.fromFen('9/9/4k4/4P4/9/9/9/3K5/9/9 b - - 0 1');
      expect(b.inCheck, isTrue);
    });

    test('将帅身后的敌卒不构成将军', () {
      // 黑卒 (4,8) 在红帅 (4,7) 下方（已过），无法回头攻击
      final b = Board.fromFen('3k5/9/9/9/9/9/9/4K4/4p4/9 w - - 0 1');
      expect(b.inCheck, isFalse);
    });

    test('黑将身后的敌兵不构成将军', () {
      // 红兵 (4,1) 在黑将 (4,2) 上方（已过），无法回头攻击
      final b = Board.fromFen('9/4P4/4k4/9/9/9/9/3K5/9/9 b - - 0 1');
      expect(b.inCheck, isFalse);
    });

    test('过河卒横向攻击红帅', () {
      // 黑卒 (3,7) 过河后横吃红帅 (4,7)
      final b = Board.fromFen('3k5/9/9/9/9/9/9/3pK4/9/9 w - - 0 1');
      expect(b.inCheck, isTrue);
    });

    test('送吃红帅给黑卒的走法非法', () {
      // 黑卒 (3,8) 正吃 (3,9)、横吃 (2,8)/(4,8)；红帅 (4,9)
      final b = Board.fromFen('k8/9/9/9/9/9/9/9/3p5/4K4 w - - 0 1');
      expect(b.isLegal(Move(4, 9, 5, 9)), isTrue); // 避开卒的攻击
      expect(b.isLegal(Move(4, 9, 4, 8)), isFalse); // 送入卒横吃
      expect(b.isLegal(Move(4, 9, 3, 9)), isFalse); // 送入卒正面
    });
  });

  group('中文记谱', () {
    test('炮二平五', () {
      final b = Board();
      // 红炮 h2 (file 7, rank 7) 平到 e2 (file 4, rank 7)
      final m = Move(7, 7, 4, 7);
      expect(moveToChinese(b, m), '炮二平五');
    });

    test('马二进三', () {
      final b = Board();
      final m = Move(7, 9, 6, 7);
      expect(moveToChinese(b, m), '马二进三');
    });

    test('黑方炮2平5', () {
      final b = Board();
      // 黑炮 b8 (file 1, rank 2) 平 e8 (file 4, rank 2)：黑2 路 -> 黑5 路
      final m = Move(1, 2, 4, 2);
      expect(moveToChinese(b, m), '炮2平5');
    });

    test('黑方马8进7（标准对照 h9g7）', () {
      final b = Board();
      // 黑马 h9 (file 7, rank 0) 进到 g7 (file 6, rank 2)：黑8 路 -> 黑7 路
      final m = Move(7, 0, 6, 2);
      expect(moveToChinese(b, m), '马8进7');
    });

    test('兵三进一', () {
      final b = Board();
      final m = Move(6, 6, 6, 5);
      expect(moveToChinese(b, m), '兵三进一');
    });

    test('同纵线进退用步数：红汉字、黑阿拉伯数字', () {
      final b = Board();
      // 红炮 h2 直进两步：炮二进二
      expect(moveToChinese(b, Move(7, 7, 7, 5)), '炮二进二');
      // 红帅直进一步：帅五进一
      expect(moveToChinese(b, Move(4, 9, 4, 8)), '帅五进一');
      // 黑炮 b7 直进两步：炮2进2（黑方步数用阿拉伯数字）
      expect(moveToChinese(b, Move(1, 2, 1, 4)), '炮2进2');
      // 黑车 a9 (黑1 路) 直进一步：车1进1
      expect(moveToChinese(b, Move(0, 0, 0, 1)), '车1进1');
    });

    test('同纵线双车用前/后缀并省略起始纵线', () {
      // 红双车同在 a 线：(0,8) 更靠对方为前车，(0,9) 为后车
      final b = Board.fromFen('4k4/9/9/9/9/9/9/9/R8/R3K4 w - - 0 1');
      expect(moveToChinese(b, Move(0, 8, 3, 8)), '前车平六');
      expect(moveToChinese(b, Move(0, 9, 3, 9)), '后车平六');
    });

    test('黑方同纵线双炮用前/后缀', () {
      // 黑双炮同在 b 线：(1,2) 更靠红方为前炮，(1,1) 为后炮
      final b = Board.fromFen('4k4/1c7/1c7/9/9/9/9/9/9/9 w - - 0 1');
      expect(moveToChinese(b, Move(1, 2, 4, 2)), '前炮平5');
      expect(moveToChinese(b, Move(1, 1, 4, 1)), '后炮平5');
    });

    test('同纵线三兵用前/中/后', () {
      // 红三兵同在 c 线 (file 2)：(2,3) 前、(2,4) 中、(2,5) 后
      final b = Board.fromFen('4k4/9/9/2P6/2P6/2P6/9/9/9/4K4 w - - 0 1');
      expect(moveToChinese(b, Move(2, 3, 1, 3)), '前兵平八');
      expect(moveToChinese(b, Move(2, 4, 1, 4)), '中兵平八');
      expect(moveToChinese(b, Move(2, 5, 1, 5)), '后兵平八');
    });
  });

  group('UCI 走法', () {
    test('格式', () {
      expect(Move(7, 7, 4, 7).uci, 'h2e2');
      expect(Move(1, 2, 4, 2).uci, 'b7e7');
    });

    test('解析与往返', () {
      expect(Move.fromUci('h2e2'), const Move(7, 7, 4, 7));
      // 边界：纵线 a/i、横线数字 0/9（数字 d 对应 rank 9-d）
      expect(Move.fromUci('a0i9'), const Move(0, 9, 8, 0));
      expect(Move.fromUci('i0a9'), const Move(8, 9, 0, 0));
    });

    test('非法格式被拒绝（不再静默映射到棋盘内错误位置）', () {
      // 回归用例：旧实现 j-'a'=9，索引 8*9+9=81<90，静默读到错误棋子
      expect(() => Move.fromUci('j1i1'), throwsFormatException);
      expect(() => Move.fromUci('0000'), throwsFormatException);
      expect(() => Move.fromUci('x1y1'), throwsFormatException);
      expect(() => Move.fromUci('a9a10'), throwsFormatException);
      expect(() => Move.fromUci('a'), throwsFormatException);
      expect(() => Move.fromUci(''), throwsFormatException);
      expect(() => Move.fromUci('A0A1'), throwsFormatException); // 大写
    });
  });

  group('重复局面判和与长将判负', () {
    test('未重复三次返回 null（对局继续）', () {
      expect(
        repetitionStatus(['A', 'B', 'C', 'B'], [false, false, false]),
        isNull,
      );
    });

    test('含初始局面的三次重复判和（回归：初始局面计入序列）', () {
      // A(初始) -> B -> A -> B -> A：初始局面出现 3 次
      final keys = ['A', 'B', 'A', 'B', 'A'];
      expect(
        repetitionStatus(keys, [false, false, false, false]),
        GameStatus.draw,
      );
    });

    test('红方长将判负', () {
      // 周期内红方每步将军（k=0,2,4），黑方不将军
      final keys = ['A', 'B', 'C', 'B', 'C', 'B'];
      final checks = [true, false, true, false, true];
      expect(repetitionStatus(keys, checks), GameStatus.blackWin);
    });

    test('黑方长将判负', () {
      final keys = ['A', 'B', 'C', 'B', 'C', 'B'];
      final checks = [false, true, false, true, false];
      expect(repetitionStatus(keys, checks), GameStatus.redWin);
    });

    test('双方均长将判和', () {
      final keys = ['A', 'B', 'C', 'B', 'C', 'B'];
      final checks = [true, true, true, true, true];
      expect(repetitionStatus(keys, checks), GameStatus.draw);
    });

    test('重复但无长将判和', () {
      final keys = ['A', 'B', 'C', 'B', 'C', 'B'];
      final checks = [false, false, false, false, false];
      expect(repetitionStatus(keys, checks), GameStatus.draw);
    });
  });

  group('重复预警辅助函数', () {
    test('repetitionCount 统计当前局面出现次数', () {
      expect(repetitionCount([]), 0);
      expect(repetitionCount(['A', 'B', 'C']), 1);
      expect(repetitionCount(['A', 'B', 'A']), 2);
      expect(repetitionCount(['A', 'B', 'A', 'B', 'A']), 3);
    });

    test('longCheckSide 识别周期内单方长将', () {
      // k=3 黑不将军、k=4 红将军 → 红方长将
      expect(longCheckSide([true, false, true, false, true], 3, 5), 'r');
      // k=3 黑将军、k=4 红不将军 → 黑方长将
      expect(longCheckSide([false, true, false, true, false], 3, 5), 'b');
      // 双方均将军 / 均非将军 → null
      expect(longCheckSide([true, true, true, true, true], 3, 5), isNull);
      expect(longCheckSide([false, false, false, false, false], 3, 5), isNull);
    });
  });

  group('半回合计数与自然限着', () {
    test('非吃子步推进计数，吃子步清零', () {
      final b = Board();
      expect(b.halfmoveClock, 0);
      b.makeMove(Move(7, 7, 4, 7)); // 炮二平五，无吃子
      expect(b.halfmoveClock, 1);
      // 黑卒 (4,5) 吃红兵 (4,4)
      final c = Board.fromFen('4k4/9/9/9/4P4/4p4/9/9/9/4K4 b - - 12 6');
      expect(c.halfmoveClock, 12);
      c.makeMove(Move(4, 5, 4, 4));
      expect(c.halfmoveClock, 0);
    });

    test('FEN 着法计数字段往返一致', () {
      final b = Board.fromFen('4k4/9/9/9/9/9/9/9/9/4K4 w - - 23 45');
      expect(b.halfmoveClock, 23);
      expect(b.fen, '4k4/9/9/9/9/9/9/9/9/4K4 w - - 23 45');
      final b2 = Board.fromFen(b.fen);
      expect(b2.halfmoveClock, 23);
    });

    test('合法走法探测不污染半回合计数', () {
      final b = Board();
      b.makeMove(Move(7, 7, 4, 7)); // clock = 1
      b.legalMoves(); // 内部多次 make/undo 探测
      expect(b.halfmoveClock, 1);
    });

    test('60 回合（120 半回合）无吃子判定自然限着', () {
      final b = Board();
      const up = Move(0, 9, 0, 8), down = Move(0, 8, 0, 9);
      for (var i = 0; i < 119; i++) {
        b.makeMove(i.isEven ? up : down);
      }
      expect(b.halfmoveClock, 119);
      expect(b.isNaturalDraw, isFalse);
      b.makeMove(down); // 第 120 半回合：车回到原位，仍无吃子
      expect(b.halfmoveClock, Board.naturalDrawHalfmoves);
      expect(b.isNaturalDraw, isTrue);
      // 回合计数：120 半回合 = 60 回合，回合数 61
      expect(b.fen, endsWith('120 61'));
    });
  });

  group('Zobrist 增量哈希', () {
    test('走子后哈希与全量重算一致', () {
      final b = Board();
      b.makeMove(Move(7, 7, 4, 7));
      expect(b.positionHash, Board.fromFen(b.fen).positionHash);
      b.makeMove(Move(7, 0, 6, 2));
      expect(b.positionHash, Board.fromFen(b.fen).positionHash);
    });

    test('undoMove 精确还原哈希（含吃子走法）', () {
      final b = Board();
      final h0 = b.positionHash;
      final m = Move(7, 7, 4, 7);
      b.makeMove(m);
      expect(b.positionHash, isNot(h0));
      b.undoMove(m, null);
      expect(b.positionHash, h0);

      final c = Board.fromFen('4k4/9/9/9/4P4/4p4/9/9/9/4K4 b - - 0 1');
      final captured = c.pieceAt(4, 4);
      final hc = c.positionHash;
      c.makeMove(Move(4, 5, 4, 4));
      c.undoMove(Move(4, 5, 4, 4), captured);
      expect(c.positionHash, hc);
    });

    test('不同局面 / 不同行棋方哈希不同', () {
      final red = Board();
      final parts = Board.startFen.split(' ');
      parts[1] = 'b';
      expect(Board.fromFen(parts.join(' ')).positionHash,
          isNot(red.positionHash));
      final moved = Board()..makeMove(Move(7, 7, 4, 7));
      expect(moved.positionHash, isNot(red.positionHash));
    });

    test('cloneFrom 保留哈希与着法计数', () {
      final b = Board()..makeMove(Move(7, 7, 4, 7));
      final c = Board.cloneFrom(b);
      expect(c.positionHash, b.positionHash);
      expect(c.halfmoveClock, b.halfmoveClock);
      expect(c.fen, b.fen);
    });
  });
}
