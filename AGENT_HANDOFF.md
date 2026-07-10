# Agent Handoff

本文档用于换会话后快速恢复项目状态。先读本文，再按需读 `README.md`、`DCHU_ADJUSTMENTS.md` 和 `findings.md`。

## 安全规则

- 不要把密码、token、数据库配置或 SSH/Gitea 凭据写进仓库。
- 提交前必须执行 `git status --branch --short` 和 `git diff --cached --name-only`，确认暂存区没有敏感文件。
- 远端 Git URL 只允许保存用户名，不要把密码写进 `git remote`。
- 用户明确要求不要查看图片或截图；UI 结果交给用户肉眼检查。
- 不要开放裸 DCHU payload、任意 AppSettings offset 或任意 EC 写入口。

## 当前仓库状态

- 项目路径：`C:\Users\pqcmm\clevo-control-center-linux`
- Linux 笔记本路径：`/home/qcqcqc/clevo-control-center-linux`
- 当前主分支：`master`
- 当前已推送提交：`e029ef1 Expose GPU MUX capability readback`
- 本地和笔记本工作树在该提交后均验证为干净。

## 常用验证

Windows 本地：

```powershell
cargo fmt --check
cargo check
cargo test
git status --branch --short
```

Linux 笔记本：

```bash
cargo check
cargo test
make -C module
sed -n '/psf2_7a/p;/bios_feature_04_08/p;/gpu_mux_04_15/p;/app_power_mode/p;/app_fan_mode/p' /proc/clevo_dchu_config
```

远端曾确认 `cargo fmt` 子命令未安装；格式检查以 Windows 本地 `cargo fmt --check` 为准。

## 架构概要

- `module/clevo_control_center.c`：唯一内核桥接模块，创建 `/proc/clevo_control_center_led`、`/proc/clevo_dchu_status`、`/proc/clevo_dchu_config`、`/proc/clevo_dchu_app_settings`、`/proc/clevo_dchu_control`。
- `src/dchu.rs`：DCHU 状态/配置解析、能力位映射、白名单 CLI 写入、风扇 RPM 和温度解析。
- `src/ui/pages.rs`：总览页，显示风扇仪表盘、灯光摘要、电源模式和风扇模式。
- `src/ui/fan.rs` 与 `src/fan_curve.rs`：本地三组自定义风扇曲线编辑，只有在总览页选择曲线时才写 EC。
- `src/ui/battery.rs` 与 `src/battery_strategy.rs`：本地电池策略配置；当前不写 EC、不调用 Battery Saver/EnergySave。
- `src/ui/advanced.rs`：只读高级页，展示 raw、解析值、能力位和 MUX 回读。
- `findings.md`：原厂逆向结论。
- `DCHU_ADJUSTMENTS.md`：当前公开 DCHU 接口和安全边界。

## 当前公开接口

- 只读状态：`/proc/clevo_dchu_status`
- 只读配置/能力：`/proc/clevo_dchu_config`
- 只读受限 AppSettings：`/proc/clevo_dchu_app_settings`
- 键盘 RGB：`/proc/clevo_control_center_led`
- 白名单控制：`/proc/clevo_dchu_control`
- `/proc/clevo_dchu_control` 只接受 `fan-mode`、`power-mode`、`fan-curve`。

## UI 约束

- 左侧侧边栏是唯一导航，不要恢复顶部 tab。
- 首页不要显示裸 firmware/raw 信息，不要显示刷新转速、读回固件、灯光、性能等重复按钮。
- 首页主要结构：上部风扇仪表盘，下部灯光摘要、电源模式、风扇模式。
- 两路风扇默认显示 CPU/GPU；第三路 tach 非 0 时才显示 PCH。
- 风扇仪表盘使用指针式效果，RPM 公式是 `2156220 / raw_tach`，raw 越小真实 RPM 越高。
- 左侧菜单使用偏科技风格的平行四边形按钮和点击动效；按钮间距已按用户要求加大。
- 背景不要恢复蓝色斜线。

## 已确认 DCHU/EC 行为

- 风扇 tach raw 是周期计数，不是线性转速；显示 RPM 使用 `2156220 / raw_tach`。
- 温度来自 EC 状态 buffer，首页只展示确认度较高的 CPU/GPU 温度，高级页展示所有温度样字段。
- 原厂电源模式选中态来自 AppSettings `page=1 offset=1`。
- 原厂风扇模式选中态来自 AppSettings `page=4 offset=5`。
- Linux 模块只实现这两个 AppSettings 字段的运行时受限镜像，不开放完整 AppSettings 空间。
- `fan-mode silent` 的原厂值是 `3`，不是旧实现用过的 `2`。
- `fan-curve` 写 EC 时用户态和内核态都校验 4 点 CPU/GPU 曲线，温度递增且占空比不下降。
- GUI 的风扇页只保存曲线到 `settings.json`；总览页选择 `曲线 1/2/3` 才调用 `fan-curve` 写 EC 并切到 custom。

## GPU MUX 逆向结论

原厂存在两代 GPU MUX：

- 旧二状态能力位：`GetWMI(122)` / `psf2_7a` 的 `0x00100000`，写 `SetWMI(121, 11, value)`，`0=MSHybrid`、`1=Discrete`。
- 新四状态能力位：`SetWMIPackageEx(4, sub=8)` 返回 buffer 的 `offset[18] bit0`，写 `SetWMIPackageEx(4, sub=22, value)`，`1=iGPU`、`2=dGPU`、`3=MSHybrid`、`4=DDS`。
- 新状态读取：`SetWMIPackageEx(4, sub=21)`，`o_buffer[0]` 是当前状态，`o_buffer[1]` 是可见选项 bitmask。
- 原厂写完 GPU MUX 后执行 `shutdown.exe -f -r -t 0`，所以 Linux 后续如实现写入口必须做受保护确认和重启流程。

本机实机读回：

```text
psf2_7a                     = 0x70020053
bios_feature_04_08_version  = 0x0100
bios_feature_04_08_offset18 = 0x4d
gpu_mux_04_15_current       = 0x02
gpu_mux_04_15_options       = 0x06
```

解释：

- 旧二状态 `0x00100000` 未置位。
- 新四状态 `offset18 bit0` 已置位，确认支持 GPU MUX。
- 当前状态是 `dGPU`。
- 原厂可见选项是 `dGPU` 和 `MSHybrid`；`iGPU` 不显示。

## 原厂逆向材料

- 原厂安装包目录：`D:\07_ControlCenter`
- 静态提取目录：`C:\Users\pqcmm\oem_cc_static`
- 反编译目录：`C:\Users\pqcmm\oem_cc_decompiled`
- InstallShield 解包目录：`C:\Users\pqcmm\oem_cc_installshield`
- 关键反编译文件：
  - `C:\Users\pqcmm\oem_cc_decompiled\FnKey\FnKey\Features.cs`
  - `C:\Users\pqcmm\oem_cc_decompiled\FnKey\FnKey\Form1.cs`
  - `C:\Users\pqcmm\oem_cc_decompiled\FanSpeedSetting\FanSpeedSetting\FAN.cs`
  - `C:\Users\pqcmm\oem_cc_decompiled\ControlCenter30\ControlCenter30\Window1.cs`

## 最近关键提交

- `e029ef1`：只读暴露新 GPU MUX capability/status，并在高级页显示。
- `67c1c22`：自定义风扇曲线通过 DCHU `fan-curve` 写 EC。
- `e8e95d7`：风扇曲线按钮 disable 按实际修改状态判断。
- `b994bd6`：稳定风扇曲线按钮布局。
- `d9482b8`：清理风扇页嵌套卡片和布局。
- `f9ea343`：新增本地电池策略页。
- `b36d76d`：去掉性能 sidebar menu。
- `acdd2ab`：新增本地风扇曲线编辑器。
- `74825a6`：按原厂能力位控制 UI 显示。
- `d9d4b9a`、`8c5cced`：电源/风扇模式读回和写入对齐原厂行为。

## 已验证结果

- 2026-07-10 本地：`cargo fmt --check`、`cargo check`、`cargo test` 通过。
- 2026-07-10 Linux 笔记本：`cargo check`、`cargo test`、`make -C module` 通过。
- 2026-07-10 Linux 笔记本加载新模块后读回 GPU MUX 数据成功。
- 模块重载会清空运行时 AppSettings 镜像；验证后已恢复为 `app_power_mode 2`、`app_fan_mode 0`。

## 暂不公开或未完成

- GPU MUX 写入未实现；仅高级页只读展示能力和当前状态。
- GPU/CPU OC、Battery Saver、EnergySave、AntiDust 未公开写入口。
- 电池页面目前只保存本地策略配置，不写 EC。
- 不实现任意 DCHU/AppSettings 调试入口。
- 如果后续实现 MUX 写入，必须先复核原厂 `Form1.cs` 写入确认和重启流程，并做失败恢复设计。
