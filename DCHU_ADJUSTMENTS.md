# DCHU 可调整项记录

本文档只记录当前程序公开支持的 DCHU 调整项。当前版本不再提供裸 DCHU 调试入口；用户态只能通过内核模块暴露的只读状态节点和白名单控制节点工作。

## 当前公开接口

| 能力 | CLI / proc | 校验规则 |
|------|------------|----------|
| 读取实时状态 | `clevo-control-center dchu status` / `/proc/clevo_dchu_status` | 只读；返回 CPU/GPU 风扇 tach 计数、温度块和硬件状态 raw buffer；tach 换算后显示为 RPM，第三路 tach 有数据时额外显示 PCH 风扇。 |
| 读取配置/能力 | `/proc/clevo_dchu_config` | 只读；返回 DCHU 0x0D 配置 buffer、`FANQ`/`KBTP`、`PSF1/PSF2/PSF4/PSF5` 能力整数和受限 AppSettings 模式读回。 |
| 读取 AppSettings 模式状态 | `clevo-control-center dchu app-settings` / `/proc/clevo_dchu_app_settings` | 只读；只返回 `page=1 offset=1` 电源模式和 `page=4 offset=5` 风扇模式，不提供任意 AppSettings 读写。 |
| 键盘 RGB | GUI / `/proc/clevo_control_center_led` | 颜色必须是 6 位十六进制；显式分区只允许 `f0..f6`；不写分区时只写默认三分区。 |
| 电源/性能档位 | `clevo-control-center dchu power-mode <0..3> --i-understand` / `/proc/clevo_dchu_control` | 只允许十进制 `0..3`。 |
| 风扇模式 | `clevo-control-center dchu fan-mode <mode> --i-understand` / `/proc/clevo_dchu_control` | 只允许 `auto/max/silent/maxq/custom` 或数字 `0/1/3/5/6`。 |

`/proc/clevo_dchu_control` 只接受两个命令：`fan-mode <value>` 和 `power-mode <value>`。额外参数、未知命令、越界数字都会被内核模块拒绝。写入成功后会同步受限 AppSettings 兼容层，GUI 的按钮选中态只从该层回读，不再从 `0x0D[0x0E]` 推导。

## 已确认映射

- `power-mode 0..3` 参考 opencontrol，对应 `Quiet/Powersaving/Performance/Entertainment`。
- `fan-mode` 以原厂 Control Center 3.0 静态分析为准：`sub=1` 时公开 `0=auto`、`1=max`、`3=silent`、`5=maxq`、`6=custom`。旧实现把 silent 写成 `2`，这是错误值。
- 原厂 UI 选中态不来自 `0x0D[0x0E]`，而是 `ReadAppSettings(1,1,1)` 读电源模式、`ReadAppSettings(4,5,1)` 读风扇模式；Linux 模块只实现这两个字段的受限兼容层，不开放完整 0x1000 AppSettings 空间。
- `status` 读取固件状态后解析当前 GUI 需要展示的 CPU/GPU 风扇转速和温度；风扇 raw tach 使用 `2156220 / raw_tach` 换算为 RPM，第三路 tach 非 0 时按 PCH 风扇显示；温度块按 `0x10..0x15` 展示，已确认的 CPU/GPU 字段直接显示为单字节摄氏度值，未知字段按 offset 展示。
- 左侧“高级”页面只读展示风扇 raw/解析值、温度块、AppSettings 模式字段、其他非零字段和完整 DCHU raw buffer；不增加新的写入入口。

## 建议测试顺序

1. 先运行 `clevo-control-center dchu status`，确认 `/proc/clevo_dchu_status` 可读。
2. 再运行 `clevo-control-center dchu power-mode 2 --i-understand`，确认普通用户可通过 `/proc/clevo_dchu_control` 写入。
3. 再运行 `clevo-control-center dchu fan-mode auto --i-understand`，确认风扇模式写入不再需要 root。
4. 测试风扇模式时观察温度和转速，避免长时间停在不熟悉的静音或自定义档位。

## 不再公开的内容

- 不再提供任意 DCHU function 读取或写入入口。
- 不再提供风扇曲线、键盘亮度、能力位读取等未收敛为稳定 UI 控件的命令。
- 不再创建 `/proc/clevo_dchu` 调试节点。
