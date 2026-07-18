//! 存储侧类型隔断 ID（原 `Repository` 泛型 trait 已删除）。

/// 类型隔断 ID(防 ID 跨类型混用)。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoId<T> {
    /// ID 字符串。
    pub id: String,
    /// PhantomData 标记。
    #[doc(hidden)]
    pub _marker: std::marker::PhantomData<T>,
}

impl<T> RepoId<T> {
    /// 创建新的 `RepoId`。
    #[must_use]
    pub fn new(id: String) -> Self {
        Self {
            id,
            _marker: std::marker::PhantomData,
        }
    }
}
