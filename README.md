# 服装质检系统

Windows 与 macOS 均可使用的完全离线桌面应用。程序不联网、不部署；批次、箱号、UPC 和质检记录保存在本机 SQLite，异常图片保存在本机应用数据目录。

## 当前业务规则

- 一个批次包含多个箱号，一个箱号可以包含多个 UPC。
- 从 `p.xlsx` 格式的 `.xlsx` 文件导入箱号、UPC 和参考数量；左侧箱号每页最多显示 70 个。
- A/B/C/D 均可新增多行。每行填写商品条码、数量、异常原因和最多 3 张 JPG/PNG 图片。
- 所有等级的数量默认 1，均可修改；每一填写行是一条独立质检记录。
- D 级自动从封箱数量中排除，封箱数量为 A+B+C。
- 本箱有记录后才能完成；完成后可重新开放。修改本箱时整箱数据一次性保存，失败不会留下半套修改。
- 批次报表显示各等级数量、占比，以及每个箱号的参考数量与实际数量差异。
- 删除批次需要两次不同形式的确认，并删除该批次的箱号、记录和本地图片。

## Excel 导出

程序一次导出：

- `封箱单 批次号 日期.xlsx`：以 `template-2.xlsx` 为模板，每个箱号一张工作表；箱号超过模板工作表数量时自动复制工作表，不设 42 箱限制。只写入 A/B/C，逐件展开，D 不进入封箱单。
- `入库质检报告 批次号 日期.xlsx`：以 `template-1.xlsx` 为模板，仅保留 A/B/C/D 等级表和 `瑕疵照片 Sample`。

等级表逐件展开。瑕疵照片表中 B 级每条记录占一行并保留录入数量，C/D 按数量逐件展开。G/H/I 列宽为 50，数据行高为 60；图片使用随单元格移动和缩放的双单元格锚点，最多 3 张。

## 本地开发

需要 Node.js、Rust stable 和 Tauri 2 对应的系统构建依赖。

```bash
npm install
rustup default stable
npm run tauri dev
```

检查前端与后端：

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

构建安装包：

```bash
npm run tauri build
```

Windows 安装包需要在 Windows 构建，macOS 安装包需要在 macOS 构建。图片仅支持 JPG/JPEG/PNG。

## 数据说明

当前版本使用全新的 `quality_v2.db` 和 `photos_v2`，不读取、不迁移旧版本数据。旧数据库文件不会被程序自动删除。
