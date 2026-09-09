// 传输层：子进程传输（Android/macOS/Windows）与进程内 FFI 传输（iOS）。
//
// 本文件是 pikafish.dart 的 part（共享库级导入与私有成员）。
// 两者以相同抽象对接引擎会话的 UCI 行协议处理，
// 握手/看门狗/请求路由/MultiPV 解析完全不感知传输差异。
part of 'pikafish.dart';

/// iOS 进程内引擎标记：exePath 位不承载路径，改选 FFI 传输层。
const String _ffiMarker = '__ffi__';

/// 引擎传输层工厂：生产为进程/FFI 实现，测试可注入内存伪造实现
/// （伪造实现随 [PikafishEngine.debugIoFactory] 注入，无需真实引擎二进制）。
typedef EngineIoFactory = Future<EngineIo> Function({
  required void Function(String line) onLine,
  required void Function(int code) onExit,
});

/// 引擎 I/O 抽象：发送 UCI 命令行 + 终止引擎。
/// 输出行与退出事件在构造时（各 start 工厂）以回调注册。
abstract class EngineIo {
  void send(String line);
  void kill();
}

/// 子进程传输：Android（jniLibs 解压路径）/ macOS / Windows。
class _ProcessIo implements EngineIo {
  _ProcessIo._(this._proc);

  final Process _proc;

  static Future<_ProcessIo> start(
    String exePath, {
    required void Function(String line) onLine,
    required void Function(int code) onExit,
  }) async {
    final proc = await Process.start(exePath, []);
    proc.stdout
        .transform(const Utf8Decoder())
        .transform(const LineSplitter())
        .listen(onLine);
    proc.exitCode.then(onExit);
    return _ProcessIo._(proc);
  }

  @override
  void send(String line) {
    try {
      _proc.stdin.writeln(line);
    } catch (_) {
      // 进程已退出：写入失败，由退出回调走失败/崩溃通道
    }
  }

  @override
  void kill() => _proc.kill();
}

/// 进程内 FFI 传输（iOS）：引擎静态库经 tool/setup_ios_engine.sh 链入
/// Runner 主可执行文件，C 适配层（ios/EngineShim/）在独立原生线程上驱动
/// Stockfish::main（上游 -DUNIVERSAL_BINARY 收编的原 main）并把
/// stdin/stdout 重定向为行队列。
///
/// - 输出行经 NativeCallable.listener 投递：回调来自引擎原生线程（非
///   isolate 线程），listener 跨线程转投 isolate 事件循环，行处理逻辑
///   与子进程模式完全一致。
/// - 'quit' 由适配层注入引擎输入队列优雅收线；失去进程隔离是已知代价
///   （引擎线程真挂死时无法强杀，见 iOS FFI 可行性评估）。
/// - NativeCallable 不显式 close：close 后原生线程仍回调属未定义行为，
///   保持引用至 isolate 终止，由运行时统一回收。
class _FfiIo implements EngineIo {
  _FfiIo._(this._b, this._lineCb, this._exitCb);

  final _FfiBindings _b;

  // ignore: unused_field — 存活引用（防垃圾回收/误报，见类注释）
  final NativeCallable<_SfLineCbC> _lineCb;

  // ignore: unused_field — 存活引用（防垃圾回收/误报，见类注释）
  final NativeCallable<_SfExitCbC> _exitCb;

  /// 静态库是否已链入主可执行文件（符号探测；未链接时 iOS 优雅降级）
  static bool get supported => _FfiBindings.instance != null;

  static Future<_FfiIo> start({
    required void Function(String line) onLine,
    required void Function(int code) onExit,
  }) async {
    final b = _FfiBindings.instance;
    if (b == null) {
      throw StateError('引擎静态库未链接（缺少 sf_start 等符号）');
    }
    // 行缓冲由适配层 malloc 分配，Dart 复制字符串后立即释放
    final lineCb = NativeCallable<_SfLineCbC>.listener((Pointer<Uint8> p) {
      final line = _readCString(p);
      b.free(p.cast<Void>());
      onLine(line);
    });
    final exitCb = NativeCallable<_SfExitCbC>.listener(onExit);
    final rc = b.start(lineCb.nativeFunction, exitCb.nativeFunction);
    if (rc != 0) {
      lineCb.close();
      exitCb.close();
      throw StateError('sf_start 失败（rc=$rc，上一轮引擎可能仍在运行）');
    }
    return _FfiIo._(b, lineCb, exitCb);
  }

  static String _readCString(Pointer<Uint8> p) {
    var len = 0;
    while (p[len] != 0) {
      len++;
    }
    return utf8.decode(p.asTypedList(len), allowMalformed: true);
  }

  @override
  void send(String line) {
    final bytes = utf8.encode(line);
    final p = malloc<Uint8>(bytes.length + 1);
    try {
      p.asTypedList(bytes.length).setAll(0, bytes);
      p[bytes.length] = 0;
      _b.send(p);
    } finally {
      malloc.free(p);
    }
  }

  @override
  void kill() {
    // 优雅收线：注入 quit 令 UCI 主循环返回；2s 内未结束则由适配层放弃
    _b.stop();
  }
}

/// C 适配层绑定（接口见 ios/EngineShim/pikafish_shim.h）。
class _FfiBindings {
  _FfiBindings._(this.start, this.send, this.stop, this.free);

  final _SfStartD start;
  final _SfSendD send;
  final _SfStopD stop;
  final _SfFreeD free;

  static _FfiBindings? _cache;

  /// 探测并缓存绑定；符号缺失（静态库未链接）返回 null，绝不抛出。
  static _FfiBindings? get instance {
    final cached = _cache;
    if (cached != null) return cached;
    try {
      // 静态链接：符号位于主可执行文件（Flutter 官方 FFI 静态链接方式，
      // 配合 Xcode OTHER_LDFLAGS 的 -Wl,-force_load 防止链接器裁剪）
      final lib = DynamicLibrary.executable();
      return _cache = _FfiBindings._(
        lib.lookupFunction<_SfStartC, _SfStartD>('sf_start'),
        lib.lookupFunction<_SfSendC, _SfSendD>('sf_send'),
        lib.lookupFunction<_SfStopC, _SfStopD>('sf_stop'),
        lib.lookupFunction<_SfFreeC, _SfFreeD>('sf_free'),
      );
    } catch (_) {
      return null;
    }
  }
}

// —— C 适配层签名（与 pikafish_shim.h 一一对应）——
typedef _SfLineCbC = Void Function(Pointer<Uint8> line);
typedef _SfExitCbC = Void Function(Int32 code);
typedef _SfStartC = Int32 Function(
    Pointer<NativeFunction<_SfLineCbC>> onLine,
    Pointer<NativeFunction<_SfExitCbC>> onExit);
typedef _SfStartD = int Function(
    Pointer<NativeFunction<_SfLineCbC>> onLine,
    Pointer<NativeFunction<_SfExitCbC>> onExit);
typedef _SfSendC = Int32 Function(Pointer<Uint8> line);
typedef _SfSendD = int Function(Pointer<Uint8> line);
typedef _SfStopC = Void Function();
typedef _SfStopD = void Function();
typedef _SfFreeC = Void Function(Pointer<Void> ptr);
typedef _SfFreeD = void Function(Pointer<Void> ptr);
