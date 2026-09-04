/// 音效管理：走子/吃子/将军/胜负/和棋，支持开关（持久化）。
///
/// 音频会话配置为「与其他音频混合」：
/// - Android：STREAM_MUSIC，不请求音频焦点，不打断音乐播放器
/// - iOS/macOS：AVAudioSessionCategory.ambient，与其他音频混音
library;

import 'package:audioplayers/audioplayers.dart';
import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// 全局音效管理器
class Sounds with ChangeNotifier {
  Sounds._();
  static final Sounds instance = Sounds._();

  static const _prefKey = 'sound_enabled';

  AudioPlayer? _player;
  final List<AudioPlayer> _pool = [];
  var _poolIndex = 0;
  SharedPreferences? _prefs;

  /// 音效开关（默认开）
  bool enabled = true;

  /// 全局音频上下文：与其他音频混合，不打断用户正在播放的音乐
  static AudioContext get _mixContext => AudioContext(
        android: const AudioContextAndroid(
          isSpeakerphoneOn: false,
          audioMode: AndroidAudioMode.normal,
          contentType: AndroidContentType.sonification,
          usageType: AndroidUsageType.game, // STREAM_MUSIC
          audioFocus: AndroidAudioFocus.none, // 不请求焦点，避免打断音乐
        ),
        // ambient 类别自动与其他音频混音，且遵循静音开关
        iOS: AudioContextIOS(category: AVAudioSessionCategory.ambient),
      );

  /// 初始化：读取开关设置并配置音频会话
  Future<void> load(SharedPreferences prefs) async {
    _prefs = prefs;
    enabled = prefs.getBool(_prefKey) ?? true;
    notifyListeners();
    try {
      // 配置混音上下文（须在创建播放器前设置）
      await AudioPlayer.global.setAudioContext(_mixContext);
      _player = AudioPlayer(playerId: 'xiangqi_sfx');
      await _player!.setReleaseMode(ReleaseMode.stop);
      // 预热播放池（短音效可重叠，如快速走子）
      for (var i = 0; i < 2; i++) {
        final p = AudioPlayer(playerId: 'xiangqi_sfx_$i');
        await p.setReleaseMode(ReleaseMode.stop);
        _pool.add(p);
      }
    } catch (_) {
      // 无音频设备/测试环境时静默忽略
    }
  }

  /// 设置开关（持久化）
  Future<void> setEnabled(bool value) async {
    enabled = value;
    notifyListeners();
    await _prefs?.setBool(_prefKey, value);
  }

  /// 播放 asset 音效（忽略错误，音效失败不影响游戏）
  Future<void> _play(String asset) async {
    if (!enabled) return;
    final player = _player;
    if (player == null) return;
    try {
      // 轮询使用播放池，避免上一次还没播完被截断
      final p = _pool.isEmpty ? player : _pool[_poolIndex++ % _pool.length];
      await p.stop();
      await p.play(AssetSource('sounds/$asset'));
    } catch (_) {
      // 播放失败静默忽略
    }
  }

  /// 落子
  Future<void> place() => _play('place.wav');

  /// 吃子
  Future<void> capture() => _play('capture.wav');

  /// 将军
  Future<void> check() => _play('check.wav');

  /// 胜利
  Future<void> win() => _play('win.wav');

  /// 失败
  Future<void> lose() => _play('lose.wav');

  /// 和棋
  Future<void> draw() => _play('draw.wav');

  @override
  void dispose() {
    super.dispose();
    _player?.dispose();
    for (final p in _pool) {
      p.dispose();
    }
  }
}
