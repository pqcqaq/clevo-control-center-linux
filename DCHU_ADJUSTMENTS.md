# DCHU 可调整项测试记录

本文档记录当前机器上已确认或推断可执行的 DCHU 调整项。除只读命令外，所有写入命令都需要 root 权限；实验写入还需要 `--i-understand`。

## 已实测可用

| 能力 | CLI | DCHU 路径 | 说明 |
|------|-----|-----------|------|
| 读取实时状态 | `clevo-control-center dchu status` | `/proc/clevo_dchu_status` / `0x0C` | 返回风扇转速、部分电池/温度原始字段；优先走只读状态节点，通常不需要 root。当前样本约 `rpm1=588`、`rpm2=663`、`rpm3=0`。 |
| 读取风扇表 | `sudo clevo-control-center dchu fan-table` | `0x0D` | 返回键盘颜色、亮度原始值、`FANQ`、`KBTP`、Fan1/Fan2/Fan3 温度/占空比表。 |
| 读取能力位 | `sudo clevo-control-center dchu caps` | `0x10/0x52/0x60/0x7A` | 当前返回 `0x93`、`0x04680025`、`0x021c`、`0x70020053`。 |
| 原始读取 | `sudo clevo-control-center dchu raw-get 0x0d` | 任意 read function | 直接输出固件返回的 integer 或 buffer。 |
| 键盘 RGB | GUI 或 `/proc/clevo_kbd_led` | `0x67` | 已长期验证，支持 `f0/f1/f2`，模块也允许传 `f3..f6`。 |
| 键盘亮度 | `sudo clevo-control-center dchu kbd-brightness <0..9> --i-understand` | `0x67` 子命令 `0x0D` | 已实测 `0` 和 `9` 都返回成功。`fan-table` 回读的 `keyboard_brightness_raw` 仍为 `0`，所以亮度方向和是否持久化以肉眼观察为准。 |

## 实验可调，暂不建议日常使用

| 能力 | CLI | DCHU 路径 | 风险 |
|------|-----|-----------|------|
| 电源/性能档位 | `sudo clevo-control-center dchu power-mode <0..3> --i-understand` | `0x79` / sub `0x19` | 会改 `EC.CPCM`、`DTTF`、平台性能状态并触发 SMI。参考 opencontrol，`0/1/2/3` 对应 `Quiet/Powersaving/Performance/Entertainment`。 |
| 风扇模式 | `sudo clevo-control-center dchu fan-mode <mode> --i-understand` | `0x79` / sub `0x01` | 参考 opencontrol/opendchu，支持 `auto/max/silent/maxq/custom/turbo`，对应 `0/1/3/5/6/7`。会直接改 EC 风扇策略，测试时要观察温度和转速。 |
| 原始 DWORD 写入 | `sudo clevo-control-center dchu raw-set-dword <function> <u32> --i-understand` | 任意 write function | 可以写 EC/固件状态，只适合复现已确认 payload。 |
| 原始 buffer 写入 | `sudo clevo-control-center dchu raw-set <function> <hex-bytes> --i-understand` | 任意 write function | 同上，payload 长度和偏移写错可能导致异常状态。 |
| 风扇曲线写入 | `sudo clevo-control-center dchu fan-curve-set '<hex>' --i-understand` | `0x0E` | 会写 Fan1/Fan2/Fan3 曲线字段。当前还没有完整确认 `F1R1..F3R3` 的安全默认值，不建议随便改。 |

## 逆向/搜索结论

- `opencontrol` 的 `PowerMode.cs` 使用 `SetDCHU_Data(121, { value, 0, 0, 25 })` 写电源模式，枚举为 `0=Quiet`、`1=Powersaving`、`2=Performance`、`3=Entertainment`。
- `opencontrol` 的 `Fan.cs` 使用 `SetDCHU_Data(121, { value, 0, 0, 1 })` 写风扇模式，常见枚举为 `0=auto`、`1=max`、`3=silent`、`5=maxq`、`6=custom`、`7=turbo`。
- `opendchu` 的 Linux DCHU 实现也把 `_DSM 121` 用作风扇模式入口，并把 `_DSM 12` 用作风扇 RPM、PWM duty、温度等状态读取入口。
- 本机 DSDT 反编译记录显示 `SCMD(0x79)` 的 `sub=0x19` 会写 `EC.CPCM`、`EC.APRD`、`EC.FCMD=0xD8`，并触发 `PRM0=0x11; PRM1=mode; SSMP=0xC0`；`APPM` 映射表为 `[0x02, 0x03, 0x01, 0x00]`。
- 参考来源：
  - `https://github.com/douweschulte/opencontrol`
  - `https://raw.githubusercontent.com/douweschulte/opencontrol/master/PowerMode.cs`
  - `https://raw.githubusercontent.com/douweschulte/opencontrol/master/Fan.cs`
  - `https://github.com/hUwUtao/opendchu`

## 当前风扇表样本

```text
status:
  rpm1: 587
  rpm2: 702
  rpm3: 0

fan1:
  step1: temp=40 duty=81  (32%)
  step2: temp=60 duty=163 (64%)
  step3: temp=80 duty=204 (80%)
  step4: temp=100 duty=255 (100%)
fan2:
  step1: temp=40 duty=81  (32%)
  step2: temp=60 duty=163 (64%)
  step3: temp=80 duty=204 (80%)
  step4: temp=97 duty=255 (100%)
fan3:
  all zero on this machine
```

## 本次测试记录

```text
sudo clevo-control-center dchu status
sudo clevo-control-center dchu fan-table
sudo clevo-control-center dchu caps
clevo-control-center dchu kbd-brightness 0
  -> blocked: dangerous write requires --i-understand
sudo clevo-control-center dchu kbd-brightness 0 --i-understand
  -> integer 0x67
sudo clevo-control-center dchu kbd-brightness 9 --i-understand
  -> integer 0x67
sudo clevo-control-center dchu power-mode 0 --i-understand
  -> d7_fbuf: 0x80, rpm around 980/985
sudo clevo-control-center dchu power-mode 1 --i-understand
  -> d7_fbuf: 0x02, rpm around 980/985
sudo clevo-control-center dchu power-mode 2 --i-understand
  -> d7_fbuf: 0x08, rpm gradually dropped toward 750/750 in the sampled run
sudo clevo-control-center dchu power-mode 3 --i-understand
  -> d7_fbuf: 0x02, rpm around 760-810 in the sampled run
clevo-control-center dchu fan-mode auto
  -> blocked: dangerous write requires --i-understand
sudo clevo-control-center dchu fan-mode auto --i-understand
  -> integer 0x79; later status sample rpm1=651 rpm2=634 rpm3=0
```

## 建议测试顺序

1. 先跑 `status`，确认 `/proc/clevo_dchu_status` 可用；再用 root 跑 `fan-table`、`caps`，确认 `/proc/clevo_dchu` 调试接口可用。
2. 用 `kbd-brightness 0..9` 做低风险写入测试，肉眼确认亮度方向。
3. 如要测试 `power-mode`，只在插电、负载较低、可观察风扇和温度时切换，并记录每个档位的 `status`。
4. 如要测试 `fan-mode`，优先从 `auto` 或当前可接受的低风险档位开始，避免长时间停在 `silent/max/custom`。
5. 暂不要写风扇曲线，除非先备份原始 DSDT/当前状态，并明确每个 payload 字段。

## 仍需确认

- `0x0E` payload 中 `F1R1..F3R3` 的实际含义和安全默认值。
- `fan-mode` 各档位在本机上的实际转速/噪音/温度表现。
- 电池充电阈值相关命令可能在 `0x76`，但当前没有暴露为 CLI 友好命令。
