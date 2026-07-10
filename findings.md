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

## 原厂反编译证据链完整记录

### 静态分析范围与工具
- 原厂安装包来源：`D:\07_ControlCenter`。
- 已解包静态工作目录：`C:\Users\pqcmm\oem_cc_static`。
- 已分析 AppX：
  - `ControlCenter\AppxManifest.xml`：`CLEVOCO.ControlCenter3.0`，版本 `3.94.0.0`，full-trust 入口 `ControlCenter30\ControlCenter30.exe`，协议 `clevocc30:`。
  - `FanSpeed\AppxManifest.xml`：`CLEVOCO.504814C03D814`，版本 `3.93.0.0`，full-trust 入口 `FanSpeedSetting\FanSpeedSetting.exe`，协议 `clevofan:`。
- 已分析二进制：
  - `ControlCenter\ControlCenter30\ControlCenter30.exe`
  - `ControlCenter\ControlCenter30\InsydeDCHU.dll`
  - `FanSpeed\FanSpeedSetting\FanSpeedSetting.exe`
  - `FanSpeed\FanSpeedSetting\InsydeDCHU.dll`
- 已继续解包/反编译的高级组件：
  - `FnKey`：托盘菜单、能力位解析、GPU MUX、Battery Saver、风扇/电源落地逻辑。
  - `DCHUService` / `LaunchFnkey`：服务启动、CPU OC AppSettings 读写、AMD Ryzen Master SDK 安装入口。
  - `ControlGPU` / `GPUOverclocking`：NVIDIA GPU 信息、限频、GC6、核心/显存 OC、风扇曲线 UI。
  - `CPUOC2` / `CC30_BG` / `CPUOC_Loading`：Intel XTU 写入、CPU OC AppSettings page 6 字段。
  - `BatteryPackUtility` / `EnergySave`：电池刷新、节能计划、充放电阈值、Battery Saver。
- 使用工具：`ilspycmd 8.2.0.7535`、`objdump`、`strings`、`7z`。全程只做静态解包、反编译、反汇编和字符串查看，未运行安装程序、未运行原厂 exe、未查看图片资源。
- `data1.cab`/`data2.cab` 是 InstallShield 数据包，`7z` 不能作为标准 CAB 直接列出。未继续通过运行安装器来展开，避免执行原厂程序。

### C# 到 InsydeDCHU.dll 的公共封装
- `ControlCenter30.DCHU` 和 `FanSpeedSetting.InsydeDriver` 都通过 P/Invoke 调用 `InsydeDCHU.dll`。
- DLL 导出函数确认如下：
  - `GetDCHU_Data_Integer`
  - `GetDCHU_Data_Buffer`
  - `ReadAppSettings`
  - `SetDCHU_Data`
  - `SetDCHU_DataEx`
  - `WriteAppSettings`
- `SetWMI(command, data)` 把 `data` 作为 little-endian 4 字节传给 `SetDCHU_Data(command, bytes, 4)`。
- `SetWMI(command, sub, data)` 先把 `data` 转成 little-endian，再把第 4 字节覆盖为 `sub`，所以 Linux 对应 payload 是 `(sub << 24) | data`。
- `SetWMIPackage(command, buffer)` 传 256 字节 buffer。
- `SetWMIPackageEx(command, buffer, out)` 传 256 字节并取回 ACPI 返回 buffer。
- `GetWMIPackage(command)` 调用 `GetDCHU_Data_Buffer(command, ref array[0])` 并返回 256 字节数组。
- `GetAPPData(page, index, length)` 是 `ReadAppSettings(page, index, length)` 的 C# 包装。
- `SetAPPData(page, index, length, data)` 是 `WriteAppSettings(page, index, length)` 的 C# 包装。

### Native DLL 路径与 IOCTL
- `InsydeDCHU.dll` 是 PE32+ x64 DLL，字符串中包含 PDB 路径 `D:\LOCAL_SOURCE_CODE\InsydeDCHU_dll\x64\Release\InsydeDCHU.pdb`。
- DLL 导入 `CM_Get_Device_Interface_List_SizeW`、`CM_Get_Device_Interface_ListW`、`CreateFileW`、`DeviceIoControl`，说明它枚举设备接口后打开设备并发 IOCTL。
- 设备接口 GUID 在 DLL 数据段中出现为 `{86994c74-ad43-4812-b7e7-0c420b5c5fd7}`。
- ACPI `_DSM` GUID 在 DLL 数据段中出现为 `{93f224e4-fbdc-4bbf-add6-db71bdc0afad}`，并能看到 `_DSM` 字符串。
- DCHU `_DSM` 调用使用 IOCTL `0x322400`。反汇编中多处 `DeviceIoControl` 前设置：
  - control code `0x322400`
  - 输入大小约 `0x40c`
  - 输出大小约 `0x420`
- AppSettings 读写使用另一条 IOCTL `0x32240c`，不是 `_DSM` 的 `0x322400`，也不是 Linux 当前直接读取到的 `0x0C/0x0D` EC buffer。
- `ReadAppSettings(page, offset, length)` 的 native 行为：
  - 读取 0x1000 字节 AppSettings 区。
  - 计算线性偏移 `page * 0x100 + offset`。
  - 从该偏移复制 `length` 字节给调用方。
- `WriteAppSettings(page, offset, length, data)` 的 native 行为：
  - 构造写包，包含 `op=1`、线性偏移 `page * 0x100 + offset`、`length` 和数据。
  - 通过 `0x32240c` 发给 Windows AcpiBridge 侧驱动。
- 结论：AppSettings 是原厂 Windows 驱动维护的独立设置区；Linux 当前只能实现受限兼容镜像，不能声称已完整读写 Windows AppSettings 存储。

### 风扇实时数据和换算
- `FanSpeedSetting.FAN.Read_FanSpeed()` 读取 `GetWMIPackage(12)`，即 DCHU command `0x0C`。
- 原厂字段读取：
  - CPU fan raw tach：`array[3] + (array[2] << 8)`
  - GPU1 fan raw tach：`array[5] + (array[4] << 8)`
  - GPU2 fan raw tach：`array[7] + (array[6] << 8)`
  - CPU fan duty raw：`array[16]`
  - GPU1 fan duty raw：`array[19]`
  - GPU2 fan duty raw：`array[22]`
- 原厂 UI 在 `Page_system_monitor.UpdateUI_CPUFan/UpdateUI_GPUFan()` 中把 raw tach 换算为显示 RPM：
  - `rpm = 60.0 / (5.565217391304348E-05 * raw_tach) * 2.0`
  - 等价常数约 `2156220 / raw_tach`
- 因此 `array[2..7]` 不是转速本身，而是 tach 周期计数；raw 越大代表实际 RPM 越低。
- 原厂 duty 显示不是直接显示 raw，而是按 `raw / TurboFan_MaxDuty * 100` 或自定义模式下 `raw / 255 * 100` 换算百分比。

### 温度读取
- 原厂 `FanSpeedSetting.FAN.Read_FanSpeed()` 同样从 `GetWMIPackage(12)` 读取温度相关字段。
- 原厂字段读取：
  - CPU remote temp：`Global.RW_REG.CalCPUTemp(Global.RW_REG.GetTDP(), array[18])`
  - GPU1 remote temp：`array[21]`
  - GPU2 remote temp：`array[24]`
- Linux 实机上 `0x0C[0x10..0x15]` 都表现为温度样字段；其中 `0x11`/`0x12` 与 CPU/GPU 传感器交叉匹配，当前 UI 只把确认度较高的 CPU/GPU 温度放在首页，其他 offset 放高级页。
- CPU 温度在原厂 UI 里可能经过 `CalCPUTemp(TDP, raw)` 修正；Linux 当前直接展示 EC 单字节摄氏度候选值，后续若要完全对齐 OEM，需要继续反编译 `RWReg.CalCPUTemp` 与 `GetTDP`。

### 风扇模式和 AppSettings
- 原厂 `FanSpeedSetting.FAN.SetFanMode(byte mode)` 明确顺序：
  - `SetWMI(121, 1, mode)`
  - `SetAPPData(4, 5, 1, [mode])`
- 对 Linux `_DSM` 来说，这等价于 command `0x79`，payload `(0x01 << 24) | mode`。
- 原厂按钮映射：
  - auto -> `mode=0`
  - max -> `mode=1`
  - silent -> `mode=3`
  - maxq -> `mode=5`
  - custom -> `mode=6`
- 旧实现把 silent 当 `2` 是错误的；官方静音值是 `3`。
- 原厂风扇当前选中态从 `GetAPPData(4, 5, 1)` 读取，不从 `0x0D[0x0E]` 推导。
- 原厂是否显示 Silent 不是无条件：
  - `Init_Fan_Set_UI()` 中如果 `!Global.support_bit.FanLess`，会移除 `SP_Silent`。
  - `FanLess` 来自能力位，而不是写一次后看是否生效。
- 原厂是否显示 MaxQ 也不是无条件：
  - `Read_WMI13()` 中 `InitFanMode == 5` 会设置 `Global.support_bit.MaxQ = true`。
  - UI 中如果 `!Global.support_bit.MaxQ`，会移除 `SP_MaxQ`。

### 风扇配置、自定义曲线与暂不开放项
- 原厂 `FAN.Read_WMI13()` 读取 `GetWMIPackage(13)`，即 DCHU command `0x0D`。
- 原厂读取：
  - `FanCount = array[12]`
  - `InitFanMode = array[14]`
  - `SupportCustomFan = false` 条件：`FanCount <= 1` 或 `((array[43] >> 1) & 1) == 1`
  - CPU 曲线：`T1/D1=array[16]/array[17]`，`T2/D2=array[18]/array[19]`，`T3/D3=array[20]/array[21]`，`T4/D4=100/100`。
  - GPU1 曲线：`T1/D1=array[24]/array[25]`，`T2/D2=array[26]/array[27]`，`T3/D3=array[28]/array[29]`，`T4/D4=100/100`。
  - GPU2 曲线：`T1/D1=array[32]/array[33]`，`T2/D2=array[34]/array[35]`，`T3/D3=array[36]/array[37]`，`T4/D4=100/100`。
  - duty raw 不是百分比，原厂按 `round(raw / 255 * 100)` 转换显示。
- 原厂 `FAN.SetCustomFanTable()` 会构造 256 字节包并调用 `SetWMIPackage(14, array)` 写入自定义风扇曲线；这对应 DCHU command `0x0E`，会写 EC 风扇表。
- `SetWMIPackage(14)` 写入格式：
  - `array[2..13]` 只写 CPU/GPU1/GPU2 的 `T2,D2,T3,D3`，duty 百分比按 `round(percent / 100 * 255)` 转回 raw。
  - `array[14..31]` 写 CPU/GPU1/GPU2 的 `R12/R23/R34` 斜率，高字节在前。
  - 斜率公式：`round((D_next - D_prev) / (T_next - T_prev) * 2.55 * 16.0)`。
- 原厂 AppSettings page 4 offset 0 len 256 还保存一份 UI 持久化风扇表：
  - offset 4/5/6/7/8 分别是 `InitFanMode/FanMode/FanCount/FanOffset/TurboFanStatus`。
  - CPU 段：duty `16..19`，默认 duty `20..21`，温度 `22..25`，默认温度 `26..27`，R 值 little-endian `28..33`。
  - GPU1 段同布局起始 `34`，GPU2 段同布局起始 `52`。
- 风扇曲线写入涉及多个温度点、duty、斜率和 AppSettings 镜像，属于高风险 EC 表写入；当前 Linux 不开放该功能，只在高级页展示只读信息。
- 原厂 `SetFanOffset(byte offset)` 使用 `SetWMI(121, 14, data)` 并 `SetAPPData(4, 7, 1, [offset])`；当前 Linux 不开放 fan offset。
- 原厂 AntiDust 相关接口使用 `SetWMI(121, 40/41, ...)` 和 AppSettings page 4 offset 80/81；当前 Linux 不开放。

### 电源模式和 AppSettings
- `ControlCenter30.Page_1App` 是控制中心主页面，电源模式 UI 只直接写事件日志：
  - Quiet click：`clevocc30^101^0`
  - PowerSaving click：`clevocc30^101^1`
  - Performance click：`clevocc30^101^2`
  - Entertainment click：`clevocc30^101^3`
- `Page_1App.ReadPowerModeInsydeBuffer()` 明确读取 `ReadAppSettings(1, 1, 1)` 作为当前电源模式，并据此高亮按钮和切换图标。
- `Page_1App` 会监听 `PowerBiosServerLog/OutLog` 事件，如收到 `clevofnkey^202` 或 `Set PowerMode:` 再重新读取 AppSettings。
- 由于真正处理 `clevocc30^101^n` 的 FnKey/服务端未在当前 AppX 静态目录中完整展开，电源硬件写入链路还带有外部服务组件缺口。
- 已找到的原厂电源行为结论：
  - 电源选中态来自 `ReadAppSettings(1, 1, 1)`。
  - 模式值为 `0=quiet`、`1=powersaving`、`2=performance`、`3=entertainment`。
  - 硬件写入应使用 `SetWMI(121, 25, mode)`，也就是 DCHU command `0x79`、subcommand `0x19`。
  - performance 模式会按 DTT/turbo 标志把写入值 OR 上 `0x80` 或 `0x40`：`ReadAppSettings(1,32,1)==1` 且 DTT 驱动已安装时 OR `0x80`，`ReadAppSettings(4,8,1)==1` 时 OR `0x40`；如果 DTT 驱动缺失，原厂会清掉对应 AppSettings 标志并回写普通 performance。
- Linux 当前只公开 `power-mode 0..3`，不公开 OR `0x80/0x40` 的裸值；这样更安全，也符合“不暴露 payload”的约束。

### TurboFan、DTT 与性能页附加开关
- Control Center 性能页 `CB_TurnOnTruboFan_Click()` 写入：
  - `WriteAppSettings(4, 8, 1, [1/0])` 保存 TurboFan 勾选状态。
  - 支持 DTT 时同步 `WriteAppSettings(1, 32, 1, [1/0])`。
  - 硬件写入 `SetWMI(121, 25, 2 | (turbo << 6) | (dtt << 7))`。
  - 写完会把风扇模式 AppSettings `4:5` 回到 `0`，即自动模式。
- `SetFanMode(7)` 在 Control Center 里被当作 TurboFan 快捷分支：它写 `SetWMI(121,1,7)`，只更新 `WriteAppSettings(4,8,1,[1])`，不把 `4:5` 写成 `7`。
- `CB_CPUDynamic_Click()` 是 DTT/CPU dynamic：选中写 `WriteAppSettings(1,32,1,[1])` 和 `SetWMI(121,25,130)`，取消写 `[0]` 和 `SetWMI(121,25,2)`。
- 结论：TurboFan/DTT 与 performance 模式耦合，会同时改风扇模式镜像和电源硬件值；Linux 目前不应把它们暴露成独立裸写入口。

### 独显直连 / GPU MUX 切换
- 原厂存在两代 MUX/显卡切换接口。
- 旧二状态接口：
  - 能力位来自 `GetWMI(122)` bit `0x100000`，设置 `SupportMSHybrid_dGPUSwicth`。
  - 当前状态读取 `Global.dchu.GetWMI(84)`；返回 `1` 时原厂勾选 Discrete，否则勾选 MSHybrid。
  - 写入 `Features.GPUSwitch(int value)`，即 `SetWMI(121, 11, value)`；`0=MSHybrid`，`1=Discrete`。
  - UI 写完会提示用户并执行 `shutdown.exe -f -r -t 0` 立即重启。
- 新四状态接口：
  - 能力 buffer 由 `SetWMIPackageEx(4, array[0]=8, out o_buffer)` 读取，原厂随后写入 AppSettings page 7。
  - 能力位来自这个 capability buffer 的 `offset[18] bit0`，设置 `SupportMSHybrid_dGPU_iGPUSwicth`。
  - 查询当前状态：`SetWMIPackageEx(4, array[0]=21, out o_buffer)`。
  - `o_buffer[0]` 状态：`1=iGPU`，`2=dGPU`，`3=MSHybrid`，`4=DDS`。
  - `o_buffer[1]` 是可见选项 bitmask：bit0 iGPU，bit1 dGPU，bit2 MSHybrid；DDS 菜单项存在且状态值可读/可写，但原厂代码没有看到按 bitmask 放出 DDS 的逻辑。
  - 写入 `Features.GPUSwitch_new(byte value)`，即 `SetWMIPackageEx(4, array[0]=22, array[1]=value)`。
  - UI 写完同样立即重启。
- 本机 Linux 已读旧 `GetWMI(122)`/`0x7A` 返回 `0x70020053`，未置位 `0x00100000`，所以如果 Windows 原厂控制台支持独显直连，应该走新四状态路径而不是旧二状态路径。
- 2026-07-10 用扩展后的只读 Linux 模块实机确认：
  - `bios_feature_04_08_version = 0x0100`。
  - `bios_feature_04_08_offset18 = 0x4d`，bit0 已置位，确认支持新四状态 GPU MUX。
  - `gpu_mux_04_15_current = 0x02`，当前是 `dGPU`。
  - `gpu_mux_04_15_options = 0x06`，原厂可见选项为 `dGPU` 和 `MSHybrid`；`iGPU` 不显示。
- 结论：MUX 切换需要先读 `WMI4/sub8 offset18 bit0` 能力，再读 `WMI4/sub21` 当前状态/可见选项；写入必须是受保护流程，写后提示并执行重启，不能作为普通即时开关。

### GPU 超频、限频与 GC6
- GPU 超频不是单纯 DCHU/EC 写入。原厂通过 `ControlGPU.exe` 调 `NVGPU_DLL.dll`：
  - `InitGPU_API()` 初始化 NVIDIA API。
  - `Get_GPU_TotalNumber()`、`Get_GPU_Base_Clock()`、`Get_NVDeviceID(0)`、`Drvier_version()` 读取设备信息。
  - 设备信息写 AppSettings page 5：offset `0` len `2` 保存 NV device id，offset `6` len `7` 保存 GPU 数量、base clock、ready flag、驱动版本。
  - `Set_CoreOC(index, offset)` 和 `Set_MEMOC(index, offset)` 写核心/显存 offset。
  - `Lock_Frequency(index, freq)` 做 NVIDIA lock frequency，多 GPU 时 index 0/1 都写。
- FnKey/GPU 组件从 `GetWMIPackage(17)` 读取默认 GPU OC 表：
  - NV ID 槽在 offset `48/50`、`64/66`、`80/82`、`96/98`。
  - 默认 core/VRAM offset 在 `52/54`、`68/70`、`84/86`、`100/102`。
  - 用户自定义 GPU OC offset 持久化在 AppSettings page 5 offset 16 len 8，最多两块 GPU 的 int16 core/mem pair。
- GPU clock limit 和 GC6：
  - 限频能力来自 v1 `offset[15] bit0` 或对应 v0 能力位，读取 `SetWMIPackageEx(4, array[0]=9)`。
  - 返回 `o_buffer[2..9]` 分别给 entertainment/performance/powerSaving/quiet 的 limit clock，`o_buffer[10..11]` 还用于临时/温度相关值。
  - GC6 能力来自 v1 `offset[15] bit1`，读取 `SetWMIPackageEx(4, array[0]=10)`，再调用 `ControlGPU.exe GC6:<state>` 写 NVIDIA 注册表 `EnableCoprocPowerControl`。
- 结论：GPU OC/限频依赖 NVIDIA 私有 DLL 和 Windows 注册表，Linux 不能把它归入 `/proc/clevo_dchu_control` 的简单白名单。

### CPU / 内存超频与 AMD Ryzen Master
- CPU OC 能力来源：
  - `GetWMI(16)` bit `0x40` 设置 `SupportCPUOC_WMI16Bit6`。
  - `GetWMI(122)` bit `0x800000` 设置 `SupportCPUOC_WMI122Bit23`。
  - `GetWMI(96)` bit `0x40` 用于 under-volting controller 检查；置位时原厂会禁用相关能力。
  - `GetWMI(122)` bit `0x1000000` 表示 XMP 能力。
- Intel CPU OC 不是 DCHU 直接调电压/倍频。UI 把设置写入 AppSettings page 6，后台 `CC30_BG` 通过 Intel XTU SDK `TuningLibrary.Instance.Tune(ID, value, rebootRequired)` 应用。
- 已确认 page 6 字段：
  - offset 32 len1：保存标志。
  - offset 33/37/41 len4：PL1、PL2、PL time float。
  - offset 49 len4：CPU VR current/limit。
  - offset 53..60 len1：P-core ratio id 29/30/31/32/42/43/96/97。
  - offset 61/65/69 len4：CPU voltage offset、override voltage float、graphics voltage。
  - offset 73/74 len1：CPU voltage mode、CPU VR auto。
  - offset 75..76 len1：额外 P-core ratio id 107/108。
  - offset 77..84 len1：E-core ratio id 4500..4507。
- `DCHUService` 对 AMD 的主要证据是安装/调用 `AMDRyzenMasterDriver.inf/sys`、`Device.dll`、`Platform.dll`、`InstallRyzenMasterSDK.exe`、`GetProductdll.dll`；它会在 AMD CPU 上通过服务侧启动安装入口。
- 结论：CPU/内存超频属于 AppSettings + XTU/Ryzen Master SDK + 后台服务共同实现，当前 Linux 不应开放写入口；若以后实现，需要另立受保护模块并做平台检测。

### 电池、节能和充放电控制
- Battery Saver 托盘项：
  - 能力位来自 v1 `offset[15] bit2`，设置 `SupportBatterySaverSetting`。
  - 读取 `SetWMIPackageEx(4, array[0]=13, array[1]=0, array[2]=0)`，状态在 `o_buffer[0]`。
  - 写入 `SetWMIPackageEx(4, array[0]=13, array[1]=1, array[2]=status)`。
- EnergySave：
  - `EnableEnergySave(false/true)` 写 `SetWMI(118, 0x05000000/0x05000001)`。
  - 节能计划会生成多条 `SetWMI(118, data)`：`0x01000000` 当前时间，`0x02000000` 星期和放电阈值，`0x03000000`/`0x04000000` 两段 schedule，`0x06000000` 停止放电/停止充电阈值。
  - 默认充放电阈值从 `GetWMIPackage(17)` offset `0xD0`/`0xD1` 读取。
  - 能源之星模式 `SetEnergyStarMode(mode)` 写 `SetWMI(79, mode)`。
- BatteryUtility 电池刷新：
  - 读取 `GetWMIPackage(7)`，解析生产日期、循环次数、满充容量、设计容量、BatteryStatus、PFStatus、OperationStatus、StopChargingThreshold。
  - 读取同一个 WMI7 的 offset 32 起阈值和条件表达式，用于判断电池健康/是否建议刷新。
  - 刷新流程写 `SetWMI(121,28,ACOFF)`、`SetWMI(121,29,Refresh)`、`SetWMI(118,7,FlexiCharge)`，并临时切换 Windows 电源计划。
- 结论：电池/节能接口会实际改变充放电策略和 Windows 电源计划，不进入当前 Linux 公开写接口；后续最多先做只读展示。

### 其他已确认但不进入当前公开接口的能力
- ASPM/电源计划：
  - `ReadASPMControlStatus()` 读 `SetWMIPackageEx(4, array[0]=15)`，返回各电源模式 AC/DC ASPM 表。
  - `Read_PwerPlanTable()` 读 `SetWMIPackageEx(4, array[0]=29)`，返回各模式对应 Windows power plan。
  - `Set_PowerPlan()` 调 Windows power plan GUID，不是 EC 控制。
- PgUp/PgDn 开关：
  - AppSettings `page=1 offset=33` 保存 UI 状态。
  - 旧 BIOS 写 `SetWMI(121,45,value)`，新 BIOS 写 `SetWMIPackageEx(4, array[0]=26, array[1]=value)`。
- Battery low control：
  - 能力位来自 v1 `offset[16] bit6`。
  - 原厂用 AppSettings `page=1 offset=13` 保存亮度和电量，电池低时改 Windows 亮度，恢复条件满足后再恢复。
- 低刷新率：
  - Control Center 用 AppSettings `page=1 offset=28` 保存勾选状态。
  - 实际刷新率切换走 Windows display API，不是 DCHU 写 EC。
- 这些能力说明原厂 Control Center 是“DCHU + AppSettings + Windows API + 第三方 SDK”的组合；Linux 侧要保持 `/proc/clevo_dchu_control` 小而可验证。

### 能力位和 UI 可见性
- `FanSpeedSetting.InsydeDriver.Init_WMI()` 先读 `GetWMI(70)`，再读 `GetAPPData(7, 0, 256)` 决定走 `BIOSSpecialFeature_v0_0` 或 `BIOSSpecialFeature_v1_0`。
- v0 能力来源：
  - `GetWMI(16)`：UWP capability bits，bit0 PowerMode、bit7 FanSetting 等。
  - `GetWMI(122)`：bit15 `FanLess`。
  - `GetWMI(96)`：bit7 `AntiDust_Fan`、bit10 关闭 `FanOffset`。
- v1 能力来源：
  - AppSettings page 7 的结构化 capability buffer。
  - `num4` bit0 PowerMode、bit7 FanSetting 等。
  - `num2` bit15 `FanLess`。
  - `num3` bit7 `AntiDust_Fan`、bit10 关闭 `FanOffset`、bit12 `DTT`。
  - `offset[15]` bit0/bit1/bit2 分别是 `SupportLimitGPUClock`、`SupportGC6Setting`、`SupportBatterySaverSetting`。
  - `offset[16]` bit4 `TurboFan`，bit5 `AMDCC30PowerMode`，bit6 `BATLowControl`。
  - `offset[17]` low nibble 是四个电源模式是否可见，high nibble 是 `SupportPowerModeUI_ID`。
  - `offset[18]` bit0 是新四状态 GPU MUX，bit1/bit2/bit3 分别对应 HDR limit、165MHz panel S3 flicker、UCSI yellow mark workaround。
- 实机 Linux 读过的能力命令：
  - `0x10` 返回 `0x93`。
  - `0x52` 返回 `0x04680025`。
  - `0x60` 返回 `0x021c`。
  - `0x7A` 返回 `0x70020053`。
  - `WMI4/sub8` 返回版本 `0x0100`，`offset[18]=0x4d`。
  - `WMI4/sub21` 返回当前状态 `0x02`、选项 bitmask `0x06`。
- Linux 模块从 2026-07-10 起把原厂新 MUX 所需的只读数据也并入 `/proc/clevo_dchu_config`：
  - `bios_feature_04_08_version`：`SetWMIPackageEx(4, sub=8)` 返回的版本字节 `[0..1]`。
  - `bios_feature_04_08_offset18`：新四状态 GPU MUX 能力位所在字节，bit0 为 `SupportMSHybrid_dGPU_iGPUSwicth`。
  - `gpu_mux_04_15_current/options`：`SetWMIPackageEx(4, sub=21)` 返回的当前状态和可见选项 bitmask。
- 当前 Linux UI 的能力裁剪应优先从稳定可解释字段开始；未完全确认的 capability bit 可以在高级页展示，不应直接变成写入口。

### 0x0D[0x0E] 的错误用法结论
- Linux 实机脚本 `scripts/probe-mode-coupling.sh` 已验证 `0x0D[0x0E]` 会被电源模式和风扇模式共同覆盖。
- 该字段无法同时区分：
  - `power:1`、`power:3`、`fan:maxq`、部分 `fan:auto` 场景都会落到 `0x02`。
  - `power:2` 和旧错误 `fan:silent=2` 都可能落到 `0x08`。
- 因此 `0x0D[0x0E]` 只能作为高级调试信息，不能作为 GUI 电源/风扇按钮的选中态来源。
- 官方 AppSettings 读回路径已经解释了为什么 UI 选中态需要另一路存储：硬件状态字段与 UI 状态字段不是同一个概念。

### 当前 Linux 对齐策略
- Linux 内核模块只保留一个统一控制入口 `/proc/clevo_dchu_control`，只接受白名单命令：
  - `fan-mode <auto|max|silent|maxq|custom|0|1|3|5|6>`
  - `power-mode <0..3>`
  - `fan-curve <cpu 4点> <gpu 4点>`，每点为 `温度:占空比`，温度递增且占空比不下降。
- Linux 内核模块只保留受限 AppSettings 兼容读回 `/proc/clevo_dchu_app_settings`：
  - `page=1 offset=1`：电源模式选中态。
  - `page=4 offset=5`：风扇模式选中态。
- 该兼容层不是完整 AppSettings，不提供任意 page/offset 读写，不暴露 payload。
- 写 `fan-mode` 按官方顺序：先写硬件 `SCMD(0x79, sub=1)`，成功后同步 AppSettings 兼容字段 `4:5`。
- 写 `power-mode` 按官方读回语义：先更新 AppSettings 兼容字段 `1:1`，再写硬件 `SCMD(0x79, sub=0x19)`；硬件失败则回滚兼容字段，避免 UI 显示未生效状态。
- 写 `fan-curve` 时，用户态和内核态都只接受固定 4 点 CPU/GPU 曲线；内核模块按原厂公式生成 `SetWMIPackage(14)` buffer，写入成功后再调用 `fan-mode custom`，因此 GUI 总览选择 `曲线 1/2/3` 才会真正应用到 EC。
- GUI 按钮高亮只读 `app_power_mode/app_fan_mode`，不再使用 `mode_status` 推导。
- GUI 的“风扇”页面编辑三组本地 CPU/GPU 曲线并保存到 app 配置；首页显示的 `曲线 1/2/3` 负责把对应曲线写入 EC。Linux 当前仍不写完整 AppSettings 风扇表持久镜像，只同步 `4:5=custom` 的受限模式读回。
- GUI 的“电池”页面可以编辑本地电池策略、充电阈值和低电量策略意图；当前不调用 Battery Saver/EnergySave 写接口，不写 EC，也不切换系统电源计划。

### 仍未完全确认的点
- Windows 原厂完整 AppSettings 存储由 AcpiBridge IOCTL `0x32240c` 管理；Linux 目前没有确认同等持久存储位置，因此受限 AppSettings 是运行时兼容镜像。
- CPU 温度的 `CalCPUTemp(TDP, raw)` 修正尚未完整复刻；当前只展示 EC raw 摄氏度候选。
- Linux GUI 已把当前可安全使用的 OEM 能力位映射为 UI 可见性规则：`PSF5 bit0` 控制电源模式按钮组，`PSF5 bit7` 控制风扇模式按钮组，`PSF2 bit15` 控制 Silent，`0x0D[0x0E] == 5` 控制 MaxQ。电池策略当前只有本地编辑；MUX、GPU/CPU OC 等仍只在高级页按能力位只读展示。
- MUX、GPU OC、CPU OC、电池节能、AntiDust 等高级写入虽然已有静态链路，但尚未做 Linux 实机逐项验证和失败恢复设计；暂不公开写入口。
- InstallShield 未解包服务组件可能包含 FnKey/PowerBiosServer 的最终电源落地逻辑；当前结论基于已解包 AppX、DLL 反汇编和 Linux 实机验证。
