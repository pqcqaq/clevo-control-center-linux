# 贡献指南

感谢你考虑为 Clevo Control Center for Linux 做贡献。本项目会直接调用笔记本固件和 EC，代码是否“能编译”不是硬件写入功能合入的充分条件。请优先保证行为可解释、失败可恢复，并清楚标注验证边界。

## 开始之前

- 功能建议、兼容性报告和普通缺陷请先提交 GitHub Issue。
- 安全漏洞不要公开提交 Issue，请按 [SECURITY.md](SECURITY.md) 披露。
- 对 DCHU、EC、风扇、MUX 或灯光协议的修改，先说明证据来源和目标机型；不接受仅凭猜测增加的裸写入口。
- 本项目与 Clevo、蓝天电脑及其品牌商没有隶属、授权或担保关系。

## 本地开发

克隆仓库后先检查环境：

```bash
git clone https://github.com/pqcqaq/clevo-control-center-linux.git
cd clevo-control-center-linux
scripts/check-env.sh
```

构建 Rust 程序和当前内核对应的模块：

```bash
scripts/build.sh
```

Rust 提交门禁：

```bash
cargo fmt --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Linux 内核模块门禁：

```bash
make -B -C module W=1
```

构建完整 Release 资产：

```bash
scripts/package-release.sh
```

该命令要求干净 Git 工作区和 Docker。二进制在 Debian Bullseye/glibc 2.31 基线中构建，并按打包机现有工具生成 source、tar.gz、deb、rpm、Arch 包、资产说明和 SHA-256 校验文件。`--skip-checks` 只用于同一提交已经在其他环境完成完整门禁的打包机。

内核模块必须在目标 Linux 内核的 headers/Kbuild 环境中检查。Windows 编辑器无法解析 `linux/*.h` 不代表模块源码存在语法错误。

## 代码边界

- GUI、后台服务和 CLI 通过 `HardwareBackend` 使用硬件能力，不要把 `/proc` 文本协议散落到 UI 代码。
- 按业务职责拆分模块，不增加只包装少量代码的 `helpers.rs`、`utils.rs` 或 `common.rs`。
- 内核模块只保留经过验证的白名单命令；不要加入任意 function、offset 或 raw payload 入口。
- 电池页只读取已确认的 WMI7/OEM 状态，并通过受能力位保护的 `battery-saver on/off` 白名单写入；不要绕过能力检查，也不要在缺少状态读回和恢复方案时开放 EnergySave 阈值、Battery Utility 刷新或任意 WMI/EC payload。
- `Diagnostics` 和 `Advanced` 仅存在于 debug 构建，Release UI 不应暴露内部诊断入口。
- 不要为消除警告大范围加入 `allow`，应处理实际类型、所有权或条件编译问题。

## 硬件改动需要的证据

涉及固件写入的 Pull Request 至少应包含：

1. 目标机器的 DMI `sys_vendor`、`product_name`、BIOS 版本和相关能力位。
2. 协议来源，例如原厂程序反编译、ACPI/WMI 定义或可重复的只读观测。
3. 写入前后的可观察结果，以及恢复到安全默认状态的方法。
4. 用户态测试、Release 构建和 `make -B -C module W=1` 结果。
5. 新增协议语义的校验测试；危险输入必须同时在用户态和内核态拒绝。

不要在 Issue 或日志中上传机器序列号、资产编号、账户信息或未经清理的完整固件转储。

## 提交与 Pull Request

- 从最新 `master` 创建主题分支。
- 每个提交只处理一个可说明的主题，提交标题使用仓库现有的简短祈使句风格，例如 `Document release packaging workflow`。
- 不提交 `target/`、`dist/`、内核构建产物、运行日志、个人设置或本地协作记录。
- Pull Request 描述应说明改了什么、为什么这样改、如何验证，以及仍未覆盖的硬件风险。
- UI 改动请附 Release 构建截图；不要用 Debug 导航截图代替正式产品状态。

提交即表示你同意按项目的 [GPL-2.0-only](LICENSE) 许可证发布你的贡献。
