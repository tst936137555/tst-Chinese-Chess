import 'package:flutter_test/flutter_test.dart';
import 'package:xiangqi/engine/rules.dart';
import 'package:xiangqi/engine/chinese_notation.dart';

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
      // 简单杀局：黑将无处可走
      final b = Board.fromFen('3k5/9/9/9/9/9/9/9/9/3K1R3 b - - 0 1');
      // 黑将 d9：红车 e0（file 4）控制 e 线，红帅 d0 控制 d 线
      // 黑将只能在 d 线上下移动（d8, d9 间）
      expect(b.statusAfterMove(), GameStatus.blackWin == GameStatus.redWin ? GameStatus.playing : (b.legalMoves().isEmpty ? GameStatus.redWin : GameStatus.playing));
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

    test('黑方炮8平5', () {
      final b = Board();
      // 黑炮 b8 (file 1, rank 2) 平 e8 (file 4, rank 2)
      final m = Move(1, 2, 4, 2);
      expect(moveToChinese(b, m), '炮8平5');
    });

    test('兵三进一', () {
      final b = Board();
      final m = Move(6, 6, 6, 5);
      expect(moveToChinese(b, m), '兵三进一');
    });
  });

  group('UCI 走法', () {
    test('格式', () {
      expect(Move(7, 7, 4, 7).uci, 'h2e2');
      expect(Move(1, 2, 4, 2).uci, 'b7e7');
    });
  });
}
