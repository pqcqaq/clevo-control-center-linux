# 进度日志

## 当前恢复入口

换会话后先读 `AGENT_HANDOFF.md`。该文件记录当前代码结构、最新提交、验证命令、实机 GPU MUX 读回值、安全边界和暂不公开项。

## 会话：2026-07-10

### 阶段：控制中心能力对齐与实机确认
- **状态：** complete
- 执行的操作：
  - 迁移到 `clevo-control-center-linux` 项目和 `clevo-control-center` 命名，移除旧 `clevo-keyboard-led` 兼容命令和打包残留。
  - 按用户要求重构 UI：左侧 sidebar 为唯一导航，去掉顶部 tab、蓝色斜线背景、首页裸固件信息和重复按钮。
  - 修复启动后默认状态读取，GUI 从服务/只读 DCHU 状态拿数据，不依赖手动刷新和 sudo。
  - 首页改为风扇仪表盘 + 灯光摘要 + 电源模式 + 风扇模式；CPU/GPU 两路默认显示，第三路 tach 有数据时显示 PCH。
  - 参考原厂和 `clevo-indicator` 修正风扇 tach：`RPM = 2156220 / raw_tach`。
  - 从 EC 状态 buffer 解析温度，首页展示确认度高的 CPU/GPU 温度，高级页展示所有温度样字段。
  - 对齐原厂电源/风扇模式读写：AppSettings `1:1` 读电源模式，`4:5` 读风扇模式；Linux 只做这两个字段的受限运行时镜像。
  - 新增“风扇”页面，本地编辑三组 CPU/GPU 自定义曲线；总览页选择曲线时才通过 `fan-curve` 写 EC 并切换 custom。
  - 新增“电池”页面，目前只保存本地策略配置，不写 EC。
  - 去掉“性能”侧边栏页面，性能/电源模式保留在首页。
  - 逆向原厂 FnKey/ControlCenter/FanSpeedSetting，记录 GPU MUX、风扇曲线、能力位、GPU OC、Battery Saver 等链路到 `findings.md`。
  - 新增 `/proc/clevo_dchu_config` 的只读 GPU MUX 回读：`WMI4/sub8` capability 和 `WMI4/sub21` status/options。
  - 实机确认本机不走旧 `PSF2 bit20` MUX，而走新四状态 `WMI4/sub8 offset18 bit0`。
  - 新增 `AGENT_HANDOFF.md` 作为后续会话恢复手册。
- 创建/修改的关键文件：
  - `AGENT_HANDOFF.md`
  - `README.md`
  - `DCHU_ADJUSTMENTS.md`
  - `findings.md`
  - `module/clevo_control_center.c`
  - `src/dchu.rs`
  - `src/ui/advanced.rs`
  - `src/ui/pages.rs`
  - `src/ui/fan.rs`
  - `src/ui/battery.rs`
  - `src/fan_curve.rs`
  - `src/battery_strategy.rs`
- 最近已推送提交：
  - `e029ef1 Expose GPU MUX capability readback`
  - `67c1c22 Apply custom fan curves through DCHU`
  - `e8e95d7 Disable unchanged fan curve actions`
  - `b994bd6 Stabilize fan curve action layout`
  - `d9482b8 Clean up fan curve page layout`
  - `f9ea343 Add local battery strategy page`
- 2026-07-10 实机 GPU MUX 读回：
  - `psf2_7a = 0x70020053`
  - `bios_feature_04_08_version = 0x0100`
  - `bios_feature_04_08_offset18 = 0x4d`
  - `gpu_mux_04_15_current = 0x02`
  - `gpu_mux_04_15_options = 0x06`
- 验证：
  - Windows 本地 `cargo fmt --check` 通过。
  - Windows 本地 `cargo check` 通过。
  - Windows 本地 `cargo test` 通过。
  - Linux 笔记本 `cargo check` 通过。
  - Linux 笔记本 `cargo test` 通过。
  - Linux 笔记本 `make -C module` 通过。
  - Linux 笔记本加载新模块后 `/proc/clevo_dchu_config` 读回 MUX 数据成功。
- 安全边界：
  - `/proc/clevo_dchu_control` 只接受 `fan-mode`、`power-mode`、`fan-curve`。
  - 不公开 MUX/GPU OC/CPU OC/Battery Saver/EnergySave/AntiDust 写入口。
  - 不记录任何密码或 token。

## 会话：2026-07-08

### 阶段 1：需求与发现
- **状态：** complete
- 执行的操作：
  - 读取 Windows C# 源码 `Form1.cs`
  - 确认颜色设置调用为 `SetDCHU_Data(103, bytes, 4)`
  - 反汇编 `InsydeDCHU.dll`
  - 提取 Windows 设备接口 GUID、`_DSM` GUID 和 IOCTL
  - 通过 SSH 检查 Linux 笔记本硬件/DMI/WMI/sysfs 状态
  - 以 root 权限搜索 ACPI 表，确认 DSDT 中存在 `_DSM` GUID
- 创建/修改的文件：
  - `task_plan.md`
  - `findings.md`
  - `progress.md`

### 阶段 2：Linux 接口定位
- **状态：** complete
- 执行的操作：
  - 已确认 Linux 没有现成 Clevo/Tuxedo 键盘灯 sysfs 接口
  - 安装了 `acpica-tools`
  - 导出并反编译了 DSDT
  - 定位到设备 `\_SB.DCHU`，`_HID` 为 `CLV0001`
  - 确认 `SCMD(0x67)` 是键盘 RGB 设置路径

### 阶段 3：实现
- **状态：** complete
- 执行的操作：
  - 检查了 `acpi_call-dkms` 源码和 README，确认输入端不能构造 package
  - 决定实现最小外部内核模块
  - 创建 `module/clevo_kbd_led.c`
  - 创建 `module/Makefile`
  - 创建 `README.md`

### 阶段 4：测试与验证
- **状态：** complete
- 执行的操作：
  - 同步项目到 `/home/qcqcqc/clevo-keyboard-led-linux`
  - 在内核 `7.0.12+kali-amd64` 上编译成功
  - 加载模块成功，`/proc/clevo_kbd_led` 出现
  - 测试 `f0 ff0000` 成功
  - 测试三段红/绿/蓝成功
  - 最终写入 `ffffff` 全白成功

## 测试结果
| 测试 | 输入 | 预期结果 | 实际结果 | 状态 |
|------|------|---------|---------|------|
| Windows 源码静态分析 | `Form1.cs` | 找到 DLL 调用方式 | 成功 | pass |
| DLL 导出和反汇编 | `InsydeDCHU.dll` | 找到 GUID/IOCTL | 成功 | pass |
| Linux sysfs/WMI 检查 | 远端工作机 | 判断是否有现成接口 | 未发现现成键盘灯接口 | pass |
| ACPI GUID 搜索 | DSDT | 找到 `_DSM` GUID | DSDT 偏移 `443606` 命中 | pass |
| DSDT 反编译 | `acpica-tools` | 定位 `_DSM` 路径 | `\_SB.DCHU._DSM` | pass |
| `acpi_call` 可用性评估 | 包源码 | 判断是否能传 package | 不支持 package 输入 | pass |
| 内核模块编译 | `make` | 生成 `.ko` | `clevo_kbd_led.ko` 成功生成 | pass |
| 模块加载 | `insmod` | `/proc/clevo_kbd_led` 出现 | 成功 | pass |
| 单区写入 | `f0 ff0000` | ACPI 返回成功 | dmesg 记录成功 | pass |
| 三区写入 | `f0/f1/f2` 红绿蓝 | ACPI 返回成功 | dmesg 三段均成功 | pass |
| 全区写入 | `ffffff` | 三段全白 | dmesg 三段均成功 | pass |

## 错误日志
| 时间戳 | 错误 | 尝试次数 | 解决方案 |
|--------|------|---------|---------|
| 2026-07-08 | PowerShell 展开远端 Bash 表达式导致命令污染 | 2 | 改用模板占位符和 base64 |
| 2026-07-08 | 普通用户读取 ACPI 表权限不足 | 1 | 改用 sudo |
| 2026-07-08 | 远端 here-doc 混入 CRLF 导致 `sed` 读取 `dsdt.dsl\r` | 2 | 改用单行 SSH 命令或清理 CRLF |

## 五问重启检查
| 问题 | 答案 |
|------|------|
| 我在哪里？ | 阶段 5：交付 |
| 我要去哪里？ | 向用户说明路径、用法、测试结果和非持久安装状态 |
| 目标是什么？ | 在 Linux 笔记本上实现键盘 RGB 设置工具 |
| 我学到了什么？ | 见 `findings.md` |
| 我做了什么？ | 见上方记录 |
