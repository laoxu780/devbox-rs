# DevBox（Rust + GPUI 重写版）

![Rust](https://img.shields.io/badge/Rust-1.88%2B-orange)
![Platform](https://img.shields.io/badge/Platform-macOS-blue)
![License](https://img.shields.io/badge/License-Apache--2.0-blue)

> A local developer toolbox, rewritten in Rust with GPUI — daily utilities, a runtime-switchable local reverse proxy, and SSH key / credential / port-forward management.

DevBox 是一个本地开发工具箱：日常小工具 + **可运行时切换后端的本地接口反向代理** + **SSH 密钥 / 凭证 / 端口转发管理**。

本目录是原 Go + Wails 版本的完整 Rust 重写，界面使用官方 [`gpui`](https://crates.io/crates/gpui)，网络服务运行在独立的 Tokio 运行时中。配置文件与原版保持相同的路径与 JSON 结构，可直接复用已有规则。

## 功能

| 分类 | 工具 | 说明 |
| --- | --- | --- |
| 总览 | 仪表盘 | 代理 / 转发状态总览、快捷开关 |
| 数据处理 | JSON 工具 | 格式化、压缩、校验 |
| 数据处理 | 文本对比 | 并排 / 合并视图查看差异 |
| 数据处理 | 二维码 / 条码 | 生成 QR、Code 39/128、EAN-13/8、UPC-A、ITF-14、Codabar；从图片或 PDF 解码二维码 |
| 编码解码 | Base64 编解码 | 文本与 Base64 互转 |
| 编码解码 | Hash 计算 | MD5、SHA-1、SHA-256 |
| 生成器 | 时间戳转换 | Unix 时间戳与本地 / UTC 日期互转 |
| 生成器 | UUID 生成器 | 批量生成 UUID v4 |
| 校验测试 | 正则测试 | 实时匹配、分组与高亮预览 |
| 网络工具 | 接口环境转发 | 本地端口固定，一键切换开发 / 生产后端 |
| 网络工具 | Hosts 环境 | 多套域名映射环境，一键切换系统 hosts |
| 网络工具 | SSH 密钥 | 查看 `~/.ssh` 密钥、生成新密钥、查看 / 复制公钥 |
| 网络工具 | SSH 凭证 | 保存服务器地址、密码或私钥认证 |
| 网络工具 | SSH 端口转发 | 本地转发、SOCKS5、HTTP 代理，断线自动重连 |

## 环境要求

- macOS（GPUI 当前主要支持 macOS）
- Rust 1.88 或更新版本（edition 2024，代码使用了 let-chains）
- Xcode Command Line Tools（链接系统框架）
- 可选：`ssh-keygen`（生成 / 读取 SSH 密钥，macOS 自带）
- 可选：`pdftoppm`（PDF 解码优先使用；没有时自动回退到 macOS 自带的 Quick Look）

依赖中启用了 `gpui` 的 `runtime_shaders` 特性，因此**不需要**安装 Metal Toolchain。

## 快速开始

```bash
git clone git@github.com:laoxu780/devbox-rs.git
# 或者：git clone https://github.com/laoxu780/devbox-rs.git
cd devbox-rs

cargo run                    # 调试运行（首次会编译大量依赖）
cargo build --release        # 发布构建
./target/release/devbox      # 运行发布版

cargo test                   # 运行测试
```

从终端启动即可，当前以普通可执行文件运行（非 `.app` 打包）。发布构建可用打包脚本生成桌面应用（见下文）。

## 使用说明

### 接口环境转发

1. 在「接口环境转发」中新增规则，填写监听端口（如 `8787`）和多个后端环境（如开发 / 生产）。
2. 前端（Vite 示例）始终指向本地端口：

   ```js
   server: {
     proxy: {
       '/api': { target: 'http://127.0.0.1:8787', changeOrigin: true },
     },
   }
   ```

3. 在界面中点击环境标签即可切换，前端无需重启。

规则同时提供管理接口（监听地址 + `/__devbox__/`）：

```bash
# 查看当前环境
curl http://127.0.0.1:8787/__devbox__/status

# 按环境名称或 ID 切换（名称不区分大小写）
curl -X POST 'http://127.0.0.1:8787/__devbox__/switch?target=生产环境'
curl -X POST http://127.0.0.1:8787/__devbox__/switch -d '{"target":"prod"}'
```

- 路径前缀（如 `/api`）会在转发时自动去掉。
- 支持 CORS 预检、HTTP/2、WebSocket 透传、自签名证书忽略（按环境配置）。
- 监听非回环地址时，界面会提示局域网内其他设备也可访问。

### 二维码 / 条码

- 生成：二维码、Code 39、Code 128、EAN-13、EAN-8、UPC-A、ITF-14、Codabar；切换类型时会自动填入示例内容。
- 解码：「选择图片 / PDF 解码」支持 PNG、JPG、TIFF 以及 PDF。PDF 会先逐页渲染为图片，再逐页识别二维码与条码（Code 128、EAN、UPC、ITF、Codabar 等）；多个结果逐行显示。

### Hosts 环境

- 可保存多套域名映射（每行 `IP 域名 [域名…]`，支持 `#` 注释），应用时只会替换 hosts 文件中由 DevBox 管理的标记块，手动添加的条目会保留。
- 应用时需要 macOS 管理员授权（系统弹窗输入密码）；Windows 需以管理员身份运行。
- 「清除 DevBox 配置」只移除管理块，不影响其他条目。

### SSH

- **密钥**：`~/.ssh` 中的密钥只读展示；DevBox 生成的密钥保存在应用数据目录的 `keys/`。
- **凭证**：密码 / 私钥路径 / 口令保存在本机配置文件（权限 `0600`）。私钥路径支持 `~/` 展开。
- **转发**：
  - 本地转发：`本地地址:端口 → 远端地址:端口`
  - SOCKS5：本地监听一个 SOCKS5 代理，流量经 SSH 服务器动态转发
  - HTTP：本地监听 HTTP 代理，支持 CONNECT 隧道；可自动设置为 macOS 系统代理
  - 本地端口填 `0` 时自动分配可用端口
  - 断线自动重连，应用重启后自动恢复启用的规则

## 快捷键

| 快捷键 | 功能 |
| --- | --- |
| `Alt+Space` | 全局唤出 / 收起命令面板（主窗口关闭时自动重建并置前） |
| `⌘K` | 在窗口内打开命令面板（搜索工具） |
| `Esc` | 关闭面板 / 取消表单 / 关闭确认框 |
| `⌘Q` | 退出 |
| `⌘1` … `⌘6` | JSON、Base64、时间戳、UUID、Hash、Hosts 环境 |
| `⌘A` / `⌘C` / `⌘V` / `⌘X` | 文本编辑器全选、复制、粘贴、剪切 |
| `←` `→` `↑` `↓` / `Home` / `End` | 光标移动（`Shift` 选择） |

点击主窗口关闭按钮只关闭窗口，应用继续驻留；点击 Dock 图标或按 `Alt+Space` 可恢复，并自动回到上次使用的工具页。`⌘Q` 才会退出应用并停止代理 / SSH 服务。

> **提示**：`Alt+Space` 全局快捷键需要在 macOS「系统设置 → 隐私与安全性 → 辅助功能」中给应用授权。若注册失败（终端会打印提示），仍可用窗口内的 `⌘K` 打开命令面板。

## 配置与数据

所有数据位于 `~/Library/Application Support/DevBox/`：

| 文件 / 目录 | 内容 |
| --- | --- |
| `api_proxy.json` | 接口转发规则与当前激活环境 |
| `hosts_profiles.json` | Hosts 环境规则与当前生效环境 |
| `ssh_profiles.json` | SSH 凭证 |
| `ssh_forwards.json` | SSH 转发规则 |
| `ui.json` | 主题、隐藏工具等界面设置 |
| `keys/` | DevBox 生成的 SSH 密钥对 |

与原 Go 版本兼容：

- 转发规则中的 `profileID`（以及新写出的 `profileId`）都能读取；
- 旧版 `authType: "private_key"` 会自动按 `key` 处理；
- 其余字段保持 camelCase 命名。

## 打包成桌面应用

图标资源在 `resources/`：`AppIcon.png`（1024×1024 源图）、`AppIcon.icns`（macOS）、`AppIcon.ico`（Windows）。可用 `python3 scripts/make_icons.py` 重新生成 PNG/ICO，`./scripts/make_icns.sh` 重新生成 ICNS。

### macOS `.app`

```bash
./scripts/build-macos.sh
```

脚本会构建 release 版本，生成 `dist/DevBox.app`（含 `Info.plist`、图标、ad-hoc 签名），双击即可运行。首次从网上下载后如被系统拦截，执行 `xattr -dr com.apple.quarantine dist/DevBox.app`。

### Windows 应用程序

在 **Windows 本机**（x64 Native Tools Command Prompt for VS，需 MSVC 与 `windres`）执行：

```powershell
.\scripts\build-windows.ps1
```

会生成 `dist/DevBox-windows/devbox.exe` 和 `dist/DevBox-windows.zip`。`build.rs` 会把 `AppIcon.ico` 嵌入 exe，任务栏和文件管理器会显示图标。

> 注意：Windows 构建建议在本机完成。在 macOS 上做 MSVC 交叉编译时，`ring` 的构建脚本需要 Windows C 工具链，通常会失败；这不是代码问题，改用 Windows 本机构建即可。
> Windows 下文件选择器使用 PowerShell/WinForms 实现；系统代理设置仍仅支持 macOS。

## 项目结构

```
src/
├── main.rs            # GPUI 入口、Tokio 运行时、快捷键
├── proxy.rs           # 接口反向代理（hyper + reqwest + WebSocket 透传）
├── ssh.rs             # SSH 凭证 / 密钥 / 端口转发（russh）
├── store.rs           # JSON 持久化与路径
├── util.rs            # 通用小工具
├── theme.rs           # 明暗主题设计令牌
└── ui/
    ├── app.rs         # 根视图、侧边栏、命令面板、确认框、工具路由
    ├── text_input.rs  # 自绘多行 / 单行文本编辑器（IME、选区、剪贴板）
    ├── widgets.rs     # 按钮、卡片、徽标、开关等组件
    └── pages/         # 13 个工具页面
```

代理与 SSH 服务通过一个独立的 Tokio 多线程运行时驱动，GPUI 界面同步读取服务状态；启动时会恢复已启用的代理规则和转发隧道。

## 许可证

本项目采用 [Apache License 2.0](LICENSE) 许可证。
