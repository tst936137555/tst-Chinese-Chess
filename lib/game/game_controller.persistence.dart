/// GameController 的存档持久化与棋谱归档实现（part 拆分）。
///
/// 本文件与 game_controller.dart 同库，私有成员互相可见；
/// 方法体自 GameController 原样搬移，行为不变。
/// 类内部可直接以 `_xxx()` 隐式调用扩展成员（无需 this 前缀）。
part of 'game_controller.dart';

/// 存档持久化与归档（自动保存 / 难度偏好 / 棋谱归档）
extension GamePersistence on GameController {
  /// 等待在飞归档落盘完成（测试收尾 / 生命周期兜底用）
  Future<void> flushArchives() => _archivePending ?? Future<void>.value();

  /// 归档对局（对局结束时）
  Future<void> _archiveGame() {
    final op = _archiveGameNow();
    _archivePending = op;
    return op;
  }

  Future<void> _archiveGameNow() async {
    if (_history.isEmpty) return;
    try {
      final ok = await GameArchive.add(
          archiveFile ?? await GameArchive.defaultArchiveFile(),
          ArchivedGame(
            time: DateTime.now(),
            userRed: userPlaysRed,
            levelName: _level.name,
            result: switch (_status) {
              GameStatus.redWin => 'redWin',
              GameStatus.blackWin => 'blackWin',
              _ => 'draw',
            },
            // 仅存 uci/captured/notation：fen 可由 uci 序列重放推导，
            // 复盘端（_historyFromArchive）以重放为准逐步重算；
            // 自定义起始局面（复盘续下）必须存 startFen 作为重放基准
            startFen: _startFen == Board.startFen ? null : _startFen,
            mode: _mode.name,
            history: _history.map((e) => {
                  'uci': e.move.uci,
                  'captured': e.capturedPiece,
                  'notation': e.notation,
                }).toList(),
          ));
      if (!ok) {
        archiveNotice = '棋谱保存失败';
      }
    } catch (e) {
      // 归档是后台任务：异常只记录，绝不冒泡为未处理异步错误
      debugPrint('棋谱归档异常: $e');
    }
  }

  /// 立即保存当前对局状态（生命周期兜底：切后台/进程终止前调用），
  /// 并等待在飞归档落盘，避免归档被进程终止打断
  Future<void> saveNow() async {
    // 页面已销毁时状态不再有效，跳过
    if (_disposed) return;
    await _saveState();
    await flushArchives();
  }

  /// 保存当前局面（自动保存）
  Future<void> _saveState() async {
    try {
      // 对局已结束：棋局不可续玩，清除存档（结果已归档到复盘棋谱）
      if (_status != GameStatus.playing) {
        await _prefs.remove('saved_game');
        _setSaveNotice(null);
        return;
      }
      await _prefs.setString('saved_game', jsonEncode({
        // 仅存 uci 序列：captured/notation/fen/status 均可在恢复重放时
        // 经 _applyMove 全量重算（旧版冗余字段由重放端兼容读取）；
        // 自定义起始局面与模式随档保存（旧档缺失字段恢复为标准开局/普通模式）
        'history': [for (final e in _history) {'uci': e.move.uci}],
        'userRed': userPlaysRed,
        'levelName': _level.name,
        'startFen': _startFen,
        'mode': _mode.name,
      }));
      _setSaveNotice(null);
    } catch (e) {
      // 自动保存失败如实提示（旧版静默吞错，用户退出后进度无声丢失）
      debugPrint('对局保存失败: $e');
      _setSaveNotice('对局保存失败，进度可能丢失：$e');
    }
  }

  Future<void> _loadSettings() async {
    try {
      final levelName = _prefs.getString('level');
      if (levelName != null) {
        for (final l in DifficultyLevel.all) {
          if (l.name == levelName) _level = l;
        }
      }
      userPlaysRed = _prefs.getBool('userRed') ?? true;
    } catch (_) {}
  }

  Future<void> _saveSettings() async {
    try {
      // 仅普通对弈写全局难度/执子偏好：复盘续下沿用原设置，
      // 不应覆盖用户普通对弈的默认选择
      if (_mode == GameMode.normal) {
        await _prefs.setString('level', _level.name);
        await _prefs.setBool('userRed', userPlaysRed);
      }
      await _saveState();
    } catch (_) {}
  }
}
