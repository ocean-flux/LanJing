# Acknowledgements

`LanJing` 站在很多优秀开源项目的肩膀上。

这份清单主要记录两类来源:

1. 直接构成当前产品基础设施的核心框架与库
2. 在交互, 兼容格式, 阅读体验, 宿主层实现等方面对 `LanJing` 有明确借鉴或研究价值的开源项目

这不是一份完整的传递依赖清单. 更完整的依赖信息请参考 `package.json`, `src-tauri/Cargo.toml` 与各 crate 的 `Cargo.toml`.

## Core stack and UI foundation

- [Tauri](https://tauri.app/) - 跨平台宿主层, 窗口系统, 打包与平台能力接入的核心基础
- [Svelte](https://svelte.dev/) 和 [Vite](https://vite.dev/) - 主界面渲染与前端构建基础设施
- [Tailwind CSS](https://tailwindcss.com/) - 视觉系统与响应式样式基础
- [shadcn-svelte](https://www.shadcn-svelte.com/) - Svelte 组件组合方式与设计系统实践参考
- [SvelteFlow](https://svelteflow.dev/) - 规则图画布和节点连线交互基础

## Product architecture and implementation building blocks

- [Paraglide JS](https://paraglidejs.com/) - 前端编译式国际化基础
- [TanStack Query](https://tanstack.com/query/latest) - 查询状态与异步数据流参考
- [Monaco Editor](https://github.com/microsoft/monaco-editor) - 规则编辑器代码编辑体验
- [Pretext](https://github.com/chenglou/pretext) - 阅读排版测量与正文分页相关实验基础
- [xmloxide](https://github.com/jonwiggins/xmloxide) - 纯 Rust HTML / XML / XPath 解析能力基础
- [Project Fluent](https://projectfluent.org/) - Rust 规则平台诊断和报告本地化语义基础

## Compatibility and ecosystem inspirations

- [Legado / 阅读](https://github.com/gedoor/legado) - 书源生态, 规则导入兼容, 阅读场景长期是 `LanJing` 的重要参考来源之一
- [MacCMS10](https://github.com/magicblack/maccms10) - 影视资源站点生态兼容的关键参考

## Reader and app experience references

- [Readest](https://github.com/readest/readest) - 现代桌面阅读器在信息层级, 阅读姿态, 内容优先布局上的参考来源

## Host chrome and native window research references

下面这些项目不一定直接作为依赖引入, 但在 `LanJing` 自研桌面标题栏, 原生窗口控制与 Snap Layouts 研究过程中提供了重要思路:

- [tauri-plugin-frame](https://github.com/clarifei/tauri-plugin-frame)
- [tauri-plugin-decorum](https://github.com/clearlysid/tauri-plugin-decorum)
- [windows-rs](https://github.com/microsoft/windows-rs)

## Thank you

感谢所有维护者, 贡献者和社区使用者.

如果未来 `LanJing` 明确借鉴, 研究, 适配或吸收了新的开源项目思路, 我们会继续把它补充到这里.
