/// 对局界面：棋盘交互、走子动画、终局遮罩与局内提示。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../engine/pikafish.dart';
import '../engine/rules.dart';
import '../game/game_controller.dart';
import '../game/sounds.dart';
import 'board_view.dart';
import 'end_game_dialog.dart';
import 'game_banner.dart';
import 'game_end_overlay.dart';
import 'game_status_bar.dart';
import 'review_screen.dart';
import 'theme.dart';

class GamePage extends StatefulWidget {
  const GamePage({
    super.key,
    required this.prefs,
    this.initialLevel,
    this.initialUserRed,
    this.resumeGame = false,
    this.startFen,
    this.gameMode = GameMode.normal,
    this.engine,
  });

  final SharedPreferences prefs;
  /// 新开局时指定的难度
  final DifficultyLevel? initialLevel;
  /// 新开局时指定的执子方
  final bool? initialUserRed;
  /// true = 继续上局
  final bool resumeGame;
  /// 自定义起始局面 FEN（复盘续下）；空 = 标准开局
  final String? startFen;
  /// 对局来源模式（归档标题前缀）
  final GameMode gameMode;
  /// 测试注入伪造引擎；空则使用全局单例（生产路径不受影响）
  @visibleForTesting
  final EngineClient? engine;

  @override
  State<GamePage> createState() => _GamePageState();
}

class _GamePageState extends State<GamePage>
    with SingleTickerProviderStateMixin, WidgetsBindingObserver {
  GameController? _controller;
  (int, int)? _selected;
  List<Move> _legalTargets = [];
  GameController get c => _controller!;

  /// 测试钩子：当前对局控制器（断言用）
  @visibleForTesting
  GameController? get controller => _controller;

  /// 走子动画
  AnimationController? _animController;
  Move? _animMove;
  Piece? _animCaptured;
  /// 已对最后一步播放过动画的标记（避免恢复对局时重播）
  int _animatedHistoryLength = -1;

  /// 结束遮罩：展示结果，未到时间前不可交互
  bool _showEndOverlay = false;
  /// 是否已到可交互时间（点击或 3 秒后）
  bool _endActionsReady = false;
  Timer? _endTimer;
  /// 结果标题与文案
  (String, String) _endInfo = ('', '');

  @override
  void initState() {
    super.initState();
    // 走子动画控制器：全局复用，避免每步重建导致首帧跳变
    _animController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 260),
    )..addStatusListener((status) {
        if (status == AnimationStatus.completed && mounted && _animMove != null) {
          setState(() {
            _animMove = null;
            _animCaptured = null;
          });
        }
      });
    // 监听 App 生命周期：切后台/进程将被终止前强制保存对局
    WidgetsBinding.instance.addObserver(this);
    _init();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // 切后台（inactive/hidden/paused）：系统随时可能回收进程，
    // 立刻同步内存状态到 SharedPreferences 持久化。
    // detached：进程即将被终止（iOS 划掉、系统回收），做最后的落盘。
    if (state != AppLifecycleState.resumed) {
      _controller?.saveNow();
    }
  }

  Future<void> _init() async {
    final controller = GameController(
      engine: widget.engine ?? PikafishEngine.instance,
      prefs: widget.prefs,
      initialLevel: widget.initialLevel,
      mode: widget.gameMode,
    );
    if (widget.resumeGame) {
      final restored = await controller.restoreGame();
      if (!restored && controller.history.isEmpty) {
        // 存档损坏：兜底开新局，避免用户执黑时轮不到任何一方走棋
        controller.newGame();
      }
    }
    if (widget.initialUserRed != null) {
      controller.newGame(
        userRed: widget.initialUserRed,
        startFen: widget.startFen,
        mode: widget.gameMode,
      );
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
    // 在状态通知（重建之前）同步启动走子动画，确保第一帧即从起点画起
    controller.addListener(_onGameChanged);
  }

  /// 对局状态变化：仅当新增着法时播放走子动画。
  /// 在 notifyListeners 回调（帧渲染前）触发，避免棋盘先画出终态再回跳。
  void _onGameChanged() {
    if (!mounted) return;
    // 棋谱归档失败提示：终局遮罩会盖住横幅区，改用 SnackBar 主动弹出（只提示一次）
    final archiveNotice = c.takeArchiveNotice();
    if (archiveNotice != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          ScaffoldMessenger.of(context)
              .showSnackBar(SnackBar(content: Text(archiveNotice)));
        }
      });
    }
    final len = c.history.length;
    final prevLen = _animatedHistoryLength;
    _animatedHistoryLength = len;
    // 长度减少（悔棋 / 新开局）时终止进行中的动画：否则动画会基于回退后的
    // 棋盘绘制，终点格为空出现"棋子空洞"，或让被恢复的棋子错误滑动。
    // 长度不变（len == prevLen）只是 thinking/横幅等纯状态通知——用户落子后
    // 引擎同步开始思考，thinking 通知恰好紧跟动画启动，重复触发会让动画
    // 从头重放、音效连响，这里只跳过，不动动画。
    if (len == 0 || len < prevLen) {
      if (_animMove != null) {
        _animMove = null;
        _animCaptured = null;
        _animController?.stop();
      }
    } else if (len > prevLen) {
      final last = c.history.last;
      _animateMove(last.move, last.capturedPieceObj);
    }
    // 对局自然结束（将死/困毙/重复判和/结束此局）时展示结果遮罩。
    // 结束瞬间的过渡通知（引擎仍在收尾等）由 !thinking/!ending 挡住，
    // 待收尾完成后的通知再触发。
    if (c.status != GameStatus.playing &&
        !c.thinking &&
        !c.ending &&
        !_showEndOverlay) {
      _showGameEnd();
    }
  }

  @override
  void dispose() {
    // 页面销毁（返回主界面）前兜底保存，防止有未落盘的状态
    _controller?.removeListener(_onGameChanged);
    _controller?.saveNow();
    WidgetsBinding.instance.removeObserver(this);
    _endTimer?.cancel();
    _animController?.dispose();
    _controller?.dispose();
    super.dispose();
  }

  /// 播放某一步走子动画（复用同一个 AnimationController，forward(from:0) 重置）
  void _animateMove(Move m, Piece? captured) {
    _animMove = m;
    _animCaptured = captured;
    // 音效：将军/吃子/落子；终局音效由 _showGameEnd 播放
    final snd = Sounds.instance;
    if (c.status == GameStatus.playing && c.checkPos != null) {
      snd.check();
    } else if (captured != null) {
      snd.capture();
    } else {
      snd.place();
    }
    _animController?.forward(from: 0);
  }

  void _onTapSquare(int file, int rank) {
    if (c.thinking || c.ending || !c.isUserTurn) return;
    final piece = c.board.pieceAt(file, rank);

    if (_selected != null) {
      // 尝试走子
      final move = _legalTargets.where((m) =>
          m.toFile == file && m.toRank == rank).firstOrNull;
      if (move != null) {
        // 走子动画由 _onGameChanged 监听通知统一触发
        if (c.tryMove(move)) {
          setState(() {
            _selected = null;
            _legalTargets = [];
          });
        }
        return;
      }
    }

    if (piece != null && piece.isRed == c.userPlaysRed) {
      // 选中己方棋子（选中时清除提示箭头）
      c.clearHints();
      setState(() {
        _selected = (file, rank);
        // 复用 controller 缓存的全量合法走法，按起点过滤即可
        _legalTargets = c.legalMoves
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

  /// 结束弹窗防抖：弹窗路由尚未接管输入前的快速连点会叠开两个弹窗，
  /// 确认上层后下层会残留挡住结算遮罩
  bool _endDialogOpen = false;

  /// 结束此局：点击后立即分析当前局势，弹窗展示评分与即将判定的胜负和，
  /// 用户确认后才结算（弹窗中已算好的评分直接复用，不重复分析）
  Future<void> _confirmEndGame() async {
    if (_endDialogOpen) return;
    _endDialogOpen = true;
    setState(() {
      _selected = null;
      _legalTargets = [];
    });
    try {
      final result = await showDialog<(bool, int?)>(
        context: context,
        builder: (ctx) => EndGameDialog(controller: c),
      );
      if (result != null && result.$1) {
        await c.endGameByScore(scoreCp: result.$2);
        if (mounted) _showGameEnd();
      }
    } finally {
      _endDialogOpen = false;
    }
  }

  /// 对局结束：展示结果遮罩，点击或 3 秒后出现操作按钮
  void _showGameEnd() {
    final s = c.status;
    if (s == GameStatus.playing) return;
    // 已展示过则跳过：_onGameChanged 与 _confirmEndGame 双路径触发时只生效一次
    if (_showEndOverlay) return;
    final snd = Sounds.instance;
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
    // 终局原因（三次重复判和 / 长将判负等规则判定）
    final reason = c.endReason;
    final fullMsg = reason == null ? msg : '$msg\n$reason';
    // 终局音效
    switch (s) {
      case GameStatus.redWin:
        c.userPlaysRed ? snd.win() : snd.lose();
      case GameStatus.blackWin:
        c.userPlaysRed ? snd.lose() : snd.win();
      case GameStatus.draw:
        snd.draw();
      default:
        break;
    }
    _endTimer?.cancel();
    setState(() {
      _endInfo = (title, fullMsg);
      _showEndOverlay = true;
      _endActionsReady = false;
    });
    // 3 秒后自动出现操作按钮
    _endTimer = Timer(const Duration(seconds: 3), () {
      if (mounted && _showEndOverlay && !_endActionsReady) {
        setState(() => _endActionsReady = true);
      }
    });
  }

  /// 点击结果遮罩：立即出现操作按钮
  void _onEndOverlayTap() {
    if (!_endActionsReady) {
      _endTimer?.cancel();
      setState(() => _endActionsReady = true);
    }
  }

  /// 返回主界面（关闭结束遮罩）
  void _quitToHome() {
    _endTimer?.cancel();
    setState(() {
      _showEndOverlay = false;
      _endActionsReady = false;
    });
    Navigator.of(context).maybePop();
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
        // 终局遮罩由 _onGameChanged 监听通知统一触发（渲染前）
        return Scaffold(
          appBar: _buildAppBar(),
          body: SafeArea(
            child: Stack(
              children: [
                Column(
                  children: [
                    // 状态栏
                    GameStatusBar(controller: c),
                    // 棋盘（结构固定：始终由 AnimatedBuilder 驱动，动画起止不再切换子树）
                    Expanded(child: _buildBoardArea()),
                    // 最近着法
                    GameMoveStrip(controller: c),
                    // 底部操作：悔棋 / 提示 / 结束
                    Padding(
                      padding: const EdgeInsets.fromLTRB(12, 4, 12, 8),
                      child: _buildActionRow(),
                    ),
                  ],
                ),
                // 对局结束遮罩：结果展示 + 操作按钮（点击或 3 秒后出现）
                if (_showEndOverlay) _buildEndOverlay(),
              ],
            ),
          ),
        );
      },
    );
  }

  /// 顶部栏：返回 + 难度升降调节
  PreferredSizeWidget _buildAppBar() {
    final busy = c.thinking || c.hinting || c.ending;
    final canLower = DifficultyLevel.all.indexOf(c.level) > 0;
    final canRaise = DifficultyLevel.all.indexOf(c.level) <
        DifficultyLevel.all.length - 1;
    return AppBar(
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
        // 当前难度文字
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 2),
          child: Center(
            child: Text(
              c.level.name,
              style: const TextStyle(
                fontSize: 15,
                fontWeight: FontWeight.w600,
              ),
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
    );
  }

  /// 棋盘区：走子动画驱动棋盘重绘。
  /// 规则横幅悬浮于棋盘区顶部空白处（Stack 覆盖层）：
  /// 出现/消失不改变布局高度，棋盘既不抖动也不被挤压。
  Widget _buildBoardArea() {
    final anim = _animController!;
    final banner = buildGameRuleBanner(c);
    final targetSquares =
        _legalTargets.map((m) => (m.toFile, m.toRank)).toList();
    return Stack(
      children: [
        Center(
          child: Padding(
            padding: const EdgeInsets.all(8),
            child: AnimatedBuilder(
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
            ),
          ),
        ),
        if (banner != null)
          Positioned(
            left: 12,
            right: 12,
            top: 4,
            child: banner,
          ),
      ],
    );
  }

  /// 底部操作行：悔棋 / 提示 / 结束
  Widget _buildActionRow() {
    final busy = c.thinking || c.hinting || c.ending;
    return Row(
      children: [
        Expanded(
          child: XqButton(
            label: '悔棋',
            icon: Icons.undo,
            variant: XqButtonVariant.tonal,
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
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: XqButton(
            label: c.hinting ? '提示中…' : '提示',
            icon: Icons.lightbulb_outline,
            variant: XqButtonVariant.tonal,
            onPressed: (busy || !c.isUserTurn ||
                    c.status != GameStatus.playing)
                ? null
                : () => c.hint(),
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: XqButton(
            label: '结束',
            icon: Icons.stop_circle_outlined,
            variant: XqButtonVariant.primary,
            onPressed: busy || c.status != GameStatus.playing
                ? null
                : _confirmEndGame,
          ),
        ),
      ],
    );
  }

  /// 对局结束遮罩：结果展示 + 操作按钮（点击或 3 秒后出现）；
  /// 复盘续下无「再来一局」语义（重玩同一续下局面易误解），仅普通对局提供
  Widget _buildEndOverlay() {
    return GameEndOverlay(
      title: _endInfo.$1,
      message: _endInfo.$2,
      actionsReady: _endActionsReady,
      onTap: _onEndOverlayTap,
      onReview: _reviewFromOverlay,
      onNewGame: c.mode == GameMode.normal ? _newGameFromOverlay : null,
      onQuit: _quitToHome,
      // 续下局的返回按钮回到的是栈下的复盘分析页，按钮文字相应调整
      quitLabel: c.mode == GameMode.normal ? '返回主界面' : '返回复盘',
    );
  }

  /// 结束遮罩「复盘此局」：先收起遮罩进入复盘，返回后重新展示（按钮立即可用）
  Future<void> _reviewFromOverlay() async {
    setState(() => _showEndOverlay = false);
    await openReviewLastGame(context, game: c, prefs: widget.prefs);
    if (mounted) {
      setState(() {
        _showEndOverlay = true;
        _endActionsReady = true;
      });
    }
  }

  /// 结束遮罩「再来一局」：清空选子状态并开新局
  void _newGameFromOverlay() {
    setState(() {
      _showEndOverlay = false;
      _selected = null;
      _legalTargets = [];
    });
    c.newGame();
  }
}
