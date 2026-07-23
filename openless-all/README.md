# ZeroType

> 智能语音输入法 · 本地优先 · 热词自学习 · 声纹识别 · 用户计费

ZeroType 是一款基于 [Tauri 2](https://tauri.app/) + Rust + React/TypeScript 构建的跨平台智能语音输入法。它从 [OpenLess](https://github.com/Open-Less/openless) 二次开发而来，在保留其本地 ASR 语音输入能力的基础上，新增了**热词自学习引擎、用户认证与计费系统、声纹识别**等模块，面向内容创作者与团队提供“越用越准”的语音写作体验。

## ✨ 核心特性

- **本地 ASR 语音输入**：基于 vendored 的 `qwen-asr` 引擎（fork 自 `antirez/qwen-asr`），语音在本地完成识别，数据不出本机。
- **热词自学习引擎（Hotword Learning）**：自动从用户输入中提取 n-gram 候选词，按「频次 ×0.6 + 新鲜度 ×0.3 + 唯一性 ×0.1」综合评分；评分 ≥ 0.75 自动晋升为热词，每 7 天衰减 ×0.9，越用越贴合个人用语习惯。
- **用户系统与计费（Auth & Billing）**：bcrypt 密码哈希 + UUID 会话（30 天有效期）+ 本地 JSON 存储；**免费额度每周 10,000 字**，于每周一自动重置。
- **声纹识别（Voiceprint）**：提取 8 维声学特征（RMS / ZCR / 频谱质心 / 频谱通量等），基于余弦相似度（阈值 0.75）做说话人校验；接口预留 sherpa-onnx 升级路径。
- **多语言界面**：内置 简体中文 / 繁体中文 / English / 日本語 / 한국어 五种语言。
- **隐私优先**：识别与用户数据默认存储于本地，不上传云端。

## 🧱 技术架构

| 层 | 技术 |
| --- | --- |
| 桌面壳 | Tauri 2 |
| 后端 | Rust（coordinator 调度、ASR、热词/认证/计费/声纹模块） |
| 前端 | React 18 + TypeScript + Vite + Tailwind CSS |
| 本地 ASR | qwen-asr（vendored C 引擎） |
| 国际化 | i18next（5 语言） |

> 最低系统要求：macOS 12.0+（开发环境）；Windows / Linux 构建脚本亦保留。

## 📁 目录结构

```
zerotype/
└── openless-all/              # 工作区根目录
    ├── app/                   # 可运行的 Tauri 应用
    │   ├── src/               # React 前端（components / pages / i18n / lib）
    │   ├── src-tauri/         # Rust 后端
    │   │   ├── src/
    │   │   │   ├── hotword/     # 热词自学习引擎
    │   │   │   ├── auth/        # 用户认证
    │   │   │   ├── billing/     # 计费配额
    │   │   │   ├── voiceprint/  # 声纹识别
    │   │   │   ├── commands/    # IPC 命令注册
    │   │   │   └── coordinator.rs  # 会话调度核心
    │   │   └── vendor/qwen-asr/  # vendored 本地 ASR 引擎
    │   └── scripts/           # 构建/测试脚本
    └── scripts/               # 工作区级脚本
```

## 🚀 快速开始

### 环境要求

- Rust（stable）及 `cargo`
- Node.js 22+
- npm
- macOS：Xcode Command Line Tools（`xcode-select --install`）

### 本地开发

```bash
# 1. 克隆仓库
git clone https://github.com/callaaron/zerotype.git
cd zerotype/openless-all

# 2. 安装前端依赖
cd app
npm ci

# 3. 启动开发模式（自动拉起 Rust 后端 + Vite 前端）
npm run tauri dev
```

> 注：`qwen-asr` 已作为 vendored 源码随仓库提供，**无需**执行 `git submodule update`。首次构建会自动编译该引擎。

### 构建分发

```bash
cd app
npm run tauri build
```

产物位置：

- macOS：`app/src-tauri/target/release/bundle/macos/ZeroType.app`
- Windows / Linux：对应 `bundle/` 目录下安装包

macOS 本地安装（开发期）：

```bash
cd app
./scripts/build-mac.sh
```

## ⚙️ 配置与使用

### 本地 ASR 模型

语音识别依赖本地 ASR 模型权重。请在应用 **设置 → 语音/模型** 中手动下载并启用模型文件；未下载前应用可正常启动，但暂无法进行语音识别。

### 免费额度与计费

- 默认账户拥有 **每周 10,000 字** 的免费识别额度。
- 额度于**每周一 00:00** 自动重置。
- 超额后需登录账户并升级套餐（套餐与云端接入详见路线图）。

### 热词自学习

- 在任意输入框使用语音输入后，引擎会在会话结束时自动分析文本。
- 高频、近期出现、且具个人特异性的词组会被推荐为热词，可在 **设置 → 热词** 中查看与管理。
- 评分 ≥ 0.75 的词组自动晋升；长期未使用的热词会按 7 天 ×0.9 衰减。

### 声纹识别

- 在 **设置 → 声纹** 中录入声纹样本。
- 后续语音输入可基于余弦相似度（阈值 0.75）校验说话人，用于多用户场景下的个性化。

## 🔐 隐私说明

ZeroType 采用 **本地优先（local-first）** 设计：

- 语音识别在本地完成，音频不上传云端。
- 用户账户、热词、声纹等数据默认存储于本机文件系统（JSON）。
- 自动更新（updater）当前未启用；后续若启用会在应用内明确提示。

## 🔁 与 OpenLess 的关系 / 主要改动

ZeroType 是 [OpenLess](https://github.com/Open-Less/openless) 的衍生项目（fork），主要新增与变更：

- 品牌重塑：应用名 `ZeroType`、标识符 `com.finelab.zerotype`、窗口标题与关于页全面替换。
- 新增模块：`hotword`（热词自学习）、`auth`（认证）、`billing`（计费）、`voiceprint`（声纹）。
- 新增 `auth` / `billing` / `voiceprint` 模块对应的 IPC 命令，并完成前端绑定与界面（登录/注册弹窗、账户页、声纹管理）。
- 补全 简体中文 / 繁体中文 / English / 日本語 / 한국어 五语言 i18n 键值。
- 重写关于页与自动更新提示，移除原项目社群入口。

上游仓库保留为 `upstream` remote，便于后续同步 OpenLess 的修复。

## 🗺️ 路线图（Roadmap）

- [ ] 云端 ASR / DeepSeek 接入，作为本地引擎的补充
- [ ] 多套餐计费与支付闭环
- [ ] 声纹引擎升级至 sherpa-onnx
- [ ] Windows / Linux 正式发布包
- [ ] 团队协作与热词共享

## 📄 许可证

本项目基于 OpenLess（MIT License）二次开发，遵循 **MIT License**。
版权归 FINELAB / 小可实验室（ZeroType）与原 OpenLess 作者共同所有，详见 [LICENSE](./LICENSE)。

## 🙏 致谢

- [OpenLess](https://github.com/Open-Less/openless) — 本地优先的语音输入法，本项目的起点。
- [qwen-asr](https://github.com/antirez/qwen-asr)（via Open-Less）— 本地 ASR 引擎。
