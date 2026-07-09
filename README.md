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

`/proc/clevo_dchu_status` 是只读状态接口，默认权限为 `0444`，GUI 和后台服务用它读取风扇转速、CPU/GPU 温度等硬件状态；RPM3 有数据时总览会额外显示 PCH 风扇。

`/proc/clevo_dchu_control` 是白名单控制接口，默认权限为 `0666`，GUI 用它写入已确认的 `fan-mode` 和 `power-mode` 命令。它不接受任意 DCHU function 或裸数据。

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
  dchu.rs                     DCHU proc 读写、CLI 输出和硬件状态解析
  effects.rs                  动态灯效颜色计算
  led.rs                      键盘灯 proc 写入
  model.rs                    页面、灯效、颜色和分区领域模型
  service.rs                  后台灯效服务、PID/lock 和硬件状态缓存
  settings.rs                 配置路径、迁移、读写和硬件状态缓存文件
  ui.rs                       egui 应用状态和页面渲染
```

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
echo 'fan-mode turbo' > /proc/clevo_dchu_control
echo 'power-mode 2' > /proc/clevo_dchu_control
```

`/proc/clevo_dchu_control` 只接受 `fan-mode <auto|max|silent|maxq|custom|turbo|0|1|3|5|6|7>` 和 `power-mode <0..3>`。其他命令、额外参数和越界值会被内核模块拒绝。

## DCHU CLI

```bash
target/release/clevo-control-center dchu status
target/release/clevo-control-center dchu fan-mode auto --i-understand
target/release/clevo-control-center dchu power-mode 2 --i-understand
```

`dchu status` 读取 `/proc/clevo_dchu_status`，通常不需要 root。`fan-mode` 和 `power-mode` 写入 `/proc/clevo_dchu_control`，普通用户可用。CLI 不再提供裸 DCHU 调试入口。

## GUI 和后台服务

启动 GUI：

```bash
scripts/run-gui.sh
```

GUI 页面：

- 总览：灯效摘要、CPU/GPU 风扇转速和温度；RPM3 有数据时额外显示 PCH 风扇
- 灯光：键盘 RGB 色块、灯效模式、速度和亮度
- 性能：DCHU 电源模式和风扇模式按钮
- 诊断：读取 DCHU 只读状态
- 设置：选择 `f0-f6` 生效分区，并查看硬件读回摘要

自定义模式下启动按钮、速度、亮度不可用；选色后会直接写入当前选中的分区。默认分区为 `f0-f2`。

普通启动 GUI 时，程序会自动拉起后台服务。后台服务通过固定目录中的 `clevo-control-center.lock` 和 `clevo-control-center.pid` 保持单例，并持续读取 `settings.json` 执行动态灯效，因此关闭 GUI 后灯效仍会继续。后台服务还会定期读取硬件状态并写入 runtime 缓存，GUI 打开后直接显示最近一次状态。

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
