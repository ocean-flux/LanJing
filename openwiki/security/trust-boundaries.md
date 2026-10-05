---
type: "参考"
title: "Trust boundaries"
openwiki_generated: true
sources:
  - id: openwiki-source-f70faf819fb2edbb8e236f45
    resource: repo://docs/adr/0004-rule-first-open-extension-architecture.md
  - id: openwiki-source-00ff4b2512b6dbfa268cbfa4
    resource: repo://src-tauri/capabilities/default.json
  - id: openwiki-source-ca67060e890937010b96de80
    resource: repo://src-tauri/Cargo.toml
  - id: openwiki-source-4866e15f73219bace3482e99
    resource: repo://src-tauri/crates/lj-integration-tests/tests/legado_rule_system.rs
  - id: openwiki-source-378ef8c9992cfb062d1d9087
    resource: repo://src-tauri/crates/lj-node-http/src/processor/adapter.rs
  - id: openwiki-source-4ae975ffc729fc8e05df77f8
    resource: repo://src-tauri/crates/lj-node-http/src/processor/import.rs
  - id: openwiki-source-9e78ff08579c5d0a1877c1c8
    resource: repo://src-tauri/crates/lj-node-http/src/processor/redirect.rs
  - id: openwiki-source-fa8644f963882ace49f0d8b3
    resource: repo://src-tauri/crates/lj-node-http/src/processor/request.rs
  - id: openwiki-source-e3712091d151264c5b636d0c
    resource: repo://src-tauri/crates/lj-node-http/src/ssrf.rs
  - id: openwiki-source-c9a97bdea04d62635b90bf91
    resource: repo://src-tauri/crates/lj-node-http/tests/processor_test.rs
  - id: openwiki-source-0dcbf0271dec640cfb5161dc
    resource: repo://src-tauri/crates/lj-node-js/src/processor.rs
  - id: openwiki-source-2aa0c74bb87d98504fb9d6f3
    resource: repo://src-tauri/crates/lj-rule-model/src/definition/config.rs
  - id: openwiki-source-8c969793b07c46132d5a3d59
    resource: repo://src-tauri/crates/lj-rule-model/src/sensitive.rs
  - id: openwiki-source-32d4001fffac613566845710
    resource: repo://src-tauri/crates/lj-rule-system/src/system/error_mapping.rs
  - id: openwiki-source-3d616a9839cf698be82f0e92
    resource: repo://src-tauri/crates/lj-runtime/src/capability.rs
  - id: openwiki-source-8ece8d8ea6055cf2f800dcb4
    resource: repo://src-tauri/crates/lj-runtime/src/effect_registry.rs
  - id: openwiki-source-ba442c85857684a6872f9ae8
    resource: repo://src-tauri/crates/lj-runtime/src/effect/contracts.rs
  - id: openwiki-source-b5b31452901a813cae4d04db
    resource: repo://src-tauri/crates/lj-runtime/src/effect/witness.rs
  - id: openwiki-source-17f1cf15274491a45980b28f
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/dispatch.rs
  - id: openwiki-source-5c7572d3e7a28403c347b69c
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/loop_execution.rs
  - id: openwiki-source-25f9975ec0abf8c6031e92de
    resource: repo://src-tauri/crates/lj-storage/src/artifact.rs
  - id: openwiki-source-3a3fdfb5a00399ef7859b900
    resource: repo://src-tauri/crates/lj-storage/src/keyring_init.rs
  - id: openwiki-source-b03c20565000a097dbf231cf
    resource: repo://src-tauri/crates/lj-storage/src/repository/secret.rs
  - id: openwiki-source-d15a9d80a96c7517107f4457
    resource: repo://src-tauri/src/commands/state.rs
  - id: openwiki-source-0abfee918aaf0d7e3ea712fc
    resource: repo://src-tauri/tauri.conf.json
generated: { by: "pi", at: "2026-10-04T13:54:24.186Z" }
---


## 边界的形状

LanJing 的不可信输入只有四条, 且**全部经 Rust 收口**:

| 输入 | 收口点 | 强制手段 |
| --- | --- | --- |
| Legado/Maccms 规则文本 | `lj-importer` → `lj-compiler` 校验 | 结构校验 + capability 交集 |
| 深链/用户提供的导入 URL | `lj-node-http` 的 import façade | 同一套 SSRF 校验 + 2 MiB 上限 |
| 规则执行时的网络响应 | `lj-node-http` effect adapter | SSRF + 逐跳 pin + 流式上限 |
| 前端 IPC 参数 | Tauri 命令层 → `RuleSystem` | 类型化 DTO + capability/凭据策略 |

WebView 侧**不是**边界: `capabilities/default.json#L1-L16` 只给 `main` 窗口 `core:default`、`core:event:default`、`mcp-bridge:default`、`dialog:default`、`zustand:default`、`deep-link:default`、`opener:default`、`core:window:default` 与四个窗口动作, 没有 fs/shell/http 等插件权限——前端没有直接发网络请求的能力, 也没有 `.zshrc` 那类文件系统入口。

## 网络出口的 SSRF 不变量

**判定范围**由 `is_blocked_ip` 覆盖(`src-tauri/crates/lj-node-http/src/ssrf.rs#L70-L111`):

- IPv4: 环回、链路本地、私有网段、未指定、广播、`0.0.0.0/8`、CGNAT `100.64.0.0/10`, 以及两个云元数据端点 `169.254.169.254`(AWS)与 `100.100.100.200`(阿里云);
- IPv6: `::1`、`::`、`fe80::/10`、`fc00::/7`(ULA)、IPv4-mapped 与已废弃的 IPv4-compatible 形式。

**防 rebinding 的机制**写在模块文档里(`ssrf.rs#L6-L18`): 自行异步 DNS 解析(hickory, 外层 `tokio::time::timeout` 兑底 10s, `#L27-L29`)→ SSRF 校验 → IP 固定; HTTP 走 IP 直连 URL 以消除 TOCTOU 窗口, HTTPS 用 `ClientBuilder::resolve` 固定 IP 并保留 SNI 与证书验证。解析结果放在 `PinnedTarget`(`#L53-L66`), 同时用于 URL 改写与 witness 记录。DNS 解析器用 `OnceLock` 缓存, 初始化失败缓存错误而不是 panic(`#L34-L51`)。

**逐跳重校验**是 redirect 的核心不变量(`src-tauri/crates/lj-node-http/src/processor/redirect.rs#L1-L5`): 自动 redirect **永远关闭**(两个共享 client 都显式 `Policy::none()`, `src-tauri/crates/lj-node-http/src/processor/request.rs#L39-L59`), 循环里每跳重新 `validate_target` 并重建 client pin; 源码注释把它写成一句话——「每次 Location 都重新解析、DNS pin 和策略校验, 绝不复用旧 target」(`redirect.rs#L131-L132`)。跳数上限 5(`redirect.rs#L20`, 超出返回 `HttpRequestError::Redirect`), 每跳都写入 `HttpRedirectWitness`。

**体积与时间上限**: body 流式读取累计计数, 超 16 MiB 返回 `BodyTooLarge`(`request.rs#L25-L26`, `#L410-L416`); 连接超时 10s、单请求总超时 30s(`#L28-L31`)。

**违反后果**: 校验失败不是「返回一个错误响应」而是走两条不同路径——`Cancelled` 直接返回 `EffectError`(不产生可归档的 live 结果), 其余(含 `TargetValidation`、`Redirect`、`BodyTooLarge`)被归档为 `EffectOutput::Failure`(`src-tauri/crates/lj-node-http/src/processor/adapter.rs#L118-L138`), 也就是**失败本身是持久化的**, replay 会复现同一次失败而不是重发请求。这点很容易被误读为「失败被吞掉」, 实际是「失败被如实记录」。对外消息是固定的泛化文案(`adapter.rs#L142-L156`), 不携带底层 reqwest 文本。

**测试位置**: `src-tauri/crates/lj-node-http/tests/processor_test.rs#L25-L56` 的 `ssrf_block_loopback_v4`、`ssrf_block_aws_metadata`、`ssrf_block_alibaba_metadata`、`ssrf_block_rfc1918`、`ssrf_block_loopback_v6`、`ssrf_allow_public`, 加上 `#L548` 的 `plan_http_effect_records_real_manual_redirect_hop` 与 `#L266` 的 `convert_response_body_size_limit`。

**一处真实的边界弱化**: 生产 adapter 是 `HttpEffectAdapter::new()`, 而 `new_test()` 把 `ssrf_enabled` 设为 false 以允许环回(`adapter.rs#L28-L47`); 它是否被生产路径使用取决于 `RuleSystemConfig::desktop` 的 `local_fixture_http` 开关(false 时用 `HttpEffectAdapter::new()`)。因此 SSRF 是否真的开启, 由配置而非编译期决定——这是可测性带来的必要代价, 但意味着**任何把 `local_fixture_http` 打开的非测试构建都会关闭 SSRF**。

## 深链导入的独立 façade

`src-tauri/crates/lj-node-http/src/processor/import.rs` 是给导入地址用的受限拉取层, 它**复用**同一套 URL 校验、逐跳 DNS pin、手动 redirect 与流式读取(`#L1-L5`), 但有一组更紧的预算与自己的错误分类:

- 响应体硬上限 **2 MiB**(`#L17-L18`), 整次拉取(含 DNS、redirect、body)30s(`#L20-L21`);
- 错误分类把每一种失败收敛成一个稳定 code(`#L23-L50`): `import_src_url_invalid`、`import_src_target_blocked`、`import_src_timeout`、`import_src_request_failed`、`import_src_redirect_invalid`、`import_src_http_status`(只暴露状态码)、`import_src_body_too_large`、`import_src_invalid_utf8`;
- 注释明确它「不接触安装 grant、来源凭据或 `RuleSystem` candidate」, 且公开错误不携带 URL query、响应 body 或底层网络错误。

这条边界的存在意义是: 深链导入发生在**任何规则被信任之前**, 所以它既不能带凭据, 也不能借用规则的 capability。

## 敏感名的三种处置

`SensitiveNamePolicy`(`src-tauri/crates/lj-rule-model/src/sensitive.rs#L12-L75`)把 header 名分成三档:

| 处置 | 触发条件 | 语义 |
| --- | --- | --- |
| `Blocked` | `Proxy-Authorization`、`Set-Cookie` | 前者会改变代理 hop 的授权边界, 后者只属于 response, 二者作为请求 header 一律拒绝 |
| `Credential` | `Authorization`、`Cookie`, 以及名字含 `token` / `secret` / `api-key` / `apikey` | 只允许由 execution-only 凭据通道注入 |
| `Public` | 其余 | 可进入 witness(仍只存名字 + value hash) |

名称比较大小写不敏感, 并把 `_` 与 `-` 视为同一分隔符(`#L4`); `url_contains_sensitive_query_name` 只做 percent-decode 后检查参数**名**, 不保留也不返回参数值(`#L61-L75`)。

配套的两道闸:

- `forbidden_secret_header` 让凭据槽永远无法覆盖 `Host` / `Content-Length` / `Transfer-Encoding` / `Connection` / `Proxy-Authorization`(`request.rs#L162-L168`)——否则一个来源规则就能改写请求的寻址与长度语义;
- 凭据 header 插入时用 `HeaderValue::set_sensitive(true)` 标记(`request.rs#L149-L158`), 让 reqwest 自身的日志也不打印它。

**witness 的构造式脱敏**: `safe_request_headers` 过滤掉非 `is_safe_header_name` 与 `SensitiveNamePolicy::is_sensitive` 的名字, 只留小写名字与 BLAKE3 摘要并排序(`request.rs#L314-L343`); `HttpRequestWitness.safe_url` 在类型文档里被限定为「只能包含 scheme、host、port 与 path, 禁止 query、fragment 与 userinfo」(`src-tauri/crates/lj-runtime/src/effect/witness.rs#L31-L41`), 由 `safe_url()` 实际构造(`request.rs#L286-L312`, 同时拒绝带 userinfo 或非 http(s) 的 URL)。request body 默认被视为 secret, 只有未来有明确 proven-safe 合同的 adapter 才能降低等级(`witness.rs#L228-L231`)。

**测试位置**: `processor_test.rs#L400` 的 `shared_policy_blocks_unsafe_request_credentials_and_redacts_response_witness`、`#L475` 的 `sensitive_plan_header_is_rejected_before_request_and_never_enters_witness`、`#L493` 的 `plan_http_effect_marks_request_body_secret_and_witness_redacted`; 以及 `sensitive.rs#L157-L195` 的策略单测(含 percent-decode 绕过尝试 `#L188`)。

## 脱敏不变量与它的证据

`lj-rule-system` 的错误映射模块把不变量写成头注释: **任何 message、diagnostic 或 trace 都不得包含 body、cookie、token、完整 URL query、Plan JSON 或 opaque payload**(`src-tauri/crates/lj-rule-system/src/system/error_mapping.rs#L1-L5`)。runtime 侧对 `EffectError::message` 有同样的约束(`src-tauri/crates/lj-runtime/src/effect/contracts.rs#L265`), 并明确失败仍须经 durable archive 以免 replay 走偏(`#L68`)。

这条不变量有跨层测试覆盖, 而不只是注释:`lj-integration-tests/tests/legado_rule_system.rs` 用 `source-static-secret` 与 `?q=修罗` 两个哨兵值, 断言序列化后的 HTTP witness、投递事件流都不包含它们(`#L520-L530`、`#L742-L747`), 并进一步断言临时目录里**不存在明文**(`assert_no_plaintext_secret`, `#L748-L749`)。这类断言的价值在于它验证的是「任何一处忘记脱敏」都会失败, 而不是只测某个函数。

## 凭据的静态存储

凭据/密钥在磁盘上的形态由 `lj-storage/src/artifact.rs` 与 `repository/secret.rs` 定义, 设计目标是让密文文件本身**不能成为 oracle**:

- 文件版本常量 `VAULT_SECRET_FILE_VERSION = 2`(`src-tauri/crates/lj-storage/src/artifact.rs#L21`), 信封形如 `[version][12B nonce][ciphertext]`;
- `key_verifier` 拒绝任何长度不等于 32 字节的密钥(`StorageError::KeyLost`), 否则返回 domain separation 前缀 `lanjing-vault-key-verifier-v1` 加密钥的 BLAKE3, 文档说明它不能用于推导任何文档或凭据明文(`artifact.rs#L127-L137`);
- `write_secret` 生成随机 `SecretArtifactId` 与随机 UUID 派生的 blob locator(**不是内容哈希**), 原子写入信封, 并把 `ciphertext_hash` 设为整个信封的 BLAKE3——因此该哈希不能形成可预计算的明文对照(`artifact.rs#L140-L171`);
- `read_secret_artifact` 先校验哈希与版本字节再解密(`artifact.rs#L172-L196`)。

SQLite 侧只存随机 `SecretArtifactId`、随机 blob locator、key ID 与 ciphertext hash; **owner 行是引用计数的唯一证据**(`src-tauri/crates/lj-storage/src/repository/secret.rs#L1-L10` 的文件级文档), 启动恢复里 `validate_secret_ref_counts`、`purge_zero_ref_artifacts`、`purge_zero_ref_secrets` 按序执行以收敛计数漂移。主密钥来自 keyring: `ensure_default_keyring_store` 复用已安装的 keyring-core 默认 store, 否则安装平台 store, 并把不可用映射为 `KeyringUnavailable`、临时锁映射为 `KeyringLocked`(`src-tauri/crates/lj-storage/src/keyring_init.rs#L34-L45`)。

## 执行侧的资源上限与沙箱

| 约束 | 值 | 位置 | 违反后果 |
| --- | --- | --- | --- |
| QuickJS 内存 | 16 MiB | `src-tauri/crates/lj-node-js/src/processor.rs#L26`, `#L263` | 脚本抛错, 归入 `QuickJsOutput::Error` 并被归档 |
| QuickJS 超时 | 5000 ms | `processor.rs#L29`, `#L99` | 由 watchdog 触发 QuickJS interrupt |
| Loop 迭代 | `min(node.max_iterations, 256)` | `src-tauri/crates/lj-rule-model/src/definition/config.rs#L12`, `src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/loop_execution.rs#L51` | `InputTypeMismatch`/上限失败, 执行终止 |
| HTTP body | 16 MiB(导入 2 MiB) | `request.rs#L25-L26`, `import.rs#L17-L18` | `BodyTooLarge` → 归档为 Failure |
| redirect 跳数 | 5 | `redirect.rs#L20` | `Redirect` → 归档为 Failure |
| capability | host ∩ source grant ∩ invocation | `src-tauri/crates/lj-runtime/src/capability.rs#L38`, `plan_runtime/scheduler/dispatch.rs#L152` | `CapabilityDenied`, 且被映射为 `RuleErrorStage::Capability`(`error_mapping.rs#L17-L21`) |
| effect handler | frozen registry, 无回退 | `lj-runtime` 的 `EffectRegistry` / `FrozenEffectRegistry`(键是 `Rule Contract` 的 `EffectKind`) | `OperationUnavailable` 而不是静默换一个 handler(`error_mapping.rs#L21-L25`) |

非 Send 的 rquickjs 对象只在 `spawn_blocking` 闭包内创建与销毁, watchdog 轮询取消令牌触发中断并 join 后才返回(见运行时页), 这是「沙箱不泄漏线程」的部分。

## 前端侧的最小暴露

CSP 是 `default-src 'self'; img-src 'self' data: https:; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; base-uri 'self'; object-src 'none'`(`src-tauri/tauri.conf.json#L30-L36`)。`connect-src 'self'` 让 WebView 无法发起跨源 fetch/XHR; 唯一的对外取数口是 `img-src` 里的 `https:`。

这个例外正是当前最大的缺口所在: ADR 0007 要求所有远程资产经 `lanjing://` 网关代理并复用 SSRF 防护, 但**该协议未注册、CSP 也没有 `lanjing:`**(全仓库检索 `register_uri_scheme` / `lanjing://` 无命中)。也就是说现在若要有图片, 走的是 CSP 放行的 `https:` 直连路径——而那恰好是 ADR 0007 判定「必然 403 且应视为缺陷」的路径。二者尚未收敛。

## 已决定但尚未实现的边界

- **资产网关**(ADR 0007): 计划成为资产访问的唯一关口并复用 `ssrf.rs`, 当前不存在。设计上它还是防盗链头的注入点, 因此也承担「来源凭据只在这一处使用」的角色。
- **effect 注册边界**: 今天运行时只注册内置的三种 effect handler(HTTP / QuickJS / Extract), 注册与查找键是 Rule Contract 的 `EffectKind`, 未注册 kind 由 dispatch 转成稳定的 `OperationUnavailable`(`src-tauri/crates/lj-runtime/src/effect_registry.rs#L61-L107`)。通用 plugin system(通用 PluginHost、plugin catalog、跨插件 service、registration lease、动态 Rust ABI、第三方 executable plugin SDK)按 ADR 0004 第 7 节属**一期明确不建设**; 原来的 `lj-plugin-contract` 与 `PluginHost` / `FrozenRegistry`(含只声明不比对的 `HOST_CONTRACT_VERSION`)已**删除**(`docs/adr/0004-rule-first-open-extension-architecture.md#L104-L114`)。
- **凭据的产品边界**: 「凭证明文永远不进 `tauri-store`」是仓库约定, 但没有测试或类型强制; 它的实际保障来自「前端根本拿不到明文」这条数据流(明文只在 Rust 侧的 artifact 端口与 effect adapter 出现)。

## 不变量总表

| 不变量 | 违反时的可见后果 | 证据 |
| --- | --- | --- |
| 内网/元数据地址不可达 | 请求失败并被归档为 Failure, 不重发 | `ssrf.rs#L70-L111`, `processor_test.rs#L25-L56` |
| redirect 每跳重新校验与 pin | 潜在 DNS rebinding 取到新的内网地址 | `redirect.rs#L131-L132` |
| 凭据不进 witness / 事件 / 磁盘明文 | 集成测试的哨兵值断言失败 | `processor_test.rs#L400`、`legado_rule_system.rs#L742-L749` |
| 敏感 header 名不可被规则覆盖 | 规则可改写寻址或走私代理授权 | `request.rs#L162-L168` |
| 密文文件不构成明文 oracle | 随机 blob locator 与信封哈希被替换成内容哈希 | `artifact.rs#L140-L171` |
| 引用计数决定密文生命周期 | 孤儿密文残留或活引用的密文被删 | `secret.rs` 文件级文档 + 启动 `validate_secret_ref_counts` |
| 执行资源有硬上限 | 恶意规则造成内存/时间耗尽 | `processor.rs#L26-L29`、`config.rs#L12`、`request.rs#L25-L26` |
| 前端无跨源出网能力 | CSP 变更引入 `connect-src` 放行 | `tauri.conf.json#L30-L36` |

## 未验证

- 真实 DNS 环境下的 pin 行为、TLS 证书校验路径没有端到端测试(全部走 wiremock), 只有 `PinnedTarget` 的构造逻辑与 `is_blocked_ip` 的单测。
- 平台 keyring 不可用/被锁的降级路径只有类型与映射代码, 无测试(测试用 `keyring_core::mock`)。
- AES-GCM 的 nonce 复用风险依赖「每次写入都新生成随机 nonce」这一实现事实, 没有专门的重复 nonce 检测或属性测试; 加密库与模式选择来自依赖实现, 未在本仓库内审计。
- `local_fixture_http` 开关在生产配置里是否可能被误开, 没有编译期或启动期断言。
