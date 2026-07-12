# DCHU 可调整项记录

本文档只记录当前程序公开支持的 DCHU 调整项。当前版本不再提供裸 DCHU 调试入口；用户态只能通过内核模块暴露的只读状态节点和白名单控制节点工作。

## 当前公开接口

| 能力 | CLI / proc | 校验规则 |
|------|------------|----------|
| 读取实时状态 | `clevo-control-center dchu status` / `/proc/clevo_dchu_status` | 只读；返回 CPU/GPU 风扇 tach 计数、温度块和硬件状态 raw buffer；tach 换算后显示为 RPM，第三路 tach 有数据时额外显示 PCH 风扇。 |
| 读取配置/能力 | `/proc/clevo_dchu_config` | 只读；返回 DCHU 0x0D 配置 buffer、`FANQ`/`KBTP`、`PSF1/PSF2/PSF4/PSF5` 能力整数、GPU MUX 新接口只读回读和受限 AppSettings 模式读回。 |
| 读取 AppSettings 模式状态 | `clevo-control-center dchu app-settings` / `/proc/clevo_dchu_app_settings` | 只读；只返回 `page=1 offset=1` 电源模式和 `page=4 offset=5` 风扇模式。这是运行时受限兼容镜像，不是完整原厂 AppSettings 持久区，不提供任意 AppSettings 读写。 |
| 键盘 RGB | GUI / `/proc/clevo_control_center_led` | 颜色必须是 6 位十六进制；显式分区只允许 `f0..f6`；不写分区时只写默认三分区。 |
| 电源/性能档位 | `clevo-control-center dchu power-mode <0..3> --i-understand` / `/proc/clevo_dchu_control` | 只允许十进制 `0..3`。 |
| 风扇模式 | `clevo-control-center dchu fan-mode <mode> --i-understand` / `/proc/clevo_dchu_control` | 只允许 `auto/max/silent/maxq/custom` 或数字 `0/1/3/5/6`。 |
| 自定义风扇曲线 | `clevo-control-center dchu fan-curve <cpu> <gpu> --i-understand` / `/proc/clevo_dchu_control` | CPU/GPU 各 4 个 `温度:占空比` 点；温度必须递增，占空比不能下降；用户态和内核态都会校验，不接受裸 payload。 |
| GPU MUX | `clevo-control-center dchu gpu-mux <dgpu\|mshybrid> --i-understand` / `/proc/clevo_dchu_control` | 只允许原厂定义的 `2=dGPU` 和 `3=MSHybrid`；GUI 有确认和重启流程，CLI/proc 不自动重启。 |

`/proc/clevo_dchu_control` 只接受四个命令：`fan-mode <value>`、`power-mode <value>`、`fan-curve <cpu> <gpu>` 和 `gpu-mux <value>`。额外参数、未知命令、越界数字和非法曲线都会被内核模块拒绝。风扇/电源写入成功后会同步受限 AppSettings 兼容层，GUI 的按钮选中态只从该层回读，不再从 `0x0D[0x0E]` 推导。

## 已确认映射

- `power-mode 0..3` 参考 opencontrol，对应 `Quiet/Powersaving/Performance/Entertainment`。
- `fan-mode` 以原厂 Control Center 3.0 静态分析为准：`sub=1` 时公开 `0=auto`、`1=max`、`3=silent`、`5=maxq`、`6=custom`。旧实现把 silent 写成 `2`，这是错误值。
- 原厂 UI 选中态不来自 `0x0D[0x0E]`，而是 `ReadAppSettings(1,1,1)` 读电源模式、`ReadAppSettings(4,5,1)` 读风扇模式；Linux 模块只实现这两个字段的运行时受限兼容层，不开放完整 0x1000 AppSettings 空间，也不声称已复刻 Windows AcpiBridge 的持久 AppSettings 存储。
- GUI 可见性按原厂能力位过滤：`PSF5 bit0` 未置位时隐藏电源模式按钮，`PSF5 bit7` 未置位时隐藏风扇模式按钮，`PSF2 bit15` 未置位时不显示 Silent，`0x0D[0x0E] != 5` 时不显示 MaxQ。
- GPU MUX 有两套原厂接口：旧二状态能力位是 `PSF2 bit20`；新四状态能力位来自 `SetWMIPackageEx(4, sub=8)` 的 `offset[18] bit0`。当前模块读取新接口的 capability/status/options，并只公开已验证的 `dGPU`/`MSHybrid` 写入。GUI 写入前要求确认，成功后立即请求系统重启；CLI/proc 写入没有确认框也不自动重启。
- `FanCount > 1` 且 `0x0D[0x2B] bit1 == 0` 说明自定义风扇表能力存在；GUI 的“风扇”页只编辑并保存三组本地 CPU/GPU 曲线，总览页选择 `曲线 1/2/3` 时才把对应曲线转换成 `SetWMIPackage(14)` 风扇表并写入 EC，同时把风扇模式切到 `custom`。
- 原厂只编辑每路风扇的 T2/D2 和 T3/D3。内核写入前重新读取 WMI13，以 CPU/GPU1/GPU2 各自的 T1/D1 作为首锚点，并把 T4/D4 固定为 `100°C/100%`；命令中的 CPU/GPU 首尾点不会覆盖这些固件锚点。
- WMI14 payload 的 T2/T3 duty 按 `round(percent / 100 * 255)` 转为 raw；R12/R23/R34 使用 `round((Dnext-Dprev)/(Tnext-Tprev)*2.55*16)`，并按高字节在前写入。WMI14 使用 package 特有返回语义，只以 ACPI evaluate 成功为写入成功，不要求返回整数回显 function `0x0e`。
- `status` 读取固件状态后解析当前 GUI 需要展示的 CPU/GPU 风扇转速和温度；风扇 raw tach 使用 `2156220 / raw_tach` 换算为 RPM，第三路 tach 非 0 时按 PCH 风扇显示；温度块按 `0x10..0x15` 展示，已确认的 CPU/GPU 字段直接显示为单字节摄氏度值，未知字段按 offset 展示。
- 左侧“高级”页面只读展示风扇 raw/解析值、温度块、AppSettings 模式字段、官方能力位解析、其他非零字段和完整 DCHU raw buffer；不增加新的写入入口。

## 建议测试顺序

1. 先运行 `clevo-control-center dchu status`，确认 `/proc/clevo_dchu_status` 可读。
2. 再运行 `clevo-control-center dchu power-mode 2 --i-understand`，确认普通用户可通过 `/proc/clevo_dchu_control` 写入。
3. 再运行 `clevo-control-center dchu fan-mode auto --i-understand`，确认风扇模式写入不再需要 root。
4. 读取 `/proc/clevo_dchu_config`，确认 WMI13 的 CPU/GPU1/GPU2 首锚点和 T2/T3 当前值。
5. 再运行 `clevo-control-center dchu fan-curve 40:32,58:42,78:72,100:100 40:32,60:44,80:74,100:100 --i-understand`，确认 WMI14 写入成功、`app_fan_mode` 进入 `6=custom`，并观察温度和 RPM。
6. 测试结束后运行 `clevo-control-center dchu fan-mode auto --i-understand` 恢复自动风扇模式。

## 不再公开的内容

- 不再提供任意 DCHU function 读取或写入入口。
- 不再提供 GPU/CPU 超频、EnergySave、Battery Utility 刷新、AntiDust 等尚未形成安全读回或恢复流程的写入命令。GPU MUX 只开放 `dGPU`/`MSHybrid` 白名单模式；Battery Saver 只开放 `on/off`，内核先检查 `WMI4/sub8 offset15 bit2`，再调用 WMI4/sub13，并以写后读回不一致作为失败。
- 不再创建 `/proc/clevo_dchu` 调试节点。
