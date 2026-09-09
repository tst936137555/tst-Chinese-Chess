/// 中国象棋主入口：单机对弈（内置皮卡鱼引擎，无网络功能）。
///
/// 交互流程：主界面选择「继续上局 / 新开局」→ 选择难度 → 选择执红/执黑 → 进入对局。
/// 对局界面：顶栏为降难度 / 当前难度 / 升难度 / 返回；底栏为悔棋 / 提示 / 结束。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import 'engine/pikafish.dart';
import 'engine/rules.dart';
import 'game/game_controller.dart';
import 'game/sounds.dart';
import 'ui/board_view.dart';
import 'ui/review_screen.dart';
import 'ui/theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final prefs = await SharedPreferences.getInstance();
  runApp(XiangqiApp(prefs: prefs));
}

/// 屏幕常亮路由观察器：对局页 / 复盘列表 / 复盘分析位于栈顶时保持屏幕常亮，
/// 离开这些页面（返回主页等）自动恢复正常熄屏策略。
final wakeLockRouteObserver = WakeLockRouteObserver();

class WakeLockRouteObserver extends RouteObserver<PageRoute<dynamic>> {
  static const _wakelockRoutes = {'/game', '/archive', '/review'};

  void _sync(Route<dynamic>? route) {
    final name = route?.settings.name;
    if (name != null && _wakelockRoutes.contains(name)) {
      WakelockPlus.enable();
    } else {
      WakelockPlus.disable();
    }
  }

  @override
  void didPush(Route<dynamic> route, Route<dynamic>? previousRoute) {
    super.didPush(route, previousRoute);
    _sync(route);
  }

  @override
  void didPop(Route<dynamic> route, Route<dynamic>? previousRoute) {
    super.didPop(route, previousRoute);
    _sync(previousRoute);
  }

  @override
  void didReplace({Route<dynamic>? newRoute, Route<dynamic>? oldRoute}) {
    super.didReplace(newRoute: newRoute, oldRoute: oldRoute);
    _sync(newRoute);
  }

  @override
  void didRemove(Route<dynamic> route, Route<dynamic>? previousRoute) {
    super.didRemove(route, previousRoute);
    _sync(previousRoute);
  }
}

class XiangqiApp extends StatelessWidget {
  const XiangqiApp({super.key, required this.prefs});

  final SharedPreferences prefs;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '中国象棋',
      debugShowCheckedModeBanner: false,
      theme: xiangqiTheme(),
      // 锁定应用内字体：不随系统字体大小缩放，UI 按固定设计尺寸渲染
      builder: (context, child) {
        return MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: TextScaler.noScaling),
          child: child!,
        );
      },
      navigatorObservers: [wakeLockRouteObserver],
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
    // 初始化音效（读取开关设置）
    Sounds.instance.load(widget.prefs);
  }

  void _checkSaved() {
    final raw = widget.prefs.getString('saved_game');
    var has = raw != null && raw.isNotEmpty;
    if (has) {
      // 仅进行中的对局可续玩：已结束/损坏的存档视为无存档
      try {
        final data = jsonDecode(raw) as Map<String, dynamic>;
        has = GameStatus.values[data['status'] as int? ?? 0] ==
            GameStatus.playing;
      } catch (_) {
        has = false;
      }
    }
    if (has != _hasSavedGame) setState(() => _hasSavedGame = has);
  }

  /// 公告：作者声明对话框
  void _showAnnouncement() {
    showDialog<void>(
      context: context,
      builder: (ctx) => XqDialog(
        title: '作者声明',
        actions: [
          XqButton(
            label: '确认',
            variant: XqButtonVariant.primary,
            onPressed: () => Navigator.of(ctx).pop(),
          ),
        ],
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              '此游戏为 tst 自用象棋，自我学习使用。',
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 15, height: 1.7),
            ),
            const SizedBox(height: 16),
            const Divider(height: 1, color: XqColors.paperDark),
            const SizedBox(height: 16),
            const Text(
              '开源软件声明 / Open Source Notices',
              style: TextStyle(fontSize: 15, fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 8),
            const Text(
              '本应用基于 GNU GPL v3.0 发布，包含以下开源组件：',
              style: TextStyle(fontSize: 13, height: 1.6),
            ),
            const SizedBox(height: 10),
            _licenseSection(
              title: '1. 本应用程序',
              lines: const [
                '许可证：GNU GPL v3.0',
                '版权：Copyright © 2026 tst-936137555',
                '源码：https://github.com/tst936137555/tst-Chinese-Chess（tag: v1.3.2）',
              ],
            ),
            _licenseSection(
              title: '2. Pikafish 引擎（皮卡鱼）',
              lines: const [
                '版本：v2026-09-06',
                '许可证：GNU GPL v3.0',
                '版权：Copyright © Pikafish contributors',
                '源码：https://github.com/official-pikafish/pikafish',
                '本应用包含 Pikafish 引擎代码，依 GPLv3 条款使用，本 App 源码已公开，满足 GPLv3 对应源码要求。',
                'Android arm64-v8a 采用官方预编译版；x86_64 为官方无预编译版、',
                '依源码 tag Pikafish-2026-09-06 + NDK r28c 自行编译（构建脚本见仓库 tool/）。',
              ],
            ),
            _licenseSection(
              title: '3. Pikafish NNUE 权重文件许可证',
              lines: const [
                '随 Pikafish 发布的权重文件（pikafish.nnue）：',
                '· 仅限合法使用，超出合法范围使用的后果由用户自行承担；',
                '· 仅授权个人非商业用途免费使用，任何商业用途须另向 Pikafish 团队申请商业许可。',
                '本 App 为非商用项目，严格遵守上述限制。',
              ],
            ),
            _licenseSection(
              title: '4. 霞鹜文楷字体（LXGW WenKai）',
              lines: const [
                '版本：v1.522（Medium）',
                '许可证：SIL Open Font License 1.1（OFL-1.1）',
                '版权：Copyright 2021-2026 LXGW；基于 Fontworks 开源的 Klee One 衍生',
                '（Copyright 2020 The Klee Project Authors）',
                '源码：https://github.com/lxgw/LxgwWenKai',
                'OFL 许可证全文随本应用分发（assets/fonts/OFL.txt）。',
              ],
            ),
            const SizedBox(height: 10),
            const Text(
              '完整 GPLv3 许可证全文见：',
              style: TextStyle(fontSize: 13, height: 1.6),
            ),
            const Text(
              'https://www.gnu.org/licenses/gpl-3.0.txt',
              style: TextStyle(fontSize: 13, height: 1.6),
            ),
          ],
        ),
      ),
    );
  }

  /// 许可声明分块
  Widget _licenseSection({required String title, required List<String> lines}) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: const TextStyle(fontSize: 13.5, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 4),
          for (final line in lines)
            Text(
              line,
              style: const TextStyle(fontSize: 12.5, height: 1.6),
            ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('中国象棋'),
        centerTitle: true,
        actions: [
          // 音效开关（全游戏唯一声音开关）
          ListenableBuilder(
            listenable: Sounds.instance,
            builder: (context, _) {
              final on = Sounds.instance.enabled;
              return IconButton(
                icon: Icon(on ? Icons.volume_up : Icons.volume_off),
                tooltip: on ? '关闭音效' : '开启音效',
                onPressed: () {
                  Sounds.instance.setEnabled(!on);
                },
              );
            },
          ),
          // 公告按钮
          IconButton(
            icon: const Icon(Icons.campaign_outlined),
            tooltip: '公告',
            onPressed: _showAnnouncement,
          ),
          const SizedBox(width: 4),
        ],
      ),
      body: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(horizontal: 36, vertical: 24),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              // 标题：两侧装饰短线
              const Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  XqTitleRule(),
                  SizedBox(width: 14),
                  Text(
                    'tst自用象棋',
                    style: TextStyle(
                      fontSize: 34,
                      fontWeight: FontWeight.w700,
                      letterSpacing: 6,
                      color: XqColors.inkBlack,
                    ),
                  ),
                  SizedBox(width: 14),
                  XqTitleRule(reverse: true),
                ],
              ),
              const SizedBox(height: 8),
              const Text(
                '本地单机对弈 · 内置皮卡鱼引擎',
                style: TextStyle(
                  fontSize: 13,
                  color: XqColors.wood,
                  letterSpacing: 1,
                ),
              ),
              const SizedBox(height: 44),
              // 继续上局
              SizedBox(
                width: double.infinity,
                child: XqButton(
                  label: _hasSavedGame ? '继续上局' : '继续上局（无存档）',
                  icon: Icons.play_arrow,
                  variant: XqButtonVariant.tonal,
                  onPressed: _hasSavedGame
                      ? () async {
                          await _startGame(resume: true);
                        }
                      : null,
                ),
              ),
              const SizedBox(height: 14),
              // 新开局
              SizedBox(
                width: double.infinity,
                child: XqButton(
                  label: '新开局',
                  icon: Icons.add,
                  variant: XqButtonVariant.primary,
                  onPressed: () async {
                    await _startGame(resume: false);
                  },
                ),
              ),
              const SizedBox(height: 26),
              // 复盘棋谱入口
              XqButton(
                label: '复盘棋谱',
                icon: Icons.history,
                variant: XqButtonVariant.outline,
                height: 44,
                onPressed: () => openReviewArchive(context),
              ),
              const SizedBox(height: 40),
              // 版本标注
              const Text(
                'v1.3.2',
                style: TextStyle(
                  fontSize: 11,
                  color: XqColors.wood,
                  letterSpacing: 1,
                ),
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
        settings: const RouteSettings(name: '/game'),
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
      builder: (ctx) => XqDialog(
        title: '选择难度',
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final l in DifficultyLevel.all)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: SizedBox(
                  width: double.infinity,
                  child: XqButton(
                    label: l.name,
                    variant: XqButtonVariant.tonal,
                    height: 46,
                    onPressed: () => Navigator.of(ctx).pop(l),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
    if (level == null || !mounted) return;

    // 第二步：选择执红 / 执黑
    final userRed = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => XqDialog(
        title: '选择执子',
        actions: [
          XqButton(
            label: '执黑',
            variant: XqButtonVariant.tonal,
            onPressed: () => Navigator.of(ctx).pop(false),
          ),
          XqButton(
            label: '执红',
            variant: XqButtonVariant.primary,
            onPressed: () => Navigator.of(ctx).pop(true),
          ),
        ],
        child: const Padding(
          padding: EdgeInsets.only(top: 2),
          child: Text(
            '执红先行，执黑后手。',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 15, height: 1.7),
          ),
        ),
      ),
    );
    if (userRed == null || !mounted) return;

    Navigator.of(context).push(MaterialPageRoute(
      settings: const RouteSettings(name: '/game'),
      builder: (_) => GamePage(
        prefs: widget.prefs,
        initialLevel: level,
        initialUserRed: userRed,
      ),
    )).then((_) => _checkSaved());
  }
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
    with SingleTickerProviderStateMixin, WidgetsBindingObserver {
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
      engine: PikafishEngine.instance,
      prefs: widget.prefs,
      initialLevel: widget.initialLevel,
    );
    if (widget.resumeGame) {
      final restored = await controller.restoreGame();
      if (!restored && controller.history.isEmpty) {
        // 存档损坏：兜底开新局，避免用户执黑时轮不到任何一方走棋
        controller.newGame();
      }
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
    // 在状态通知（重建之前）同步启动走子动画，确保第一帧即从起点画起
    controller.addListener(_onGameChanged);
  }

  /// 对局状态变化：仅当新增着法时播放走子动画。
  /// 在 notifyListeners 回调（帧渲染前）触发，避免棋盘先画出终态再回跳。
  void _onGameChanged() {
    if (!mounted) return;
    final len = c.history.length;
    final prevLen = _animatedHistoryLength;
    _animatedHistoryLength = len;
    // 悔棋 / 新开局等长度减少或不变时不需要动画
    if (len == 0 || len <= prevLen) {
      // 悔棋时终止进行中的动画：否则动画会基于回退后的棋盘绘制，
      // 终点格为空出现"棋子空洞"，或让被恢复的棋子错误滑动
      if (_animMove != null) {
        _animMove = null;
        _animCaptured = null;
        _animController?.stop();
      }
      return;
    }
    final last = c.history.last;
    _animateMove(last.move, last.capturedPieceObj);
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
      builder: (ctx) => XqDialog(
        title: '结束此局',
        actions: [
          XqButton(
            label: '继续下',
            variant: XqButtonVariant.tonal,
            onPressed: () => Navigator.of(ctx).pop(false),
          ),
          XqButton(
            label: '结束',
            variant: XqButtonVariant.primary,
            onPressed: () => Navigator.of(ctx).pop(true),
          ),
        ],
        child: const Text(
          '确定要结束此局吗？将根据当前局势判定胜负：\n\n'
          '分差 600 厘兵以内为平局，某方超过 600 则判定该方获胜。',
          style: TextStyle(fontSize: 15, height: 1.7),
        ),
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

  /// 对局结束：展示结果遮罩，点击或 3 秒后出现操作按钮
  void _showGameEnd() {
    final s = c.status;
    if (s == GameStatus.playing) return;
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
        // 对局自然结束（将死/困毙/重复）时展示结果遮罩
        // （条件不满足时直接短路，避免每帧注册回调）
        if (c.status != GameStatus.playing &&
            !c.thinking && !c.ending && !_showEndOverlay && mounted) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted &&
                c.status != GameStatus.playing &&
                !c.thinking &&
                !c.ending &&
                !_showEndOverlay) {
              _showGameEnd();
            }
          });
        }

        // 走子动画已由 _onGameChanged 在状态通知时同步触发（渲染前）

        final busy = c.thinking || c.hinting || c.ending;
        final targetSquares =
            _legalTargets.map((m) => (m.toFile, m.toRank)).toList();
        final anim = _animController;
        final banner = _buildRuleBanner(c);
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
          ),
          body: SafeArea(
            child: Stack(
              children: [
                Column(
                  children: [
                // 状态栏
                XqPanel(
                  padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                  margin: const EdgeInsets.fromLTRB(12, 4, 12, 4),
                  child: Row(
                    children: [
                      Text(
                        'AI：${c.level.name}',
                        style: const TextStyle(
                            fontSize: 15,
                            fontWeight: FontWeight.w600,
                            color: XqColors.red),
                      ),
                      const SizedBox(width: 12),
                      if (c.thinking)
                        const Expanded(
                          child: Row(
                            children: [
                              SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                              SizedBox(width: 8),
                              Text('皮卡鱼思考中…',
                                  style: TextStyle(fontSize: 15)),
                            ],
                          ),
                        )
                      else if (c.hinting)
                        const Expanded(
                          child: Row(
                            children: [
                              SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                              SizedBox(width: 8),
                              Text('引擎计算建议中…',
                                  style: TextStyle(fontSize: 15)),
                            ],
                          ),
                        )
                      else if (c.ending)
                        const Expanded(
                          child: Row(
                            children: [
                              SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                              SizedBox(width: 8),
                              Text('正在分析局势判定胜负…',
                                  style: TextStyle(fontSize: 15)),
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
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                                fontSize: 15, fontWeight: FontWeight.w500),
                          ),
                        ),
                      Text(
                        '第 ${c.history.length ~/ 2 + 1} 回合',
                        style: const TextStyle(
                            fontSize: 13, color: XqColors.wood),
                      ),
                    ],
                  ),
                ),
                // 棋盘（结构固定：始终由 AnimatedBuilder 驱动，动画起止不再切换子树）。
                // 规则横幅悬浮于棋盘区顶部空白处（Stack 覆盖层）：
                // 出现/消失不改变布局高度，棋盘既不抖动也不被挤压。
                Expanded(
                  child: Stack(
                    children: [
                      Center(
                        child: Padding(
                          padding: const EdgeInsets.all(8),
                          child: AnimatedBuilder(
                            animation: anim!,
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
                  ),
                ),
                // 最近着法
                SizedBox(
                  height: 38,
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
                            fontSize: 15,
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
                  ),
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

  /// 局内醒目提示横幅：将军（红色）/ 重复局面与长将预警（橙色）。
  /// 仅对局进行中显示；将军与预警可同时出现。
  /// 返回 null 表示当前无横幅；横幅作为棋盘区 Stack 的悬浮层展示，
  /// 不参与 Column 布局，出现/消失不引起棋盘抖动。
  Widget? _buildRuleBanner(GameController c) {
    if (c.status != GameStatus.playing) return null;
    final inCheck = c.checkPos != null;
    final notice = c.ruleNotice;
    final engineNotice = c.engineNotice;
    if (!inCheck && notice == null && engineNotice == null) return null;
    final text = [
      if (inCheck) '将军！',
      ?notice,
      ?engineNotice,
    ].join('　');
    final color = inCheck ? Colors.red.shade700 : Colors.orange.shade800;
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 12),
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(10),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.25),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Row(
        children: [
          Icon(
            inCheck
                ? Icons.notification_important_rounded
                : Icons.warning_amber_rounded,
            color: Colors.white,
            size: 18,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              textAlign: TextAlign.center,
              style: const TextStyle(
                color: Colors.white,
                fontSize: 15,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// 对局结束遮罩
  Widget _buildEndOverlay() {
    final (title, msg) = _endInfo;
    return Positioned.fill(
      child: GestureDetector(
        // 未到时间前点击：立即出现操作按钮；已出现则不拦截
        onTap: _onEndOverlayTap,
        child: Container(
          color: Colors.black.withValues(alpha: 0.55),
          child: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                // 结果标题
                Text(
                  title,
                  style: const TextStyle(
                    fontSize: 44,
                    fontWeight: FontWeight.w700,
                    color: Colors.white,
                  ),
                ),
                const SizedBox(height: 10),
                Text(
                  msg,
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    fontSize: 16,
                    color: Colors.white.withValues(alpha: 0.85),
                  ),
                ),
                const SizedBox(height: 36),
                // 操作按钮：点击或 3 秒后出现
                AnimatedOpacity(
                  opacity: _endActionsReady ? 1.0 : 0.0,
                  duration: const Duration(milliseconds: 250),
                  child: IgnorePointer(
                    ignoring: !_endActionsReady,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        XqButton(
                          label: '复盘此局',
                          icon: Icons.history,
                          variant: XqButtonVariant.ghost,
                          onPressed: () async {
                            setState(() => _showEndOverlay = false);
                            await openReviewLastGame(context, game: c);
                            // 复盘返回后重新展示遮罩，按钮立即可用
                            if (mounted) {
                              setState(() {
                                _showEndOverlay = true;
                                _endActionsReady = true;
                              });
                            }
                          },
                        ),
                        const SizedBox(width: 12),
                        XqButton(
                          label: '再来一局',
                          icon: Icons.refresh,
                          variant: XqButtonVariant.primary,
                          onPressed: () {
                            setState(() {
                              _showEndOverlay = false;
                              _selected = null;
                              _legalTargets = [];
                            });
                            c.newGame();
                          },
                        ),
                        const SizedBox(width: 12),
                        XqButton(
                          label: '返回主界面',
                          icon: Icons.home_outlined,
                          variant: XqButtonVariant.ghost,
                          onPressed: _quitToHome,
                        ),
                      ],
                    ),
                  ),
                ),
                if (!_endActionsReady)
                  Padding(
                    padding: const EdgeInsets.only(top: 20),
                    child: Text(
                      '点击任意处继续',
                      style: TextStyle(
                        fontSize: 12,
                        color: Colors.white.withValues(alpha: 0.6),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

