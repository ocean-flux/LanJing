//! Legado 宿主 API bridge — 在 rquickjs 中注册 Legado 约定的全局变量和函数。
//!
//! 首刀实现：result/JSON.stringify 由 rquickjs 内置支持，无需额外注入。

/// 在 JS 上下文中初始化 Legado 约定的全局变量。
///
/// 首刀为空操作 —— rquickjs 的 eval 自动处理 `var` 声明，
/// JSON 对象由 rquickjs 内置提供。
///
/// # Panics
///
/// 理论上不会 panic，若 rquickjs 内部状态损坏可能 panic。
pub fn init_legado_globals(_ctx: &rquickjs::Context, _page: Option<u32>, _key: Option<&str>) {
    // Legado 约定 @js: 块中 `var result=[]` 然后 push，
    // rquickjs 的 eval 自动处理 var 声明，不需要预注入。
    // 但需要确保 JSON 对象可用（rquickjs 内置）。
}

/// 从 JS 上下文提取 result（JSON 字符串）。
///
/// 执行 `JSON.stringify(result)` 并返回字符串。
///
/// # Errors
///
/// 返回 [`crate::error::JsError::EvalError`] 当 JS 执行失败。
pub fn extract_result(ctx: &rquickjs::Context) -> Result<String, crate::error::JsError> {
    ctx.with(|ctx| {
        ctx.eval("JSON.stringify(result)")
            .map_err(|e| crate::error::JsError::EvalError(e.to_string()))
    })
}
