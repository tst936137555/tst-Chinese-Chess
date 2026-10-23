# 图标设计

使用内置 image_gen 工具生成。原始作品为 `app-source.png`，平台资源仅做高质量缩放及格式转换。

主题：朱红漆面象棋、象牙白“象”字、暖纸色背景，保留小尺寸辨识度，取消旧图标中的多行应用名。

Windows：`app.ico` 已嵌入 EXE，窗口使用预生成的 16/32/64 PNG。
Android：各密度 PNG 和 adaptive icon，主体留出系统遮罩安全区。
iOS：全套 AppIcon，1024 图标无透明通道。
macOS：`app.icns`，窗口/Dock 使用同一新图案。

完整生成提示词：

Use case: logo-brand. Asset type: production app icon for an offline Chinese chess game, shared Windows desktop, macOS, Android and iOS. Create one polished 1024x1024 square icon. A single front-facing vermilion-red lacquer Chinese chess piece with the exact bold ivory Chinese character '象' centered and extremely legible, one restrained fine ivory inset rim, a refined dark-red beveled edge and a soft short shadow. Warm ivory full-bleed background, exceptionally subtle paper texture; a tiny muted jade curved accent behind the disc may add balance. Centered strong silhouette, the chess disc occupies about 68% of the square and all essential content stays within the central 70% safe area for circle and rounded-square masks. Modern premium Chinese aesthetic, elegant restrained material depth, clean graphic design, crisp edges, excellent readability at 32px. No app name, no Latin letters, no other Chinese characters, no chessboard, no border around the canvas, no surrounding device mockup, no watermark, no pre-rounded outside corners. Opaque background.

再生成平台尺寸：`python tool/generate_icons.py`（需要 Pillow）。

