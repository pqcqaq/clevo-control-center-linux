# Clevo/Insyde 键盘灯 Linux 控制器

这是一个给 Clevo/Insyde DCHU 方案笔记本使用的 Linux 键盘 RGB 控制器，提供图形界面、后台灯效服务、内核模块和安装包构建脚本。

项目由两部分组成：

- `module/`：最小 Linux 内核模块，负责调用 ACPI `_DSM`，并暴露 `/proc/clevo_kbd_led`
- `src/`：Rust 程序，同一个二进制同时提供前台 GUI 和后台灯效服务

GUI 只负责修改配置和启动后台服务；动态灯效由后台服务持续执行，所以关闭 GUI 后灯效不会停止。后台服务通过运行目录中的锁文件保持单例，多个 GUI 窗口共享同一个 `settings.json` 状态。

## 硬件接口

ACPI 路径：

- 设备：`\_SB.DCHU`
- 方法：`_DSM`
- UUID：`93f224e4-fbdc-4bbf-add6-db71bdc0afad`
- Function：`0x67`
- Payload：`Package(Buffer(0x100) { G, R, B, zone, ... })`

用户态通过 `/proc/clevo_kbd_led` 写入颜色。内核模块接收普通 `RRGGBB` 或 `zone RRGGBB` 输入，并转换成固件需要的 `[G, R, B, zone]` 数据。

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
  package-tar.sh              生成通用 Linux tar.gz 安装包
  package-deb.sh              生成 Debian/Ubuntu deb 安装包
  run-gui.sh                  启动 GUI
  run-service.sh              手动启动后台服务
  stop-service.sh             停止后台服务
packaging/
  install.sh                  通用包安装脚本
  deb/                        Debian 包控制文件和安装钩子
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

## 打包和安装

通用 Linux 包：

```bash
scripts/package-tar.sh
```

输出示例：

```text
dist/clevo-keyboard-led-0.1.0-linux-x86_64.tar.gz
```

安装通用包：

```bash
tar -xf dist/clevo-keyboard-led-0.1.0-linux-x86_64.tar.gz -C /tmp
/tmp/clevo-keyboard-led-0.1.0-linux-x86_64/install.sh
```

默认安装到：

- 程序目录：`~/.local/lib/clevo-keyboard-led`
- 命令入口：`~/.local/bin/clevo-keyboard-led`
- 桌面入口：`~/.local/share/applications/clevo-keyboard-led.desktop`

卸载通用包：

```bash
/tmp/clevo-keyboard-led-0.1.0-linux-x86_64/install.sh uninstall
```

Debian/Ubuntu 包：

```bash
scripts/package-deb.sh
sudo apt install ./dist/clevo-keyboard-led_0.1.0_amd64.deb
```

`.deb` 会安装：

- `/usr/bin/clevo-keyboard-led`
- `/usr/lib/clevo-keyboard-led/`
- `/usr/share/applications/clevo-keyboard-led.desktop`

内核模块不能跨内核通用分发。安装脚本和 `.deb` 会携带模块源码，并在目标机器存在当前内核 headers 时尝试本机编译和加载模块。

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

- 左上角：logo 菜单，打开后可进入设置窗口选择生效分区
- 左侧：圆形色块，自定义模式下点击可打开系统调色盘，并写入设置中选中的分区
- 中间：模式下拉框、速度滑块、亮度滑块
- 右侧：开始/结束按钮

自定义模式下开始按钮、速度、亮度不可用；选色后会直接写入当前选中的分区。默认分区为 `f0-f2`，设置窗口可选择 `f0-f6`。

普通启动 GUI 时，程序会自动拉起后台服务。后台服务通过 `clevo-keyboard-led.lock` 和 `clevo-keyboard-led.pid` 保持单例，并持续读取 `settings.json` 执行动态灯效，因此关闭 GUI 后灯效仍会继续。

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

以下文件会生成在项目运行目录，并已加入 `.gitignore`：

- `settings.json`：模式、速度、亮度、颜色、生效分区、运行状态、窗口位置
- `clevo-keyboard-led.pid`：后台服务进程号
- `clevo-keyboard-led.lock`：后台服务单例锁
- `clevo-keyboard-led.service.log`：后台服务错误日志

## 桌面启动器

桌面文件：

```text
app/clevo-keyboard-led.desktop
```

通用安装脚本和 `.deb` 会自动安装并刷新桌面入口。手动调试时也可以复制：

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
