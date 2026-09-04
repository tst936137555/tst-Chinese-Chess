/// 中国象棋主入口：单机对弈（内置皮卡鱼引擎，无网络功能）。
library;

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'engine/pikafish.dart';
import 'engine/rules.dart';
import 'game/game_controller.dart';
import 'game/stats_service.dart';
import 'ui/board_view.dart';
import 'ui/dialogs.dart';
import 'ui/review_screen.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  runApp(XiangqiApp(prefs: prefs));
}

class XiangqiApp extends StatelessWidget {
  const XiangqiApp({super.key, required this.prefs});

  final SharedPreferences prefs;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '中国象棋',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF8B2F1F)),
        useMaterial3: true,
      ),
      home: const HomePage(),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  GameController? _controller;
  final _stats = StatsService();

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    await _stats.load();
    final prefs = await SharedPreferences.getInstance();
    final controller = GameController(
      engine: PikafishEngine.instance,
      stats: _stats,
      prefs: prefs,
    );
    if (!mounted) return;
    setState(() => _controller = controller);
    // 尝试恢复上次对局
    final restored = await controller.restoreGame();
    if (!restored && mounted && controller.history.isEmpty) {
      controller.newGame();
    }
  }

  @override
  Widget build(BuildContext context) {
    final controller = _controller;
    if (controller == null) {
      return const Scaffold(
        body: Center(child: CircularProgressIndicator()),
      );
    }
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) => GameScreen(controller: controller),
    );
  }
}

/// 主对局界面
class GameScreen extends StatefulWidget {
  const GameScreen({super.key, required this.controller});

  final GameController controller;

  @override
  State<GameScreen> createState() => _GameScreenState();
}

class _GameScreenState extends State<GameScreen>
    with SingleTickerProviderStateMixin {
  (int, int)? _selected;
  List<Move> _legalTargets = [];
  GameController get c => widget.controller;

  /// 走子动画
  AnimationController? _animController;
  Move? _animMove;
  Piece? _animCaptured;
  /// 已对最后一步播放过动画的标记（避免恢复对局时重播）
  int _animatedHistoryLength = -1;

  @override
  void initState() {
    super.initState();
    _animatedHistoryLength = c.history.length;
  }

  @override
  void dispose() {
    _animController?.dispose();
    super.dispose();
  }

  /// 播放某一步走子动画
  void _animateMove(Move m, Piece? captured) {
    _animMove = m;
    _animCaptured = captured;
    _animController?.dispose();
    _animController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 260),
    )..addStatusListener((status) {
        if (status == AnimationStatus.completed) {
          setState(() => _animMove = null);
        }
      })
      ..forward();
    setState(() {});
  }

  void _onTapSquare(int file, int rank) {
    if (c.thinking || !c.isUserTurn) return;
    final piece = c.board.pieceAt(file, rank);

    if (_selected != null) {
      // 尝试走子
      final move = _legalTargets.where((m) =>
          m.toFile == file && m.toRank == rank).firstOrNull;
      if (move != null) {
        final captured = c.board.pieceAt(move.toFile, move.toRank);
        final moved = c.tryMove(move);
        if (moved) {
          setState(() {
            _selected = null;
            _legalTargets = [];
          });
          _animateMove(move, captured);
        }
        return;
      }
    }

    if (piece != null && piece.isRed == c.userPlaysRed) {
      // 选中己方棋子
      setState(() {
        _selected = (file, rank);
        _legalTargets = c.board
            .legalMoves()
            .where((m) => m.fromFile == file && m.fromRank == rank)
            .toList();
      });
    } else {
      setState(() {
        _selected = null;
        _legalTargets = [];
      });
    }
  }

  /// 确认认输
  Future<void> _confirmResign() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('认输'),
        content: const Text('确定要认输吗？本局将判负。'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('继续下'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('认输'),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      setState(() {
        _selected = null;
        _legalTargets = [];
      });
      c.resign();
    }
  }

  void _showGameEnd() {
    final s = c.status;
    if (s == GameStatus.playing) return;
    final (title, msg) = switch (s) {
      GameStatus.redWin => (
          c.userPlaysRed ? '胜利！' : '惜败',
          c.userPlaysRed ? '红方获胜，恭喜你赢了！' : '红方获胜，再接再厉。'
        ),
      GameStatus.blackWin => (
          c.userPlaysRed ? '惜败' : '胜利！',
          c.userPlaysRed ? '黑方获胜，再接再厉。' : '黑方获胜，恭喜你赢了！'
        ),
      GameStatus.draw => ('和棋', '双方不变作和。'),
      _ => ('', ''),
    };
    showDialog(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: Text(title),
        content: Text(msg),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              setState(() {
                _selected = null;
                _legalTargets = [];
              });
            },
            child: const Text('查看棋盘'),
          ),
          TextButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              _showReviewChoices(context, c);
            },
            child: const Text('复盘'),
          ),
          FilledButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              setState(() {
                _selected = null;
                _legalTargets = [];
              });
              c.newGame();
            },
            child: const Text('再来一局'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    // 对局结束时弹出对话框
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (c.status != GameStatus.playing && !c.thinking && mounted) {
        _showGameEnd();
      }
    });

    // AI 落子后自动播放动画
    if (c.history.isNotEmpty &&
        c.history.length != _animatedHistoryLength &&
        _animMove == null) {
      final last = c.history.last;
      if (last.move != _animMove) {
        _animatedHistoryLength = c.history.length;
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) {
            _animateMove(last.move, last.capturedPieceObj);
            _animatedHistoryLength = c.history.length;
          }
        });
      }
    }

    final targetSquares = _legalTargets.map((m) => (m.toFile, m.toRank)).toList();
    final anim = _animController;

    return Scaffold(
      appBar: AppBar(
        title: const Text('中国象棋'),
        centerTitle: true,
        actions: [
          IconButton(
            icon: const Icon(Icons.replay_circle_filled),
            tooltip: '复盘',
            onPressed: () => _showReviewChoices(context, c),
          ),
          IconButton(
            icon: const Icon(Icons.bar_chart),
            tooltip: '战绩',
            onPressed: () => showStatsDialog(context, c),
          ),
          IconButton(
            icon: const Icon(Icons.tune),
            tooltip: '难度',
            onPressed: () => showLevelDialog(context, c),
          ),
          PopupMenuButton<String>(
            onSelected: (v) async {
              switch (v) {
                case 'new_red':
                  c.newGame(userRed: true);
                  break;
                case 'new_black':
                  c.newGame(userRed: false);
                  break;
                case 'about':
                  showAboutDialog(
                    context: context,
                    applicationName: '中国象棋',
                    applicationLegalese: '本地单机对弈，内置皮卡鱼引擎，无联网功能。',
                  );
                  break;
              }
              setState(() {
                _selected = null;
                _legalTargets = [];
              });
            },
            itemBuilder: (ctx) => const [
              PopupMenuItem(value: 'new_red', child: Text('执红开局')),
              PopupMenuItem(value: 'new_black', child: Text('执黑开局')),
              PopupMenuItem(value: 'about', child: Text('关于')),
            ],
          ),
        ],
      ),
      body: SafeArea(
        child: Column(
          children: [
            // 状态栏：难度 + 回合 + 思考指示
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              child: Row(
                children: [
                  Chip(
                    avatar: const Icon(Icons.computer, size: 18),
                    label: Text('AI：${c.level.name}'),
                    visualDensity: VisualDensity.compact,
                  ),
                  const SizedBox(width: 8),
                  if (c.thinking)
                    const Expanded(
                      child: Row(
                        children: [
                          SizedBox(
                            width: 14,
                            height: 14,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                          SizedBox(width: 8),
                          Text('皮卡鱼思考中…'),
                        ],
                      ),
                    )
                  else
                    Expanded(
                      child: Text(
                        c.status == GameStatus.playing
                            ? (c.isUserTurn ? '轮到你走棋' : '轮到对方走棋')
                            : '对局结束',
                        style: const TextStyle(fontWeight: FontWeight.w500),
                      ),
                    ),
                  Text(
                    '第 ${c.history.length ~/ 2 + 1} 回合',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ),
            ),
            // 棋盘
            Expanded(
              child: Center(
                child: Padding(
                  padding: const EdgeInsets.all(8),
                  child: anim != null && _animMove != null
                      ? AnimatedBuilder(
                          animation: anim,
                          builder: (context, _) => BoardView(
                            board: c.board,
                            onTapSquare: _onTapSquare,
                            flipBoard: c.flipBoard,
                            selected: _selected,
                            legalTargets: targetSquares,
                            lastMove: c.lastMove,
                            checkPos: c.checkPos,
                            animatingMove: _animMove,
                            animationProgress: anim.value,
                            capturedPiece: _animCaptured,
                          ),
                        )
                      : BoardView(
                          board: c.board,
                          onTapSquare: _onTapSquare,
                          flipBoard: c.flipBoard,
                          selected: _selected,
                          legalTargets: targetSquares,
                          lastMove: c.lastMove,
                          checkPos: c.checkPos,
                        ),
                ),
              ),
            ),
            // 最近着法
            SizedBox(
              height: 34,
              child: ListView.builder(
                scrollDirection: Axis.horizontal,
                padding: const EdgeInsets.symmetric(horizontal: 12),
                itemCount: c.history.length,
                itemBuilder: (ctx, i) {
                  final e = c.history[i];
                  final isRedMove = i.isEven;
                  return Padding(
                    padding: const EdgeInsets.only(right: 12),
                    child: Text(
                      '${isRedMove ? '${i ~/ 2 + 1}. ' : ''}${e.notation}',
                      style: TextStyle(
                        color: isRedMove
                            ? const Color(0xFFB03020)
                            : const Color(0xFF222222),
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  );
                },
              ),
            ),
            // 底部操作
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 4, 12, 8),
              child: Row(
                children: [
                  Expanded(
                    child: FilledButton.tonalIcon(
                      onPressed: (c.history.isEmpty || c.thinking)
                          ? null
                          : () {
                              c.undo();
                              setState(() {
                                _selected = null;
                                _legalTargets = [];
                              });
                            },
                      icon: const Icon(Icons.undo, size: 18),
                      label: const Text('悔棋'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: FilledButton.tonalIcon(
                      onPressed: (c.history.isEmpty || c.thinking ||
                              c.status != GameStatus.playing)
                          ? null
                          : _confirmResign,
                      icon: const Icon(Icons.flag, size: 18),
                      label: const Text('认输'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: FilledButton.icon(
                      onPressed: c.thinking
                          ? null
                          : () {
                              c.newGame();
                              setState(() {
                                _selected = null;
                                _legalTargets = [];
                              });
                            },
                      icon: const Icon(Icons.refresh, size: 18),
                      label: const Text('新对局'),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// 复盘入口：复盘上局 / 复盘棋谱
  void _showReviewChoices(BuildContext context, GameController c) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('复盘'),
        content: const Text('选择复盘方式'),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              if (c.history.isEmpty) return;
              openReviewLastGame(context, game: c);
            },
            child: const Text('复盘上局'),
          ),
          FilledButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              openReviewArchive(context);
            },
            child: const Text('复盘棋谱'),
          ),
        ],
      ),
    );
  }
}
