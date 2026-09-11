/// 作者声明对话框：作者声明与开源许可证信息（GPLv3 / Pikafish / NNUE / 字体）。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;

import 'theme.dart';

/// 展示作者声明对话框
void showAnnouncementDialog(BuildContext context) {
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
              '源码：https://github.com/tst936137555/tst-Chinese-Chess',
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
            '完整许可证全文已随本应用打包（assets/licenses/），可离线查看：',
            style: TextStyle(fontSize: 13, height: 1.6),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: XqButton(
                  label: 'GPLv3 全文',
                  variant: XqButtonVariant.outline,
                  height: 40,
                  onPressed: () => _showLicenseText(
                    ctx,
                    title: 'GNU GPL v3.0',
                    assetPath: 'assets/licenses/COPYING-pikafish.txt',
                  ),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: XqButton(
                  label: 'NNUE 许可证',
                  variant: XqButtonVariant.outline,
                  height: 40,
                  onPressed: () => _showLicenseText(
                    ctx,
                    title: 'NNUE License',
                    assetPath: 'assets/licenses/NNUE-License.md',
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          const Text(
            '在线副本：https://www.gnu.org/licenses/gpl-3.0.txt',
            style: TextStyle(fontSize: 12, height: 1.5),
          ),
        ],
      ),
    ),
  );
}

/// 加载并展示随包分发的许可证全文（assets/licenses/，可离线查看）
Future<void> _showLicenseText(
  BuildContext context, {
  required String title,
  required String assetPath,
}) async {
  String text;
  try {
    text = await rootBundle.loadString(assetPath);
  } catch (_) {
    text = '许可证文本加载失败：$assetPath';
  }
  if (!context.mounted) return;
  showDialog<void>(
    context: context,
    builder: (ctx) => XqDialog(
      title: title,
      width: 380,
      actions: [
        XqButton(
          label: '关闭',
          variant: XqButtonVariant.tonal,
          onPressed: () => Navigator.of(ctx).pop(),
        ),
      ],
      child: SelectableText(
        text,
        style: const TextStyle(fontSize: 11.5, height: 1.5),
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
