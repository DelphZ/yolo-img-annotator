# Rust Image Annotator
**Developed by SHANGHAI ROBOT EDUCATION TECHNOLOGY CO., LTD.**  
**Version:** 2.2  
**License:** Educational Use Only  

## 📖 Overview

A lightweight, local GUI tool built with Rust and egui/eframe for drawing and editing bounding box annotations on images.  
Run directly from the repository:

```bash
cargo run -- /path/to/image_folder
```

Or build a release binary:

```bash
cargo build --release
./target/release/img-annotator /path/to/image_folder
```

## Folder Structure & File Formats

- **Image Folder:** Place your images here.  
- **_darknet.labels:** A file that contains your class names. One class name per line. Created/updated when you add classes.  
- **Per-image annotation files:** `<image_name>.txt` (same base name as image). Each line:

  ```
  <class_id> <cx> <cy> <width> <height>
  ```

  - `class_id`: Zero-based index into `classes.txt` (0 = first line).
  - `cx`, `cy`, `width`, `height`: Ratios (0..1) relative to image width/height. `cx`, `cy` are box centers.

## Loader Compatibility

- When loading annotation files, the app accepts either numeric `class_id` (preferred) or textual class names (legacy).
- Unknown names/IDs are added to `classes.txt` automatically.
- IDs beyond the current `classes.txt` will create placeholder class entries.

## UI Overview

### Top Bar

- **Prev / Next:** Navigate images
- **Save:** Write current image’s `.txt`
- **Reload folder:** Re-scan image folder and `classes.txt`
- **Quit:** Exit app

### Left Panel

- **Class selector:** Pick the class for new boxes
- **Add new class:** Type a class name and click Add (appends to `classes.txt`)
- **Settings:**
  - Click tolerance (px): How close a click near a box counts as clicking it
  - Min box pixels: Minimum width/height in screen pixels to accept a new box
- **Image list:** Click an image to open it

### Center (Image Area)

- **Click-and-drag outside boxes:** Create a new box (only if width and height ≥ min box pixels)
- **Click (or near) a box:** Select it
- **Drag inside a selected box:** Move it
- **Drag a corner handle (or near a corner):** Resize it
- **On selection:** Corner handles and highlighted stroke appear
- **Using two fingers to zoom in and out** 

### Tools (Near Image)

- **Delete Selected Box**
- **Duplicate Selected Box**
- **Selected-box class combo:** Pick an existing class for the selected box
- **Assign current left-class to selected:** Set selected box class to the class currently chosen in the left panel

### Keyboard Shortcuts

- **Ctrl + Z:** Undo last change (create / move / resize / delete / duplicate / class change)
  - On macOS, you may use Command as the modifier.

- **Left Arrow Button** Move the image toward left
- **Right Arrow Button** Move the image toward right
- **Up Arrow Button** Move the image toward up
- **Down Arrow Button** Move the image toward down

## Tips & Troubleshooting

- **Annotations use class IDs** for compatibility with common training pipelines. `classes.txt` is the authoritative class list.
- **Existing annotations:** If you already have per-image `.txt` files and `classes.txt`, the app will load and let you edit them directly.
- **Legacy annotation files:** If an annotation file uses class names, the loader will accept them and convert to IDs on save.
- **Selection sensitivity:** Adjust click tolerance in the left panel.
- **Tiny boxes:** Increase min box pixels to avoid creating boxes smaller than the threshold.
- **Undo stack:** Limited to avoid excessive memory use; defaults are suitable for typical workflows.

### Common Issues

- **Images or annotations not appearing:**  
  Confirm the folder path, supported extensions (`png`, `jpg`, `jpeg`, `bmp`, `webp`, `tif`), and that per-image `.txt` files are in the same folder with the same base name.

- **Classes not saved:**  
  Ensure the process has write permission in the image folder.

## Minimal Examples

**Example `classes.txt`:**
```
blue_ring
red_ring
```

**Example annotation file (`image_001.txt`):**
```
0 0.402686 0.633823 0.076793 0.046866
1 0.113584 0.733202 0.125353 0.072840
```
(First line = class `blue_ring` (index 0) at center 0.402686, 0.633823 and size 0.076793×0.046866 of the image.)

## Intent & License

This tool is intended as a lightweight local annotator to quickly label images for training or testing.  
It’s designed to be self-contained and simple to run from the project root.

**License:**
© 2025 SHANGHAI ROBOT EDUCATION TECHNOLOGY CO., LTD.
All rights reserved.

This software is provided for educational and internal use only.
Redistribution, copying, or commercial use is strictly prohibited.

本软件版权归 上海瑞卜德教育科技有限公司 所有。
仅限教育及内部用途。
禁止任何形式的传播、拷贝或商业用途。

Violations of this license may lead to legal action.
任何违规使用行为将被追究法律责任。

---

# Rust图片标注工具（中文说明）

一个轻量级的本地GUI工具，使用Rust和egui/eframe开发，用于在图片上绘制和编辑边界框标注。  
可直接从仓库运行：

```bash
cargo run -- /path/to/image_folder
```

或先构建发布版：

```bash
cargo build --release
./target/release/img-annotator /path/to/image_folder
```

## 文件结构与格式

- **图片文件夹：** 放置你的图片。
- **_darknet.txt：** 每行一个类别名称。添加类别时自动创建/更新。
- **每张图片的标注文件：** `<image_name>.txt`（与图片同名）。每行格式：

  ```
  <class_id> <cx> <cy> <width> <height>
  ```

  - `class_id`：类别在`classes.txt`中的索引（从0开始）。
  - `cx`, `cy`, `width`, `height`：相对于图片宽高的比例（0..1），`cx`和`cy`为框中心。

## 兼容性

- 加载标注文件时，支持数字`class_id`（推荐）或类别名称（兼容旧格式）。
- 未知类别会自动添加到`classes.txt`。
- 超出当前类别列表的ID会创建占位类别。

## 用户界面简介

### 顶部栏

- **Prev / Next：** 切换图片
- **Save：** 保存当前图片标注
- **Reload folder：** 重新扫描图片文件夹和类别文件
- **Quit：** 退出程序

### 左侧面板

- **类别选择器：** 选择新建框的类别
- **添加新类别：** 输入类别名并点击Add（追加到类别文件）
- **设置：**
  - 点击容差（像素）：点击框附近多远算选中
  - 最小框像素：新建框的最小宽高（像素）
- **图片列表：** 点击图片打开

### 中央（图片区域）

- **在空白处点击拖动：** 新建框（宽高需≥最小像素）
- **点击框或附近：** 选中框
- **在选中框内拖动：** 移动框
- **拖动角点或附近：** 调整框大小
- **选中时：** 显示角点和高亮边框

### 工具栏（图片附近）

- **删除选中框**
- **复制选中框**
- **选中框类别选择器**
- **将左侧类别赋值给选中框**

### 快捷键

- **Ctrl + Z：** 撤销上一步（新建/移动/调整/删除/复制/类别变更）
  - macOS可用Command键。

## 使用提示与故障排查

- **标注使用类别ID**，兼容主流训练流程。`classes.txt`为权威类别列表。
- **已有标注文件可直接编辑**，无需重新创建。
- **旧格式类别名自动兼容**，保存时转为ID。
- **选框灵敏度可调**，左侧面板设置。
- **避免小框**，提高最小框像素。
- **撤销栈有限**，默认足够日常使用。

### 常见问题

- **图片或标注未显示：**  
  检查文件夹路径、支持的扩展名（png, jpg, jpeg, bmp, webp, tif），以及标注文件是否同名同目录。

- **类别未保存：**  
  确认程序有写入权限。

## 示例

**classes.txt：**
```
blue_ring
red_ring
```

**标注文件（image_001.txt）：**
```
0 0.402686 0.633823 0.076793 0.046866
1 0.113584 0.733202 0.125353 0.072840
```
（第一行为类别blue_ring（索引0），中心点0.402686,0.633823，宽高0.076793×0.046866）

## 目的与许可

本工具旨在快速本地标注图片，便于训练或测试。  
设计为自包含、易于运行。

**许可：** 
© 2025 SHANGHAI ROBOT EDUCATION TECHNOLOGY CO., LTD.
All rights reserved.

This software is provided for educational and internal use only.
Redistribution, copying, or commercial use is strictly prohibited.

本软件版权归 上海瑞卜德教育科技有限公司 所有。
仅限教育及内部用途。
禁止任何形式的传播、拷贝或商业用途。

Violations of this license may lead to legal action.
任何违规使用行为将被追究法律责任。