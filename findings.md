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
- 模块暴露 `/proc/clevo_kbd_led`，权限为 `root:root 0644`。
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
