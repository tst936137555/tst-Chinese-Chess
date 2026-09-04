/// 中国象棋主入口：单机对弈（内置皮卡鱼引擎，无网络功能）。
///
/// 交互流程：主界面选择「继续上局 / 新开局」→ 选择难度 → 选择执红/执黑 → 进入对局。
/// 对局界面：顶栏为降难度 / 当前难度 / 升难度 / 返回；底栏为悔棋 / 提示 / 结束。
library;

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'engine/pikafish.dart';
import 'engine/rules.dart';
import 'game/game_controller.dart';
import 'ui/board_view.dart';
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
      home: HomePage(prefs: prefs),
    );
  }
}

// ---------------------------------------------------------------------------
// 主界面：入口选择（继续上局 / 新开局）
// ---------------------------------------------------------------------------

class HomePage extends StatefulWidget {
  const HomePage({super.key, required this.prefs});

  final SharedPreferences prefs;

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  /// 是否存在可继续的对局
  bool _hasSavedGame = false;

  @override
  void initState() {
    super.initState();
    _checkSaved();
  }

  void _checkSaved() {
    final raw = widget.prefs.getString('saved_game');
    final has = raw != null && raw.isNotEmpty;
    if (has != _hasSavedGame) setState(() => _hasSavedGame = has);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('中国象棋'), centerTitle: true),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 40),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Text(
                '中国象棋',
                style: TextStyle(fontSize: 34, fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 6),
              Text(
                '本地单机对弈 · 内置皮卡鱼引擎',
                style: TextStyle(
                    fontSize: 13, color: Colors.grey.withValues(alpha: 0.9)),
              ),
              const SizedBox(height: 48),
              // 继续上局
              SizedBox(
                width: double.infinity,
                height: 52,
                child: FilledButton.tonalIcon(
                  onPressed: _hasSavedGame
                      ? () async {
                          await _startGame(resume: true);
                        }
                      : null,
                  icon: const Icon(Icons.play_arrow),
                  label: Text(_hasSavedGame ? '继续上局' : '继续上局（无存档）'),
                ),
              ),
              const SizedBox(height: 16),
              // 新开局
              SizedBox(
                width: double.infinity,
                height: 52,
                child: FilledButton.icon(
                  onPressed: () async {
                    await _startGame(resume: false);
                  },
                  icon: const Icon(Icons.add),
                  label: const Text('新开局'),
                ),
              ),
              const SizedBox(height: 32),
              // 复盘棋谱入口
              TextButton.icon(
                onPressed: () => openReviewArchive(context),
                icon: const Icon(Icons.history, size: 18),
                label: const Text('复盘棋谱'),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// 进入对局流程：新开局时依次选择难度与执子方
  Future<void> _startGame({required bool resume}) async {
    if (resume) {
      // 继续上局：使用已保存的难度与执子方
      if (!mounted) return;
      Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => GamePage(
          prefs: widget.prefs,
          resumeGame: true,
        ),
      )).then((_) => _checkSaved());
      return;
    }

    // 第一步：选择难度
    final level = await showDialog<DifficultyLevel>(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => SimpleDialog(
        title: const Text('选择难度'),
        children: [
          for (final l in DifficultyLevel.all)
            SimpleDialogOption(
              onPressed: () => Navigator.of(ctx).pop(l),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 6),
                child: Row(
                  children: [
                    Icon(_levelIcon(l),
                        color: Theme.of(ctx).colorScheme.primary),
                    const SizedBox(width: 12),
                    Text(l.name,
                        style: const TextStyle(
                            fontSize: 17, fontWeight: FontWeight.w500)),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
    if (level == null || !mounted) return;

    // 第二步：选择执红 / 执黑
    final userRed = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: const Text('选择执子'),
        content: const Text('执红先行，执黑后手。'),
        actions: [
          FilledButton.tonal(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('执黑'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('执红'),
          ),
        ],
      ),
    );
    if (userRed == null || !mounted) return;

    Navigator.of(context).push(MaterialPageRoute(
      builder: (_) => GamePage(
        prefs: widget.prefs,
        initialLevel: level,
        initialUserRed: userRed,
      ),
    )).then((_) => _checkSaved());
  }
}

/// 难度对应的图标
IconData _levelIcon(DifficultyLevel l) {
  if (l == DifficultyLevel.beginner) return Icons.sentiment_satisfied;
  if (l == DifficultyLevel.easy) return Icons.sentiment_satisfied_alt;
  if (l == DifficultyLevel.medium) return Icons.sentiment_neutral;
  if (l == DifficultyLevel.hard) return Icons.sentiment_dissatisfied;
  return Icons.whatshot;
}

// ---------------------------------------------------------------------------
// 对局界面
// ---------------------------------------------------------------------------

class GamePage extends StatefulWidget {
  const GamePage({
    super.key,
    required this.prefs,
    this.initialLevel,
    this.initialUserRed,
    this.resumeGame = false,
  });

  final SharedPreferences prefs;
  /// 新开局时指定的难度
  final DifficultyLevel? initialLevel;
  /// 新开局时指定的执子方
  final bool? initialUserRed;
  /// true = 继续上局
  final bool resumeGame;

  @override
  State<GamePage> createState() => _GamePageState();
}

class _GamePageState extends State<GamePage>
    with SingleTickerProviderStateMixin {
  GameController? _controller;
  (int, int)? _selected;
  List<Move> _legalTargets = [];
  GameController get c => _controller!;

  /// 走子动画
  AnimationController? _animController;
  Move? _animMove;
  Piece? _animCaptured;
  /// 已对最后一步播放过动画的标记（避免恢复对局时重播）
  int _animatedHistoryLength = -1;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    final controller = GameController(
      engine: PikafishEngine.instance,
      prefs: widget.prefs,
      initialLevel: widget.initialLevel,
    );
    if (widget.resumeGame) {
      await controller.restoreGame();
    }
    if (widget.initialUserRed != null) {
      controller.newGame(userRed: widget.initialUserRed);
    } else if (!widget.resumeGame && controller.history.isEmpty) {
      controller.newGame();
    }
    if (!mounted) {
      controller.dispose();
      return;
    }
    setState(() {
      _controller = controller;
      _animatedHistoryLength = controller.history.length;
    });
  }

  @override
  void dispose() {
    _animController?.dispose();
    _controller?.dispose();
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
      })..forward();
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
      // 选中己方棋子（选中时清除提示箭头）
      c.clearHints();
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

  /// 结束此局（二次确认后按局势评分判定胜负）
  Future<void> _confirmEndGame() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('结束此局'),
        content: const Text('确定要结束此局吗？将根据当前局势判定胜负：\n\n'
            '分差 1000 以内为平局，某方超过 1000 则判定该方获胜。'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('继续下'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('结束'),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      setState(() {
        _selected = null;
        _legalTargets = [];
      });
      await c.endGameByScore();
      if (mounted) _showGameEnd();
    }
  }

  /// 对局结束对话框：复盘此局 / 重新开始
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
      GameStatus.draw => ('和棋', '双方局面相当，握手言和。'),
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
              openReviewLastGame(context, game: c);
            },
            child: const Text('复盘此局'),
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
            child: const Text('重新开始'),
          ),
        ],
      ),
    );
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
      builder: (context, _) {
        // 对局自然结束（将死/困毙/重复）时弹对话框
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (c.status != GameStatus.playing &&
              !c.thinking && !c.ending && mounted) {
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

        final busy = c.thinking || c.hinting || c.ending;
        final targetSquares =
            _legalTargets.map((m) => (m.toFile, m.toRank)).toList();
        final anim = _animController;
        final canLower =
            DifficultyLevel.all.indexOf(c.level) > 0;
        final canRaise =
            DifficultyLevel.all.indexOf(c.level) <
                DifficultyLevel.all.length - 1;

        return Scaffold(
          appBar: AppBar(
            title: const Text('对局'),
            centerTitle: true,
            leading: IconButton(
              icon: const Icon(Icons.arrow_back),
              tooltip: '返回',
              onPressed: () => Navigator.of(context).maybePop(),
            ),
            actions: [
              // 降难度
              IconButton(
                icon: const Icon(Icons.remove_circle_outline),
                tooltip: '降低难度',
                onPressed: canLower && !busy ? c.lowerLevel : null,
              ),
              // 当前难度图标
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 2),
                child: Center(
                  child: Tooltip(
                    message: '难度：${c.level.name}',
                    child: Icon(_levelIcon(c.level)),
                  ),
                ),
              ),
              // 升难度
              IconButton(
                icon: const Icon(Icons.add_circle_outline),
                tooltip: '提升难度',
                onPressed: canRaise && !busy ? c.raiseLevel : null,
              ),
              const SizedBox(width: 8),
            ],
          ),
          body: SafeArea(
            child: Column(
              children: [
                // 状态栏
                Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                  child: Row(
                    children: [
                      Text(
                        'AI：${c.level.name}',
                        style: const TextStyle(
                            fontSize: 13, fontWeight: FontWeight.w500),
                      ),
                      const SizedBox(width: 12),
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
                              Text('皮卡鱼思考中…',
                                  style: TextStyle(fontSize: 13)),
                            ],
                          ),
                        )
                      else if (c.hinting)
                        const Expanded(
                          child: Row(
                            children: [
                              SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                              SizedBox(width: 8),
                              Text('引擎计算建议中…',
                                  style: TextStyle(fontSize: 13)),
                            ],
                          ),
                        )
                      else if (c.ending)
                        const Expanded(
                          child: Row(
                            children: [
                              SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                              SizedBox(width: 8),
                              Text('正在分析局势判定胜负…',
                                  style: TextStyle(fontSize: 13)),
                            ],
                          ),
                        )
                      else
                        Expanded(
                          child: Text(
                            c.status == GameStatus.playing
                                ? (c.isUserTurn
                                    ? '轮到你走棋（${c.userPlaysRed ? "红" : "黑"}方）'
                                    : '轮到对方走棋')
                                : '对局结束',
                            style: const TextStyle(
                                fontSize: 13, fontWeight: FontWeight.w500),
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
                                suggestedMoves: c.hints,
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
                              suggestedMoves: c.hints,
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
                // 底部操作：悔棋 / 提示 / 结束
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 4, 12, 8),
                  child: Row(
                    children: [
                      Expanded(
                        child: FilledButton.tonalIcon(
                          onPressed: (c.history.isEmpty ||
                                  c.thinking ||
                                  c.status != GameStatus.playing)
                              ? null
                              : () {
                                  c.undo();
                                  c.clearHints();
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
                          onPressed: (busy || !c.isUserTurn ||
                                  c.status != GameStatus.playing)
                              ? null
                              : () => c.hint(),
                          icon: const Icon(Icons.lightbulb_outline, size: 18),
                          label: Text(c.hinting ? '提示中…' : '提示'),
                        ),
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: FilledButton.icon(
                          onPressed: busy ||
                                  c.status != GameStatus.playing
                              ? null
                              : _confirmEndGame,
                          icon: const Icon(Icons.stop_circle_outlined, size: 18),
                          label: const Text('结束'),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
