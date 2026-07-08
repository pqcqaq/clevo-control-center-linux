# 任务计划：Clevo/Insyde 键盘灯 Linux 移植

## 目标
在 Linux 笔记本上实现一个可测试的键盘 RGB 设置工具，复刻 Windows C# 程序通过 `InsydeDCHU.dll` 设置键盘灯颜色的能力。

## 当前阶段
阶段 5

## 各阶段

### 阶段 1：需求与发现
- [x] 理解用户意图
- [x] 确定 Windows 源码调用方式
- [x] 初步逆向 `InsydeDCHU.dll` 的关键 GUID、IOCTL 和参数
- [x] 将发现记录到 `findings.md`
- **状态：** complete

### 阶段 2：Linux 接口定位
- [x] 检查 Linux 上是否已有 Clevo/Tuxedo/WMI/sysfs 键盘灯接口
- [x] 搜索 ACPI 表中是否存在 DLL 使用的 `_DSM` GUID
- [x] 反编译 ACPI 表，定位具体设备路径和 `_DSM` 参数语义
- **状态：** complete

### 阶段 3：实现
- [x] 选择低风险调用方式
- [x] 创建 Linux 测试工具
- [x] 支持单次设置三区颜色
- **状态：** complete

### 阶段 4：测试与验证
- [x] 在笔记本上编译
- [x] 小范围测试单色写入
- [x] 记录返回值、内核日志
- **状态：** complete

### 阶段 5：交付
- [ ] 整理用法
- [ ] 说明风险边界和下一步
- [ ] 给用户交付路径和测试结论
- **状态：** in_progress

## 关键问题
1. Linux 上应通过现成内核接口、`acpi_call`，还是自写小内核模块调用该 `_DSM`？
2. ACPI `_DSM` 所在设备路径是什么？
3. `_DSM` 函数 `0x67` 在 Linux 侧直接调用是否与 Windows DLL 行为一致？

## 已做决策
| 决策 | 理由 |
|------|------|
| 不修改原 C# 项目，单独创建 Linux 移植项目 | 避免污染原始 Windows 示例代码 |
| 先定位 ACPI/WMI 接口，再写调用工具 | 键盘灯属于固件/EC 操作，先确认路径可降低风险 |
| 不直接使用 `acpi_call` 现成包 | 该包输入端不能构造 `_DSM` 需要的 package 参数 |
| 创建最小外部内核模块 | 可精确传 `Arg3 = Package(Buffer)`，并限制接口为单次写入 |
| 不做持久安装或开机自启 | 当前仍处于硬件验证阶段，保持系统可回退 |

## 遇到的错误
| 错误 | 尝试次数 | 解决方案 |
|------|---------|---------|
| PowerShell 提前展开远端 Bash 的 `$()` 和 `$1` | 2 | 后续使用单引号模板、占位符或 base64 传输脚本 |
| 普通用户无法读取 `/sys/firmware/acpi/tables` | 1 | 改用 sudo 读取 |
