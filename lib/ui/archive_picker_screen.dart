/// 归档棋谱选择页：棋谱列表、收藏置顶、清空与进入复盘。
library;

import 'package:flutter/material.dart';

import '../engine/rules.dart';
import '../game/game_archive.dart';
import '../game/game_controller.dart';
import 'review_screen.dart';
import 'theme.dart';

/// 复盘棋谱：打开存档列表选择一局
Future<void> openReviewArchive(BuildContext context) {
  return Navigator.of(context).push(MaterialPageRoute(
    settings: const RouteSettings(name: '/archive'),
    fullscreenDialog: true,
    builder: (_) => const ArchivePickerScreen(),
  ));
}

/// 从存档重建历史；走法非法或局面数据损坏时截断（丢弃其后数据），
/// 避免损坏的 fen 在复盘 build 中触发 Board.fromFen 异常
List<HistoryEntry> _historyFromArchive(ArchivedGame game) {
  final history = <HistoryEntry>[];
  final board = Board();
  for (final e in game.history) {
    final uci = e['uci'] as String? ?? '';
    if (uci.length < 4) break;
    final Move m;
    try {
      m = Move.fromUci(uci);
    } catch (_) {
      break;
    }
    if (!board.isLegal(m)) break;
    final fen = e['fen'] as String? ?? '';
    final Board after;
    try {
      after = Board.fromFen(fen); // 解析即校验
    } catch (_) {
      break;
    }
    history.add(HistoryEntry(
      move: m,
      capturedPiece: e['captured'] as String?,
      notation: e['notation'] as String? ?? uci,
      fenAfter: fen,
      posHash: after.positionHash,
    ));
    board.makeMove(m);
  }
  return history;
}

/// 存档选择页
class ArchivePickerScreen extends StatefulWidget {
  const ArchivePickerScreen({super.key});

  @override
  State<ArchivePickerScreen> createState() => _ArchivePickerScreenState();
}

class _ArchivePickerScreenState extends State<ArchivePickerScreen> {
  List<ArchivedGame> _games = [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final file = await GameArchive.defaultArchiveFile();
    // 存在且非空的文件却读出空列表 = 存档损坏（正常情况不会写入空档）
    final suspicious = file.existsSync() && file.lengthSync() > 0;
    _games = await GameArchive.loadAll(file);
    final corrupted = suspicious && _games.isEmpty;
    _resort();
    _loading = false;
    if (mounted) setState(() {});
    if (corrupted && mounted) {
      // 等首帧完成后再提示，避免 initState 期间展示 SnackBar
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('棋谱读取失败，数据可能已损坏')),
          );
        }
      });
    }
  }

  /// 置顶排序：收藏在前，各组内保持"新的在前"
  void _resort() {
    final favs = <ArchivedGame>[];
    final rest = <ArchivedGame>[];
    for (final g in _games) {
      (g.favorite ? favs : rest).add(g);
    }
    _games = [...favs, ...rest];
  }

  /// 切换收藏并持久化（收藏置顶且不会被自动移除）。
  /// 锁内重读-定位-改-存，不依赖本页持有的旧快照：归档页打开期间
  /// 新终局归档落盘后不会被旧快照覆写丢失。
  /// 收藏数达上限时拒绝新增；保存失败时回滚列表并提示。
  Future<void> _toggleFavorite(ArchivedGame game) async {
    final updated = game.withFavorite(!game.favorite);
    // 先查后改：收藏数已达上限时拒绝新增
    if (updated.favorite &&
        _games.where((g) => g.favorite).length >= kMaxFavoriteGames) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(
          content: Text(
              '收藏已达上限（$kMaxFavoriteGames 条），请先取消部分收藏')));
      return;
    }
    // 浅拷贝快照：失败回滚用
    final before = List<ArchivedGame>.of(_games);
    setState(() {
      _games[_games.indexOf(game)] = updated;
      _resort();
    });
    final file = await GameArchive.defaultArchiveFile();
    final result = await GameArchive.toggleFavorite(file, game);
    if (result == null) {
      // 保存失败或条目已不在存档中：回滚列表并提示
      if (!mounted) return;
      setState(() {
        _games = before;
        _resort();
      });
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('棋谱保存失败')));
      return;
    }
    // 以文件最新内容为准（可能含并发新增的对局）
    if (!mounted) return;
    setState(() {
      _games = result;
      _resort();
    });
  }

  /// 删除单局棋谱（滑动确认后调用）。
  /// 快照回滚 + 锁内重读定位删除，同 [_toggleFavorite] 范式：
  /// 归档页打开期间新终局归档落盘后不会被旧快照覆写丢失。
  Future<void> _removeGame(ArchivedGame game) async {
    final key = GameArchive.keyOf(game);
    // 浅拷贝快照：失败回滚用
    final before = List<ArchivedGame>.of(_games);
    setState(() {
      _games.removeWhere((g) => GameArchive.keyOf(g) == key);
    });
    final file = await GameArchive.defaultArchiveFile();
    final result = await GameArchive.remove(file, game);
    if (!mounted) return;
    if (result == null) {
      // 写入失败：回滚列表并提示
      setState(() {
        _games = before;
        _resort();
      });
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('棋谱删除失败')));
      return;
    }
    // 以文件最新内容为准（可能含并发新增的对局）
    setState(() {
      _games = result;
      _resort();
    });
  }

  /// 选中一局后，在棋谱列表之上打开复盘分析页；退出复盘时返回本列表
  Future<void> _openReview(BuildContext context, ArchivedGame game) async {
    final history = _historyFromArchive(game);
    if (history.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('该棋谱数据异常')),
      );
      return;
    }
    await Navigator.of(context).push(MaterialPageRoute(
      fullscreenDialog: true,
      builder: (_) => ReviewScreen(
        history: history,
        userPlaysRed: game.userRed,
      ),
    ));
  }

  /// 规则说明弹窗：棋谱保留规则 + 左滑删除操作说明
  void _showRulesDialog(BuildContext context) {
    showDialog<void>(
      context: context,
      builder: (ctx) => XqDialog(
        title: '规则说明',
        actions: [
          XqButton(
            label: '我知道了',
            variant: XqButtonVariant.primary,
            onPressed: () => Navigator.pop(ctx),
          ),
        ],
        child: const Text(
          '· 棋谱最多保留100局，收藏上限50局，超出自动移除最早对局，收藏棋谱不会被自动移除。\n\n'
          '· 在棋谱列表中向左滑动任意一局即可删除该棋谱，删除前会弹出确认框，删除后不可恢复。',
          style: TextStyle(fontSize: 14, height: 1.7),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('复盘棋谱'),
        centerTitle: true,
        actions: [
          // 规则说明按钮：替代原副标题小字，点击弹窗查看
          Padding(
            padding: const EdgeInsets.only(right: 8),
            child: TextButton.icon(
              onPressed: () => _showRulesDialog(context),
              icon: const Icon(Icons.info_outline, size: 18),
              label: const Text('规则说明', style: TextStyle(fontSize: 13)),
              style: TextButton.styleFrom(
                foregroundColor: Colors.white,
              ),
            ),
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _games.isEmpty
              ? const Center(child: Text('暂无历史棋谱\n完成一局对战后自动保存', textAlign: TextAlign.center))
              : ListView.separated(
                  itemCount: _games.length,
                  separatorBuilder: (_, _) => const Divider(height: 1),
                  itemBuilder: (ctx, i) {
                    final g = _games[i];
                    final isWin = g.resultLabel == '胜';
                    final isDraw = g.resultLabel == '和';
                    // 左滑删除：以条目身份键（时间+执子方）标识，
                    // 未确认（取消/点弹窗外）时弹回原位
                    return Dismissible(
                      key: ValueKey(GameArchive.keyOf(g)),
                      direction: DismissDirection.endToStart,
                      background: Container(
                        color: XqColors.red,
                        alignment: Alignment.centerRight,
                        padding: const EdgeInsets.only(right: 24),
                        child: const Icon(Icons.delete_outline,
                            color: Colors.white),
                      ),
                      confirmDismiss: (_) => showDialog<bool>(
                        context: context,
                        builder: (dlgCtx) => XqDialog(
                          title: '删除棋谱',
                          actions: [
                            XqButton(
                              label: '取消',
                              variant: XqButtonVariant.tonal,
                              onPressed: () => Navigator.pop(dlgCtx, false),
                            ),
                            XqButton(
                              label: '删除',
                              variant: XqButtonVariant.primary,
                              onPressed: () => Navigator.pop(dlgCtx, true),
                            ),
                          ],
                          child: Text(
                            '确定删除「${g.title}」吗？\n此操作不可恢复。',
                            style: const TextStyle(fontSize: 14, height: 1.7),
                          ),
                        ),
                      ),
                      onDismissed: (_) => _removeGame(g),
                      child: ListTile(
                        contentPadding:
                            const EdgeInsets.symmetric(horizontal: 20),
                        leading: CircleAvatar(
                          radius: 17,
                          backgroundColor: isWin
                              ? const Color(0xFF2E7D32)
                              : isDraw
                                  ? XqColors.wood
                                  : XqColors.red,
                          child: Text(
                            g.resultLabel,
                            style: const TextStyle(
                                color: Colors.white, fontWeight: FontWeight.w700),
                          ),
                        ),
                        // 标题：【对局时间-执红/执黑-胜负】
                        title: Text(
                          g.title,
                          style: const TextStyle(
                              fontSize: 14, color: XqColors.inkBlack),
                        ),
                        subtitle: Text('${g.history.length} 步 · ${g.levelName}',
                            style: const TextStyle(
                                fontSize: 12, color: XqColors.wood)),
                        trailing: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            // 收藏开关：置顶且不会被自动移除
                            IconButton(
                              icon: Icon(
                                g.favorite
                                    ? Icons.star_rounded
                                    : Icons.star_border_rounded,
                                color: g.favorite
                                    ? const Color(0xFFF5A623)
                                    : XqColors.wood,
                              ),
                              tooltip: g.favorite ? '取消收藏' : '收藏',
                              visualDensity: VisualDensity.compact,
                              padding: EdgeInsets.zero,
                              constraints: const BoxConstraints(),
                              onPressed: () => _toggleFavorite(g),
                            ),
                            const Icon(Icons.chevron_right,
                                color: XqColors.wood),
                          ],
                        ),
                        onTap: () => _openReview(context, g),
                      ),
                    );
                  },
                ),
    );
  }
}
