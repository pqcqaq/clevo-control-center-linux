# 任务计划：代码质量与编辑器警告审计

## 目标
系统检查 Rust 用户态、C 内核模块、VS Code/rust-analyzer 配置与当前未提交改动，区分真实缺陷、编译器警告、Clippy 可维护性问题和编辑器误报；修复明确且低风险的问题，并完成本地与远端验证。

## 当前阶段
阶段 11

## 各阶段

### 阶段 1：建立诊断基线
- [x] 检查仓库结构、工具链和编辑器配置
- [x] 运行 Rust fmt/check/test/clippy 全量检查
- [x] 运行远端内核模块警告级构建
- **状态：** complete

### 阶段 2：代码结构审计
- [x] 检查大文件、大函数、重复代码和边界处理
- [x] 检查 unsafe、unwrap、错误吞噬和进程调用
- [x] 核对 VS Code 警告来源
- **状态：** complete

### 阶段 3：修复
- [x] 修复确认的编译/Clippy/语法问题
- [x] 增补必要的项目级 VS Code 配置
- [x] 避免改动硬件协议与未提交功能语义
- **状态：** complete

### 阶段 4：验证
- [x] 本地 fmt/check/clippy/test 全通过
- [x] 远端 Linux release 与内核模块构建通过
- [x] 检查 diff 和远端运行状态
- **状态：** complete

### 阶段 5：交付
- [x] 按严重度汇总代码质量发现
- [x] 说明已修复项、剩余风险和编辑器设置
- **状态：** complete

### 阶段 6：UI 页面模块化
- [x] `pages.rs` 仅保留页面分发与小型系统页面
- [x] 总览、灯光、显卡页面按业务职责拆分
- [x] 测试跟随所属页面模块
- **状态：** complete

### 阶段 7：DCHU 模块化
- [x] 分离硬件解析、控制 CLI 与核心领域类型
- [x] 测试跟随解析/CLI 模块或保留集中集成测试
- [x] 保持现有公开 API 与硬件协议不变
- **状态：** complete

### 阶段 8：应用层与错误处理复审
- [x] 评估 `ui/app.rs` 是否存在可独立业务职责
- [x] 收敛值得处理的静默错误，保留明确 best-effort 路径
- [x] 不创建只包装一两行的小 helper 模块
- **状态：** complete

### 阶段 9：全量验证与远端部署
- [x] 本地 fmt/check/严格 Clippy/test 全通过
- [x] 远端 check/test/release 与内核模块 W=1 通过
- [x] 重启远端 GUI 并确认服务不受影响
- **状态：** complete

### 阶段 10：结构交付
- [x] 更新 README 目录结构与维护约定
- [x] 汇总新结构、剩余债务和后续扩展入口
- **状态：** complete

### 阶段 11：硬件后端抽象
- [x] 定义不泄露 Linux `/proc` 细节的硬件后端契约
- [x] 以现有灯光与 DCHU I/O 实现 Linux 后端
- [x] 让 GUI、后台服务和 CLI 统一通过后端调用
- [x] 保持当前 Linux 行为、白名单校验与硬件协议不变
- [x] 完成本地与远端编译/测试验证
- [ ] 远端图形认证加载内核模块并恢复 GUI/服务运行态
- **状态：** blocked（远端 `zenity`/polkit 等待用户认证，`sudo -n` 不可用）

## 审计边界
- 保留用户现有未提交改动，不回滚或覆盖无关内容。
- 不改变 DCHU/EC 写入协议、能力位解释和硬件控制语义，除非发现明确 bug。
- 不为了消除警告而添加大范围 `allow`。
- 按业务职责拆模块，不为少量代码创建 `helpers/utils/common` 一类无语义容器。

## 遇到的错误
| 错误 | 尝试次数 | 解决方案 |
|------|---------|---------|
| 旧计划停留在早期键盘灯移植阶段 | 1 | 更新为当前代码质量审计计划，历史保留在日志文件 |
| 并行基线命令因 Clippy 非零退出只显示失败输出 | 1 | 记录 Clippy 结果，后续把其余检查拆开复核，不重复同一调用 |
| Windows 环境没有 `head` 命令，文件行数排序命令失败 | 1 | 后续不截断输出或使用已有 `bat/rg/fd` 组合，不重复该命令 |
| Windows 环境没有 `wc` 且 `sort` 被 PowerShell 别名接管 | 1 | 改用 `rg -c '^'` 统计行数，避免依赖 Unix 管道 |
| 本机未安装 `cargo-outdated` | 1 | 不为审计临时安装额外工具；跳过过期版本扫描，继续 Clippy 与依赖树检查 |
| 记录 `cargo-outdated` 缺项时补丁因空格差异未匹配 | 1 | 先用 `rg` 精确读取目标行，再按实际文本追加 |
| 远端 Kali 未安装 `cargo fmt` / `cargo clippy` 子命令 | 1 | 不修改系统包；远端以 check/test/release 验证，共享 rust-analyzer 后台检查使用 `cargo check` |
| 多文件 `scp` 把 `.vscode/settings.json` 按 basename 放到远端仓库根目录 | 1 | 验证内容后删除误放副本，单独同步到远端 `.vscode/settings.json` |
| 远端 grep 的 JSON 引号在 PowerShell/zsh 边界被拆分 | 1 | 改用 Python JSON 解析直接校验键值，并单独复核被遮住的本地结果 |
| 规划完整性脚本未识别中文自定义阶段格式，显示 0/0 | 1 | 以计划中五个阶段的显式 complete 状态为准；不影响代码验证结果 |
| 整文件读取输出被工具截断，截断标记写入拆分后的 `gpu.rs` | 1 | 远端原文件未同步且完整；改从临时副本按小段读取，通过 apply_patch 重建，不再读取整文件 |
| 阶段状态补丁因上下文顺序不匹配未应用 | 1 | 精确读取计划位置后拆成独立小补丁更新 |
| 已知 PowerShell 环境无 `head` 却再次用于截断 rg 输出 | 2 | 停止使用 head，直接使用 rg 完整输出 |
| DCHU 拆分后快照 impl 仍依赖 io 私有函数，测试也缺少显式模块导入 | 1 | 将快照构造归入 io；测试显式导入 io/cli 测试接口，收紧根 re-export |
| 初次调用点搜索漏掉 advanced.rs 的分组 import | 1 | 保留 `fan_rpm_from_tach` 作为根模块公开换算接口，其余解析器保持内部可见 |
| Windows 下 `rg` 不展开 `src/ui/pages/*.rs` / `src/ui/app/*.rs` 通配路径 | 2 | 后续直接把对应目录交给 `rg`，不依赖 shell glob |
| UI 根模块误读为 `src/ui.rs` | 1 | 实际入口是 `src/ui/mod.rs`，后续按目录模块结构读取 |
| `app::persistence` 中保存方法的 `pub(super)` 对 UI 兄弟模块不可见 | 1 | 仅将页面实际调用的两个方法改为 `pub(in crate::ui)`，同步细节继续保持在 app 子树内 |
| 远端 `192.168.4.70:22` 首次 SSH 连接超时 | 1 | 先诊断局域网与端口连通性，再使用短连接超时重试，不重复长时间阻塞 |
| 远端主机 ping 无响应且三次 SSH 均超时 | 3 | 本地工作已全部完成；等待服务器上线或恢复同一局域网后，从同步与 Linux 验证继续 |
| 远端存在未引用的 `src/pages.rs` 旧页面副本 | 1 | 已确认本地不存在且根模块未引用；部署时删除该历史同步残留 |
| 远端测试命令误用了 `/home/qcqc` 路径 | 1 | 正确仓库路径为 `/home/qcqcqc/clevo-control-center-linux`，重新执行测试 |
| PowerShell 提前展开远端进程查询中的 `$()` | 1 | 用 PowerShell 单引号包住整段远端脚本，避免本地解释 Bash/zsh 变量 |
| zsh 命令替换未按换行拆分多个 PID | 1 | 改用 `pgrep | while read` 逐行处理 PID，不依赖 shell word splitting |
| 硬件后端首轮编译仍有旧模式类型和 I/O 可见性引用 | 1 | 将 proc 写入口收窄为 crate 可见，并把测试/页面选中值迁移到领域枚举 |
| 曲线序列化迁移后仍是父模块可见 | 1 | 仅提升为 `pub(crate)` 供 Linux 后端调用，不扩大二进制外部 API |
| 远端 GUI 重启前发现后台服务已不存在 | 1 | 安全检查在杀 GUI 前退出；检查 PID/锁与日志后启动缺失服务，再重启 GUI |
| 远端内核模块未加载且非交互 sudo 需要密码 | 1 | 新 GUI 已启动并显示图形认证窗口；等待用户点击“立即处理”并完成认证 |
| 新增 GPU MUX 写能力后模块 API 仍为 2 | 1 | 内核模块和加载器最低要求同步升到 API 3，确保旧 API 2 自动触发更新 |
| 模块源码被 scp 到仓库根目录且 SSH 启动 GUI 的 pkexec 无 `/dev/tty` | 1 | 源码改同步到 `module/` 并验证 `modinfo version: 3`；改用远端图形终端执行 sudo 更新 |
