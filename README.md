# Clevo/Insyde 键盘灯 Linux 控制器

这是一个给 Clevo/Insyde DCHU 方案笔记本使用的 Linux 键盘 RGB 控制器。

项目由两部分组成：

- `module/`：最小 Linux 内核模块，负责调用 ACPI `_DSM`，并暴露 `/proc/clevo_kbd_led`
- `src/`：Rust 程序，同一个二进制同时提供前台 GUI 和后台灯效服务

GUI 只负责修改配置和启动后台服务；动态灯效由后台服务持续执行，所以关闭 GUI 后灯效不会停止。

## 已验证的硬件调用

当前实现复刻 Windows `ColorfulLedKeyboardSet` 调用 `InsydeDCHU.dll` 的行为。

ACPI 路径：

- 设备：`\_SB.DCHU`
- 方法：`_DSM`
- UUID：`93f224e4-fbdc-4bbf-add6-db71bdc0afad`
- Function：`0x67`
- Payload：`Package(Buffer(0x100) { G, R, B, zone, ... })`

用户态 `/proc/clevo_kbd_led` 接口使用普通 `RRGGBB` 输入；内核模块内部会转换成固件需要的 `[G, R, B, zone]`。

## 目录结构

```text
app/
  clevo-keyboard-led.desktop  桌面启动器
  run-clevo-led-gui.sh        桌面启动器调用的脚本
module/
  clevo_kbd_led.c             内核模块源码
  Makefile                    内核模块构建入口
scripts/
  check-env.sh                环境和依赖检查
  build.sh                    构建内核模块和 Rust 程序
  run-gui.sh                  启动 GUI
  run-service.sh              手动启动后台服务
  stop-service.sh             停止后台服务
src/
  main.rs                     GUI、配置读写、后台服务实现
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
target/release/clevo-keyboard-led
```

## 加载内核模块

```bash
sudo insmod module/clevo_kbd_led.ko
cat /proc/clevo_kbd_led
```

卸载：

```bash
sudo rmmod clevo_kbd_led
```

## /proc 控制接口

设置三个基础分区为同色：

```bash
echo ff0000 | sudo tee /proc/clevo_kbd_led
```

设置单个分区：

```bash
echo 'f0 ff0000' | sudo tee /proc/clevo_kbd_led
echo 'f1 00ff00' | sudo tee /proc/clevo_kbd_led
echo 'f2 0000ff' | sudo tee /proc/clevo_kbd_led
```

## GUI 和后台服务

启动 GUI：

```bash
scripts/run-gui.sh
```

GUI 布局：

- 左侧：`f0` 圆形色块，自定义模式下点击可打开系统调色盘
- 中间：模式下拉框、速度滑块、亮度滑块
- 右侧：开始/结束按钮

自定义模式下开始按钮、速度、亮度不可用；选色后会直接写入 `f0`。

普通启动 GUI 时，程序会自动拉起后台服务。后台服务持续读取 `settings.json` 并执行动态灯效，因此关闭 GUI 后灯效仍会继续。

手动启动后台服务：

```bash
scripts/run-service.sh
```

停止后台服务：

```bash
scripts/stop-service.sh
```

## 运行时文件

以下文件会生成在项目运行目录，并已加入 `.gitignore`：

- `settings.json`：模式、速度、亮度、颜色、运行状态、窗口位置
- `clevo-keyboard-led.pid`：后台服务进程号
- `clevo-keyboard-led.service.log`：后台服务错误日志

## 桌面启动器

桌面文件：

```text
app/clevo-keyboard-led.desktop
```

安装或刷新桌面入口：

```bash
cp app/clevo-keyboard-led.desktop ~/.local/share/applications/
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
ls -l /proc/clevo_kbd_led
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
