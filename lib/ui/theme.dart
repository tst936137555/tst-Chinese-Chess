/// 全局视觉主题：与棋盘配色同源的「宣纸 · 木色 · 朱红」中国风。
///
/// 统一约定：
/// - 按钮 [XqButton]：高度 48、圆角 12、字号 16/w600，四档配色
/// - 弹窗 [XqDialog]：宽 320、圆角 16、纸色底 + 细描边、居中标题
/// - 面板 [XqPanel]：纸色圆角容器，用于状态栏/信息卡等分块
library;

import 'package:flutter/material.dart';

/// 调色板（与棋盘绘制配色同源）
abstract final class XqColors {
  /// 宣纸（面板/弹窗底色）
  static const paper = Color(0xFFF7EBD3);

  /// 纸色深一档（描边/分隔线）
  static const paperDark = Color(0xFFE3D2AC);

  /// 深木色（标题/线条）
  static const wood = Color(0xFF5B3A1E);

  /// 棋盘木色（点缀）
  static const board = Color(0xFFE8C88A);

  /// 朱红（主色，与红子同源）
  static const red = Color(0xFF8B2F1F);

  /// 墨色（正文文字）
  static const inkBlack = Color(0xFF3A2A1A);
}

/// 全局主题
ThemeData xiangqiTheme() {
  const scheme = ColorScheme.light(
    primary: XqColors.red,
    onPrimary: Colors.white,
    secondary: XqColors.board,
    surface: XqColors.paper,
    onSurface: XqColors.inkBlack,
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    scaffoldBackgroundColor: const Color(0xFFF3E7CD),
    appBarTheme: const AppBarTheme(
      backgroundColor: XqColors.red,
      foregroundColor: Colors.white,
      centerTitle: true,
      elevation: 0,
      titleTextStyle: TextStyle(
        fontSize: 18,
        fontWeight: FontWeight.w700,
        color: Colors.white,
        letterSpacing: 3,
      ),
      iconTheme: IconThemeData(color: Colors.white),
    ),
    textTheme: const TextTheme(
      bodyMedium: TextStyle(color: XqColors.inkBlack, height: 1.5),
    ),
    snackBarTheme: SnackBarThemeData(
      backgroundColor: XqColors.wood,
      contentTextStyle: const TextStyle(color: Colors.white, fontSize: 14),
      behavior: SnackBarBehavior.floating,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
    ),
    dividerTheme: const DividerThemeData(color: XqColors.paperDark, thickness: 1),
    dialogTheme: DialogThemeData(
      backgroundColor: XqColors.paper,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: const BorderSide(color: XqColors.paperDark, width: 1.5),
      ),
    ),
    progressIndicatorTheme: const ProgressIndicatorThemeData(color: XqColors.red),
  );
}

/// 按钮配色档位
enum XqButtonVariant {
  /// 朱红底白字（主操作：新开局 / 结束 / 执红 等）
  primary,

  /// 纸色底红字（次操作：悔棋 / 提示 / 取消 等）
  tonal,

  /// 纸色底墨字（轻操作：复盘入口 等）
  outline,

  /// 深木色底白字（深色遮罩上的次操作）
  ghost,
}

/// 统一风格的按钮
class XqButton extends StatelessWidget {
  const XqButton({
    super.key,
    required this.label,
    this.onPressed,
    this.icon,
    this.variant = XqButtonVariant.tonal,
    this.height = 48,
  });

  final String label;
  final VoidCallback? onPressed;
  final IconData? icon;
  final XqButtonVariant variant;
  final double height;

  @override
  Widget build(BuildContext context) {
    final disabled = onPressed == null;
    final (bg, fg) = switch (variant) {
      XqButtonVariant.primary => (XqColors.red, Colors.white),
      XqButtonVariant.tonal => (XqColors.paper, XqColors.red),
      XqButtonVariant.outline => (XqColors.paper, XqColors.inkBlack),
      XqButtonVariant.ghost => (XqColors.wood, Colors.white),
    };
    // 禁用态统一为实心灰底深灰字（与配色档位无关）
    final fgFinal = disabled ? Colors.grey.shade600 : fg;
    final bgFinal = disabled ? Colors.grey.shade300 : bg;
    final radius = BorderRadius.circular(12);
    return SizedBox(
      height: height,
      child: Material(
        color: bgFinal,
        borderRadius: radius,
        child: InkWell(
          onTap: onPressed,
          borderRadius: radius,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                if (icon != null) ...[
                  Icon(icon, size: 20, color: fgFinal),
                  const SizedBox(width: 6),
                ],
                Flexible(
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                      color: fgFinal,
                      letterSpacing: 1,
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

/// 统一风格的弹窗：居中标题 + 内容区 + 底部按钮行。
///
/// [actions] 为 [XqButton]；多于一个时等宽排列。
class XqDialog extends StatelessWidget {
  const XqDialog({
    super.key,
    required this.title,
    required this.child,
    this.actions = const [],
    this.width = 320,
  });

  final String title;
  final Widget child;
  final List<Widget> actions;
  final double width;

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: Colors.transparent,
      child: Container(
        width: width,
        padding: const EdgeInsets.fromLTRB(22, 18, 22, 16),
        decoration: BoxDecoration(
          color: XqColors.paper,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: XqColors.paperDark, width: 1.5),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.22),
              blurRadius: 18,
              offset: const Offset(0, 6),
            ),
          ],
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              title,
              textAlign: TextAlign.center,
              style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w700,
                color: XqColors.inkBlack,
                letterSpacing: 2,
              ),
            ),
            const SizedBox(height: 6),
            const Divider(height: 1, color: XqColors.paperDark),
            const SizedBox(height: 14),
            Flexible(child: SingleChildScrollView(child: child)),
            if (actions.isNotEmpty) ...[
              const SizedBox(height: 18),
              Row(
                children: [
                  for (var i = 0; i < actions.length; i++) ...[
                    if (i > 0) const SizedBox(width: 12),
                    Expanded(child: actions[i]),
                  ],
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// 统一风格的圆角面板（状态栏 / 信息卡等分块容器）
class XqPanel extends StatelessWidget {
  const XqPanel({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
    this.margin,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final EdgeInsetsGeometry? margin;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: margin,
      padding: padding,
      decoration: BoxDecoration(
        color: XqColors.paper,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: XqColors.paperDark, width: 1),
      ),
      child: child,
    );
  }
}

/// 主界面装饰性分隔线（标题两侧短线）
class XqTitleRule extends StatelessWidget {
  const XqTitleRule({super.key, this.width = 56, this.reverse = false});

  final double width;

  /// true = 渐变方向翻转（标题右侧用）
  final bool reverse;

  @override
  Widget build(BuildContext context) {
    final colors = [
      XqColors.red.withValues(alpha: 0),
      XqColors.red,
    ];
    return Container(
      width: width,
      height: 1.5,
      decoration: BoxDecoration(
        gradient: LinearGradient(
          colors: reverse ? colors.reversed.toList() : colors,
        ),
      ),
    );
  }
}
