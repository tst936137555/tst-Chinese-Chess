/// 主界面：入口选择（继续上局 / 新开局 / 复盘棋谱）。
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../engine/pikafish.dart';
import '../engine/rules.dart';
import '../game/sounds.dart';
import 'announcement_dialog.dart';
import 'game_screen.dart';
import 'review_screen.dart';
import 'theme.dart';

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
            onPressed: () => showAnnouncementDialog(context),
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
                'v1.3.3',
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
