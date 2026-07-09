# DCHU 可调整项记录

本文档只记录当前程序公开支持的 DCHU 调整项。当前版本不再提供裸 DCHU 调试入口；用户态只能通过内核模块暴露的只读状态节点和白名单控制节点工作。

## 当前公开接口

| 能力 | CLI / proc | 校验规则 |
|------|------------|----------|
| 读取实时状态 | `clevo-control-center dchu status` / `/proc/clevo_dchu_status` | 只读；返回两路主要风扇转速、CPU/GPU 温度和部分硬件状态字段。 |
| 键盘 RGB | GUI / `/proc/clevo_control_center_led` | 颜色必须是 6 位十六进制；显式分区只允许 `f0..f6`；不写分区时只写默认三分区。 |
| 电源/性能档位 | `clevo-control-center dchu power-mode <0..3> --i-understand` / `/proc/clevo_dchu_control` | 只允许十进制 `0..3`。 |
| 风扇模式 | `clevo-control-center dchu fan-mode <mode> --i-understand` / `/proc/clevo_dchu_control` | 只允许 `auto/max/silent/maxq/custom/turbo` 或数字 `0/1/3/5/6/7`。 |

`/proc/clevo_dchu_control` 只接受两个命令：`fan-mode <value>` 和 `power-mode <value>`。额外参数、未知命令、越界数字都会被内核模块拒绝。

## 已确认映射

- `power-mode 0..3` 参考 opencontrol，对应 `Quiet/Powersaving/Performance/Entertainment`。
- `fan-mode` 参考 opencontrol/opendchu，常见映射为 `0=auto`、`1=max`、`3=silent`、`5=maxq`、`6=custom`、`7=turbo`。
- `status` 读取固件状态后解析当前 GUI 需要展示的两路主要风扇转速和 CPU/GPU 温度，第三路风扇在当前机器样本中为 `0`。

## 建议测试顺序

1. 先运行 `clevo-control-center dchu status`，确认 `/proc/clevo_dchu_status` 可读。
2. 再运行 `clevo-control-center dchu power-mode 2 --i-understand`，确认普通用户可通过 `/proc/clevo_dchu_control` 写入。
3. 再运行 `clevo-control-center dchu fan-mode auto --i-understand`，确认风扇模式写入不再需要 root。
4. 测试风扇模式时观察温度和转速，避免长时间停在不熟悉的静音或自定义档位。

## 不再公开的内容

- 不再提供任意 DCHU function 读取或写入入口。
- 不再提供风扇曲线、键盘亮度、能力位读取等未收敛为稳定 UI 控件的命令。
- 不再创建 `/proc/clevo_dchu` 调试节点。
