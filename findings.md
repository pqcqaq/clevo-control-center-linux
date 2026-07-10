# 发现与决策

## 需求
- 用户希望把 Windows C# 程序中通过 `InsydeDCHU.dll` 修改键盘灯光的能力移植到 Linux。
- 目标硬件是 Linux 工作机 `qcqcqc@192.168.4.70`，DMI 显示为 `Notebook NP5x_NP6x_NP7xPNP`，BIOS 厂商 `INSYDE Corp.`，版本 `1.07.05`。

## Windows 源码发现
- 源码路径：`D:\Develop\Coding\C#\Colorful-Keyborad-Led-Color-Setting-main\Colorful-Keyborad-Led-Color-Setting-main\ColorfulLedKeyboardSet`
- C# 只导入了一个 DLL 函数：
  `SetDCHU_Data(int command, byte[] buffer, int length)`
- 设置颜色时调用：
  `SetDCHU_Data(103, bytes, 4)`
- `bytes` 来源为 32 位值的小端序：
  `zone << 24 | B << 16 | R << 8 | G`
- 因此实际传给 DLL 的 4 字节是：
  `[G, R, B, zone]`
- 区域映射：
  - `KbPart 1 -> zone 0xF0`
  - `KbPart 2 -> zone 0xF1`
  - `KbPart 3 -> zone 0xF2`
  - `KbPart 7 -> zone 0xF6`
  - `KbPart 8 -> zone 0xF3`

## DLL 逆向发现
- `InsydeDCHU.dll` 是 PE32+ x64 DLL。
- 导出函数：
  - `GetDCHU_Data_Integer`
  - `GetDCHU_Data_Buffer`
  - `ReadAppSettings`
  - `SetDCHU_Data`
  - `SetDCHU_DataEx`
  - `WriteAppSettings`
- DLL 导入了 `CM_Get_Device_Interface_ListW`、`CreateFileW`、`DeviceIoControl`，说明它先枚举设备接口，再向设备发 IOCTL。
- Windows 设备接口 GUID：
  `{86994c74-ad43-4812-b7e7-0c420b5c5fd7}`
- `_DSM` GUID：
  `{93f224e4-fbdc-4bbf-add6-db71bdc0afad}`
- `SetDCHU_Data` 使用 IOCTL：
  `0x322400`
- `SetDCHU_Data` 输入包中包含：
  - `_DSM` GUID
  - 方法名 `_DSM`
  - command/function index `0x67`
  - buffer 类型参数，长度 0x100，前 4 字节为 `[G, R, B, zone]`

## Linux 初查
- 当前内核：`7.0.12+kali-amd64`
- 未发现现成的 `clevo`、`tuxedo`、`kbd_backlight` sysfs 接口。
- 已加载 WMI 相关模块：`wmi`、`mxm_wmi`，但 `/sys/bus/wmi/devices` 未列出可用设备。
- ACPI 表中发现 `_DSM` GUID `{93f224e4-fbdc-4bbf-add6-db71bdc0afad}`，位于 `/sys/firmware/acpi/tables/DSDT` 偏移 `443606`。
- `acpica-tools` 已安装，用于反编译 ACPI 表。
- DSDT 中核心设备为 `\_SB.DCHU`，`_HID` 为 `CLV0001`。
- `\_SB.DCHU._DSM` 对 GUID `{93f224e4-fbdc-4bbf-add6-db71bdc0afad}` 分发命令。
- 命令 `0x67` 属于 `SCMD` setter 类。
- `SCMD(0x67)` 读取 `Arg3` 的第一个 package 元素作为 0x100 buffer，再把前 4 字节解释为 32 位整数 `ARGS`。
- C# 的 4 字节 `[G,R,B,zone]` 对应 `ARGS = zone << 24 | B << 16 | R << 8 | G`，可直接复刻。
- `SCMD(0x67)` 当 `zone nibble == 0x0F` 且颜色区域码为 0、1、2 时，最终向 EC 写：
  - `FDAT = region + 0x03`
  - `FBUF = B`
  - `FBF1 = R`
  - `FBF2 = G`
  - `FCMD = 0xCA`
- EC 字段位于 `\_SB.PC00.LPCB.EC` 的 EmbeddedControl 区域 offset `0xF8` 起：`FCMD/FDAT/FBUF/FBF1/FBF2/FBF3`。
- 现成 `acpi_call-dkms` 输入端只支持 integer/string/buffer，不支持构造 package；而 `\_SB.DCHU._DSM` 需要 `Arg3 = Package(Buffer)`，所以不能可靠复现 Windows DLL 调用。
- 已实现并在笔记本编译 `clevo_kbd_led.ko`。
- 模块暴露 `/proc/clevo_kbd_led`（键盘灯写入，`0666`）、`/proc/clevo_dchu_status`（只读状态，`0444`）和 `/proc/clevo_dchu`（调试读写，`0600`）。
- 测试写入结果：
  - `f0 ff0000` 成功，dmesg 记录 `set zone=0xf0 rgb=ff0000`
  - `f0 ff0000`、`f1 00ff00`、`f2 0000ff` 均成功
  - `ffffff` all-zones 写法成功，dmesg 记录 `f0/f1/f2` 全部成功
- 当前未做持久安装，模块只是从项目目录手动加载；重启后不会自动加载。

## 技术决策
| 决策 | 理由 |
|------|------|
| 优先反编译 ACPI 表 | 需要获得 `_DSM` 所在设备路径，才能在 Linux 上调用 |
| 先做单次颜色写入工具，不做循环 RGB | 小范围测试固件调用，降低风险 |
| 写最小外部内核模块调用 ACPI | 需要传正确 package 参数，用户态现成工具不够 |
| `/proc` 接口仅 root 可写 | 控制 EC 的接口不应对普通用户全员可写 |

## 遇到的问题
| 问题 | 解决方案 |
|------|---------|
| Linux 上没有现成 Clevo/Tuxedo sysfs 接口 | 转向 ACPI `_DSM` 路径定位 |
| `acpidump`、`iasl` 未安装 | 下一步评估并安装 `acpica-tools` |
| `acpi_call-dkms` 不能构造 package 输入 | 不用它直接测试 `_DSM`，改写专用模块 |

## DCHU 风扇与电源接口追加发现
- 进一步反编译 `D:\ColorfulLedKeyboardSet\InsydeDCHU.dll` 后确认：
  - `GetDCHU_Data_Integer(int command, int *out)` 成功时返回写入的整数值，失败返回 0。
  - `GetDCHU_Data_Buffer(int command, uint8_t *out)` 只有 2 个有效参数；成功时返回 command 本身，失败返回 0。
  - `SetDCHU_DataEx(int command, uint8_t *input, int input_len, uint8_t *out)` 可取回 ACPI buffer 返回值。
- Windows 当前环境调用 `GetDCHU_Data_Buffer(0x0D)` 返回 0，说明本机 DeviceIoControl 没拿到 DCHU 返回，不把这次 Windows 返回当作硬件数据。
- 远端 Linux 上临时编译只读 probe 模块，调用 `\_SB.DCHU._DSM`，只读确认如下：
  - `0x0C` 返回 256 字节 buffer，包含三路风扇 raw tach 计数等实时状态字段；raw tach 需要按 `2156220 / raw_tach` 换算成 RPM。一次样本：`tach1=0x026e` -> `3466 RPM`、`tach2=0x02be` -> `3071 RPM`、`tach3=0x0000`。
  - `0x0C` buffer 的 `0x10..0x15` 是实测温度样字段；`0x11`/`0x12` 与 Linux CPU/GPU 传感器读数交叉匹配，其他字段暂按 offset 展示，不硬编码具体硬件名称。
  - `0x0D` 返回 256 字节 buffer，包含键盘颜色、`FANQ`、`KBTP` 和三组风扇曲线表。一次样本：`FANQ=0x02`、`KBTP=0x06`。
  - `0x10` 返回 integer `0x93`，对应 DSDT 中 `PSF5` 能力掩码结果。
  - `0x52` 返回 integer `0x04680025`，对应 `PSF1`。
  - `0x60` 返回 integer `0x021c`，对应 `PSF4` 加运行时状态位。
  - `0x7A` 返回 integer `0x70020053`，对应 `PSF2` 加平台能力位。
- DSDT 中 `PK0D` 的 `0x0D` buffer 关键偏移：
  - `0x02..0x0A`：键盘三分区当前 RGB，顺序为 left/middle/right 的 R/G/B。
  - `0x0B`：键盘亮度 `KBBH`。
  - `0x0C`：`FANQ`。
  - `0x0E`：执行 `FCMD=0xD7` 后读出的 `FBUF`。
  - `0x0F`：`KBTP`。
  - `0x10..0x17`：Fan1 的 `T1/D1/T2/D2/T3/D3/T4/D4`。
  - `0x18..0x1F`：Fan2 的 `T1/D1/T2/D2/T3/D3/T4/D4`。
  - `0x20..0x27`：Fan3 的 `T1/D1/T2/D2/T3/D3/T4/D4`。
  - `0x2B`：`KPCR`。
- DSDT 中 `PK0E` 是风扇曲线写入口，属于写 EC 的危险接口，暂未调用：
  - 输入 buffer `0x02..0x0D` 写 Fan1/Fan2/Fan3 的 `T2/D2/T3/D3`。
  - 输入 buffer `0x0E..0x1F` 按 little-endian word 写 `F1R1..F3R3`。
  - 固件返回固定整数 `0x14`。
- DSDT 中 `SCMD(0x79)` 是电源/性能相关写入口，暂未调用：
  - payload 仍是首个 DWORD，`sub = payload >> 24`，`value = payload & 0x00ffffff`。
  - `sub=0x19` 且 `value & 0x3f < 4` 时会设置 `EC.CPCM`、`EC.APRD`、`EC.FCMD=0xD8`，并触发 `PRM0=0x11; PRM1=mode; SSMP=0xC0`。
  - `APPM` 映射表为 `[0x02, 0x03, 0x01, 0x00]`，说明 0..3 四个性能档位会映射为不同 EC 参数，但还不能直接命名为静音/娱乐/性能等，需要和原厂控制中心 UI 或实际行为再对照。
- 2026-07-10 使用 `scripts/probe-mode-coupling.sh` 在实机验证电源模式和风扇模式写入后的 `0x0D[0x0E]` 读回值。该字段不是两个独立状态源，而是会被电源/风扇写入共同覆盖：
  - `power:0 -> 0x80`
  - `power:1 -> 0x02`
  - `power:2 -> 0x08`
  - `power:3 -> 0x02`
  - `fan:max -> 0x10`
  - `fan:silent` 旧实现错误值 `2 -> 0x08`
  - `fan:maxq -> 0x02`
  - `fan:auto` 在 `power:0` 基线下保持 `0x80`，在 `0x02` 基线下保持 `0x02`
- 同次隔离测试的旧 UI 选中推导结果：
  - 以 `fan:max` 为基线，写 `power:0/1/2/3` 都会让旧风扇推导从 `max` 变为 `none/power2-or-old-fan2/none` 等状态，说明电源写入会影响该字段。
  - 以 `power:0` 为基线，写 `fan:max/silent/maxq` 会让旧电源推导从 `0` 变为 `none/power2-or-old-fan2/none`，说明风扇写入也会影响该字段。
  - `0x08` 同时对应旧规则里的 `power:2` 和旧实现错误风扇值 `2`，不能同时作为两个按钮组的可靠选中依据。
  - `0x02` 也不能区分 `power:1`、`power:3`、`fan:maxq`、部分 `fan:auto` 场景。
  - 结论：在找到独立 EC 状态位之前，GUI 不应仅靠 `0x0D[0x0E]` 同时高亮电源模式和风扇模式；最多只能把该字段作为高级调试信息或单组临时回读。
- 后续如果要实现风扇/电源功能，建议先做只读 CLI/debug 接口展示 `0x0C/0x0D`，写接口必须二次确认并加显式风险开关，不应直接放进 GUI 默认功能。

## 原厂 Control Center 3.0 静态分析追加发现
- 原厂包路径：`D:\07_ControlCenter`，InstallShield 包显示为 `ControlCenter 3.0 Package v3.97`。仅做静态解包和反编译，未运行安装程序或原厂可执行程序。
- `FanSpeedSetting` 中风扇按钮写入链路：
  - `RB_FAN_auto_Click -> SetFanMode(0)`
  - `RB_FAN_max_Click -> SetFanMode(1)`
  - `RB_FAN_Silent_Click -> SetFanMode(3)`
  - `RB_FAN_Maxq_Click -> SetFanMode(5)`
  - `FAN.SetFanMode` 先 `SetWMI(121, 1, mode)`，再 `SetAPPData(4, 5, 1, [mode])`。
- `FnKey` 中电源模式 enum 为 `0=quiet`、`1=pwrsaving`、`2=performance`、`3=entertainment`。`Features.SetPowerMode(mode)` 先 `WriteAppSettings(1, 1, 1, [mode])`，再写硬件：
  - 普通路径：`SetWMI(121, 25, mode)`，也就是 `SCMD(0x79)` 的 `sub=0x19`。
  - `mode == 2` 时会按 DTT/turbo 标志 OR 上 `0x80` 或 `0x40` 后再写 `SetWMI(121, 25, value)`。
- 原厂 UI 的“当前选中”不读 `0x0D[0x0E]`：
  - 电源模式读 `ReadAppSettings(1, 1, 1)`。
  - 风扇模式读 `ReadAppSettings(4, 5, 1)`。
  - turbo fan 状态读 `ReadAppSettings(4, 8, 1)`。
- 反汇编 `InsydeDCHU.dll` 确认 `ReadAppSettings/WriteAppSettings` 走 Windows `AcpiBridge` 设备的另一路 IOCTL `0x32240c`，读写 0x1000 字节的 AppSettings 区；这不是当前 Linux `_DSM` 直接读到的 `0x0C/0x0D` EC buffer。
  - `ReadAppSettings(page, offset, length)` 先通过 IOCTL 读取 0x1000 字节 AppSettings 区，再从 `page * 0x100 + offset` 复制 `length` 字节。
  - `WriteAppSettings(page, offset, length, data)` 发送 `{op=1, offset=page * 0x100 + offset, length, data...}` 到同一个 IOCTL。
- Linux 模块不开放完整 AppSettings 空间，只实现原厂当前模式读回所需的白名单字段：`page=1 offset=1` 电源模式、`page=4 offset=5` 风扇模式。写 `fan-mode` 成功后按原厂顺序同步 `SetAPPData(4,5,1)`；写 `power-mode` 时先同步 `WriteAppSettings(1,1,1)` 再写 `SCMD(0x79, sub=0x19)`，硬件写失败会回滚该字段，避免 GUI 显示未生效状态。
- 因此 Linux GUI 不应把 `0x0D[0x0E]` 当作电源/风扇选中态；按钮高亮只来自受限 AppSettings 兼容层。
