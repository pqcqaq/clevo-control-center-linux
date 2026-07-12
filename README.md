# 蓝天控制中心 Linux 版

这是一个给蓝天/Clevo/Insyde DCHU 方案笔记本使用的 Linux 控制中心，提供图形界面、后台服务、键盘 RGB 控制、受限 DCHU 控制入口、内核模块和安装包构建脚本。

项目由两部分组成：

- `module/`：最小 Linux 内核模块，负责调用 ACPI `_DSM`，并暴露键盘灯、只读硬件状态和白名单 DCHU 控制 proc 节点
- `src/`：Rust 程序，同一个二进制同时提供前台 GUI、后台服务和 DCHU 测试 CLI

GUI 只负责修改配置和启动后台服务；动态灯效由后台服务持续执行，所以关闭 GUI 后灯效不会停止。后台服务通过固定 runtime 目录中的锁文件保持单例，多个 GUI 窗口共享同一个配置状态。

## 硬件接口

ACPI 路径：

- 设备：`\_SB.DCHU`
- 方法：`_DSM`
- UUID：`93f224e4-fbdc-4bbf-add6-db71bdc0afad`
- 键盘灯 Function：`0x67`

用户态通过 `/proc/clevo_control_center_led` 写入颜色。内核模块接收普通 `RRGGBB` 或 `zone RRGGBB` 输入，并转换成固件需要的数据。

`/proc/clevo_dchu_status` 是只读状态接口，默认权限为 `0444`，GUI 和后台服务用它读取风扇 tach 计数、CPU/GPU 温度等硬件状态；tach 会按 Clevo EC 公式换算成 RPM，第三路 tach 有数据时总览会额外显示 PCH 风扇。debug 构建的“高级”页面会保留并展示 DCHU 0x0C 原始 buffer、风扇 raw/解析值、温度块和其他非零字段。

`/proc/clevo_dchu_config` 是只读配置/能力接口，默认权限为 `0444`，返回 DCHU 0x0D 配置 buffer、`PSF1/PSF2/PSF4/PSF5` 能力整数、GPU MUX 新接口 capability/status/options，以及受限 AppSettings 兼容层里的电源/风扇模式读回。GUI 会按原厂能力位决定控制项是否可见：电源模式看 `PSF5 bit0`，风扇设置看 `PSF5 bit7`，Silent 看 `PSF2 bit15`，MaxQ 看 `0x0D[0x0E] == 5`。MUX、超频、电池策略等能力只在 debug 构建的“高级”页面只读展示，不作为写入控件公开。

`/proc/clevo_dchu_control` 是白名单控制接口，默认权限为 `0666`，GUI 用它写入已确认的 `fan-mode`、`power-mode` 和 `fan-curve` 命令。它会按原厂顺序同步受限 AppSettings 状态，但不接受任意 DCHU function、任意 AppSettings offset 或裸数据。

## 目录结构

```text
app/
  clevo-control-center.desktop 桌面启动器
  run-clevo-control-center.sh 桌面启动器调用的脚本
module/
  clevo_control_center.c      内核模块源码
  Makefile                    内核模块构建入口
scripts/
  check-env.sh                环境和依赖检查
  build.sh                    构建内核模块和 Rust 程序
  package-tar.sh              生成通用 Linux tar.gz 安装包
  package-deb.sh              生成 Debian/Ubuntu deb 安装包
  run-gui.sh                  启动 GUI
  run-service.sh              手动启动后台服务
  stop-service.sh             停止后台服务
packaging/
  install.sh                  通用包安装脚本
  deb/                        Debian 包控制文件和安装钩子
src/
  main.rs                     程序入口、CLI 分发和 GUI 启动
  dchu.rs                     DCHU 领域类型、能力位和可用模式
  dchu/
    io.rs                     proc I/O、响应解析和硬件快照解码
    cli.rs                    受保护写入、参数校验和 CLI 分发
    tests.rs                  DCHU 能力、解析和控制边界测试
  hardware.rs                 硬件后端契约和原生后端工厂
  hardware/
    linux.rs                  Linux 灯光与 DCHU 后端实现
  effects.rs                  后台软件灯效帧生成与时间轴
  model.rs                    页面、灯效、颜色和分区领域模型
  module_loader.rs            GUI 内核模块版本检查和认证加载
  preferences.rs              语言偏好、系统语言识别和主题色模型
  service.rs                  后台灯效服务、PID/lock 和硬件状态缓存
  settings.rs                 配置路径、迁移、读写和硬件状态缓存文件
  battery_strategy.rs         本地电池策略配置模型和校验
  fan_curve.rs                本地风扇曲线配置模型和校验
  ui/
    app.rs                    应用状态和用户操作
    app/                      设置/硬件同步与窗口生命周期
    pages.rs                  页面分发和 debug 诊断页
    pages/                    总览、灯光、显卡和设置业务页面
    advanced.rs               debug-only DCHU 高级只读信息解释
    fan.rs                    风扇曲线编辑页
    battery.rs                本地电池策略页
    fan_gauge.rs              风扇仪表盘组件和绘制
    color_picker.rs           Linux 原生调色盘调用和结果解析
    layout.rs                 侧边栏与主区域布局
    theme.rs                  全局交互强调色调色板和 egui 主题应用
    widgets.rs                跨页面基础控件和字体安装
```

模块按业务职责划分，不使用 `helpers.rs`、`utils.rs` 或 `common.rs` 收纳零散逻辑。页面专属绘制与交互保留在所属页面；只有具备独立状态、平台边界或测试职责的组件才单独成模块。

GUI、后台服务和 DCHU CLI 只通过 `HardwareBackend` 执行硬件操作。当前 `native_backend()` 只返回 Linux 实现；`/proc` 路径、灯光命令序列化和 DCHU 控制文本不会进入 UI 或服务业务代码。该边界为后续平台实现预留，但当前项目不包含 Windows 后端或 DLL 调用。

## 环境要求

需要以下依赖：

- Rust：`cargo`、`rustc`
- 当前内核对应的构建头文件：`/lib/modules/$(uname -r)/build`
- `make`
- `pkexec`
- `zenity` 或 `kdialog`，用于弹出系统调色盘

检查环境：

```bash
scripts/check-env.sh
```

## 构建

一键构建：

```bash
scripts/build.sh
```

手动构建：

```bash
make -C module
cargo build --release
```

## 代码质量与 VS Code

提交前建议运行完整 Rust 检查：

```bash
cargo fmt --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

仓库提供共享 VS Code 配置：rust-analyzer 后台使用 `cargo check` 并覆盖全部 target 和 feature。没有强制保存时格式化或后台 Clippy，因为部分发行版会把 `rustfmt`、`clippy` 拆成未默认安装的独立软件包；安装对应组件后再手动运行上面的完整门禁。Pedantic/Nursery lint 不作为默认门禁，因为其中包含大量 `const fn`、浮点测试和 UI 数值转换等风格建议。

`module/clevo_control_center.c` 是 Linux 内核模块，必须依赖目标机器当前内核的 Kbuild 头文件和生成配置。直接在 Windows 本地用 C/C++ 扩展打开时，`linux/*.h` 缺失及相关宏错误属于解析环境误报。检查该文件应通过 VS Code Remote SSH 打开 Linux 项目目录，并以以下命令为准：

```bash
make -B -C module W=1
```

如果只在 Windows 本地编辑 Rust，请不要为了消除内核 C 文件的红线而添加假的 Windows include path 或关闭整个工作区的诊断。

构建完成后的主程序：

```text
target/release/clevo-control-center
```

## 打包和安装

通用 Linux 包：

```bash
scripts/package-tar.sh
```

输出示例：

```text
dist/clevo-control-center-0.1.0-linux-x86_64.tar.gz
```

安装通用包：

```bash
tar -xf dist/clevo-control-center-0.1.0-linux-x86_64.tar.gz -C /tmp
/tmp/clevo-control-center-0.1.0-linux-x86_64/install.sh
```

默认安装到：

- 程序目录：`~/.local/lib/clevo-control-center`
- 命令入口：`~/.local/bin/clevo-control-center`
- 桌面入口：`~/.local/share/applications/clevo-control-center.desktop`

卸载通用包：

```bash
/tmp/clevo-control-center-0.1.0-linux-x86_64/install.sh uninstall
```

Debian/Ubuntu 包：

```bash
scripts/package-deb.sh
sudo apt install ./dist/clevo-control-center_0.1.0_amd64.deb
```

`.deb` 会安装：

- `/usr/bin/clevo-control-center`
- `/usr/lib/clevo-control-center/`
- `/usr/share/applications/clevo-control-center.desktop`

内核模块不能跨内核通用分发。安装脚本和 `.deb` 会携带模块源码，并在目标机器存在当前内核 headers 时尝试本机编译和加载模块。

## 加载内核模块

```bash
sudo insmod module/clevo_control_center.ko
cat /proc/clevo_control_center_led
cat /proc/clevo_dchu_status
cat /proc/clevo_dchu_control
```

卸载：

```bash
sudo rmmod clevo_control_center
```

## /proc 控制接口

设置三个基础分区为同色：

```bash
echo ff0000 > /proc/clevo_control_center_led
```

设置单个分区：

```bash
echo 'f0 ff0000' > /proc/clevo_control_center_led
echo 'f1 00ff00' > /proc/clevo_control_center_led
echo 'f2 0000ff' > /proc/clevo_control_center_led
```

切换受限风扇和电源模式：

```bash
echo 'fan-mode auto' > /proc/clevo_dchu_control
echo 'fan-mode silent' > /proc/clevo_dchu_control
echo 'power-mode 2' > /proc/clevo_dchu_control
echo 'fan-curve 40:28,58:42,78:72,100:100 42:25,60:44,80:74,100:100' > /proc/clevo_dchu_control
```

`/proc/clevo_dchu_control` 只接受 `fan-mode <auto|max|silent|maxq|custom|0|1|3|5|6>`、`power-mode <0..3>` 和 `fan-curve <cpu> <gpu>`。`fan-curve` 的 CPU/GPU 参数各包含 4 个 `温度:占空比` 点，温度必须递增，占空比不能下降。原厂 Control Center 3.0 使用 `3` 作为静音风扇模式值；旧的 `2` 不再公开为有效模式。其他命令、额外参数、越界值和非法曲线会被内核模块拒绝。

## DCHU CLI

```bash
target/release/clevo-control-center dchu status
target/release/clevo-control-center dchu app-settings
target/release/clevo-control-center dchu fan-mode auto --i-understand
target/release/clevo-control-center dchu power-mode 2 --i-understand
target/release/clevo-control-center dchu fan-curve 40:28,58:42,78:72,100:100 42:25,60:44,80:74,100:100 --i-understand
```

`dchu status` 读取 `/proc/clevo_dchu_status`，`dchu app-settings` 读取受限 AppSettings 模式状态，通常不需要 root。`fan-mode`、`power-mode` 和 `fan-curve` 写入 `/proc/clevo_dchu_control`，普通用户可用。CLI 不再提供裸 DCHU 调试入口。

## GUI 和后台服务

启动 GUI：

```bash
scripts/run-gui.sh
```

GUI 页面：

- 总览：灯效摘要、CPU/GPU 风扇转速和温度；第三路风扇 tach 有数据时额外显示 PCH 风扇
- 灯光：键盘 RGB 色块、后台 60 FPS 软件灯效、能力感知分区和连续亮度
- 风扇：本地自定义风扇曲线开关、曲线 1/2/3 编辑、保存、重置和恢复
- 电池：本地电池策略开关、标准/保养/续航预设、充电阈值和低电量策略配置
- 诊断（仅 debug 构建）：读取 DCHU 只读状态
- 设置：切换界面语言与主题色、查看键盘灯类型和可用区域，并查看硬件读回摘要
- 高级（仅 debug 构建）：风扇 raw/解析值、温度块、受限 AppSettings 模式状态、GPU MUX 只读回读、官方能力位解析和其他 DCHU raw 状态

键盘类型直接读取 Clevo WMI13 的 `KBTP`：`2` 为三区 RGB，`6/22` 为单区 RGB15Color，逐键家族归为 Per-key。GUI 只展示硬件实际具备的用户区域；Logo 和灯条仅在各自能力位明确支持时出现，不把 `f0-f6` 固件命令码当作通用分区。

亮度百分比连续映射到固件值。循环、波浪、闪烁和呼吸由后台服务按 15ms deadline 生成软件帧，为固件偶发慢写保留余量并保证实际刷新率不低于 60 FPS；设置文件仍按 250ms 检查，硬件快照由独立工作线程每 2 秒更新。单区 RGB 每帧只写一次主区域，三区波浪才使用左、中、右空间相位；落后帧会跳过过期 deadline，避免累计漂移。GUI 只保存配置，所有灯光写入继续由后台服务串行完成。

“风扇”页中的自定义曲线保存到 `settings.json`。开启后，总览页的风扇模式行会额外显示 `曲线 1/2/3`；点击某条曲线时，程序会把对应 CPU/GPU 曲线转换成受限 `fan-curve` 命令写入 EC 风扇表，并把风扇模式切到 `custom`。曲线数据只以温度/占空比点传递，不暴露 EC raw payload。

“电池”页中的策略当前也只保存到 `settings.json`。页面可配置启用状态、预设、充电起止阈值和低电量相关策略意图；当前版本不写入 EC、不切换系统电源计划，也不调用原厂 Battery Saver/EnergySave 写接口。

普通启动 GUI 时，程序会自动拉起后台服务。后台服务通过固定目录中的 `clevo-control-center.lock` 和 `clevo-control-center.pid` 保持单例，并持续读取 `settings.json` 执行动态灯效，因此关闭 GUI 后灯效仍会继续。后台服务还会定期读取硬件状态并写入 runtime 缓存，GUI 打开后直接显示最近一次状态。

界面语言默认跟随 `LANGUAGE`、`LC_ALL`、`LC_MESSAGES`、`LANG` 中首个有效的系统 locale，也可以固定为简体中文或 English。主题默认保持原有琥珀色，可切换为青色、翠绿或玫红；风险、错误、电池健康和 GPU 模式等语义色不会随个性主题改变。

可以同时打开多个 GUI 窗口。每个窗口都会把操作写入同一个 `settings.json`，并自动读取其他窗口保存的设置变化。

手动启动后台服务：

```bash
scripts/run-service.sh
```

停止后台服务：

```bash
scripts/stop-service.sh
```

## 运行时文件

运行时文件使用固定 XDG 路径，不依赖启动时的工作目录：

- 配置：`${XDG_CONFIG_HOME:-~/.config}/clevo-control-center/settings.json`
- pid/lock：`${XDG_RUNTIME_DIR:-/tmp/clevo-control-center-$(id -u)}/clevo-control-center/`
- 日志：`${XDG_STATE_HOME:-~/.local/state}/clevo-control-center/clevo-control-center.service.log`

首次启动时，如果固定配置目录不存在 `settings.json`，GUI 会先显示硬件控制免责声明。用户明确确认并成功写入默认配置后，程序才会检查/加载内核模块并启动后台服务；退出、关闭窗口或配置保存失败都不会启动硬件写入服务。旧版配置成功迁移时视为已有配置，不重复显示免责声明。

首次启动时，如果固定配置目录还没有 `settings.json`，程序会尝试从旧的 `~/.config/clevo-keyboard-led/settings.json` 或当前目录的旧版 `settings.json` 复制一份过去。

## 桌面启动器

桌面文件：

```text
app/clevo-control-center.desktop
```

通用安装脚本和 `.deb` 会自动安装并刷新桌面入口。手动调试时也可以复制：

```bash
cp app/clevo-control-center.desktop ~/.local/share/applications/
update-desktop-database ~/.local/share/applications 2>/dev/null || true
```

## Cargo 镜像

项目内置 Cargo 镜像配置：

```text
.cargo/config.toml
```

当前使用 `rsproxy`，用于减少 crates.io 下载超时。

## 常见问题

GUI 能打开但灯不变：

```bash
ls -l /proc/clevo_control_center_led
```

确认模块已加载，且当前用户可写。

动态灯效停止：

```bash
scripts/run-service.sh
```

或者重新打开 GUI，GUI 会尝试自动启动后台服务。

调色盘打不开：

```bash
sudo apt install zenity
```

或者安装 `kdialog`。

内核模块构建失败：

确认安装了和当前内核匹配的 headers：

```bash
uname -r
ls /lib/modules/$(uname -r)/build
```
