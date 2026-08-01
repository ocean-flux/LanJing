//! `SeaORM` 2.0 连接、事务与参数化 statement 基础设施。

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, DbErr, ExecResult, FromQueryResult, QueryResult, Statement,
    TransactionOptions, TransactionTrait, Value,
};

use crate::types::{StorageConfig, StorageError};

enum SessionBackend {
    Pool(DatabaseConnection),
    Transaction(Option<DatabaseTransaction>),
}

/// writer actor 或 read pool 借用的 async `SeaORM` session。
pub(crate) struct DatabaseSession {
    backend: SessionBackend,
}

pub(crate) type TransactionFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, StorageError>> + Send + 'a>>;

impl DatabaseSession {
    pub(crate) async fn connect(path: &Path, read_only: bool) -> Result<Self, StorageError> {
        let connection = connect_pool(path, read_only, 1).await?;
        Ok(Self::from_connection(connection))
    }

    pub(crate) fn from_connection(connection: DatabaseConnection) -> Self {
        Self {
            backend: SessionBackend::Pool(connection),
        }
    }

    pub(crate) fn connection(&self) -> Option<&DatabaseConnection> {
        match &self.backend {
            SessionBackend::Pool(connection) => Some(connection),
            SessionBackend::Transaction(_) => None,
        }
    }

    pub(crate) async fn execute(&self, statement: Statement) -> Result<ExecResult, DbErr> {
        match &self.backend {
            SessionBackend::Pool(connection) => connection.execute_raw(statement).await,
            SessionBackend::Transaction(Some(transaction)) => {
                transaction.execute_raw(statement).await
            }
            SessionBackend::Transaction(None) => {
                Err(DbErr::Custom("transaction 已结束".to_string()))
            }
        }
    }

    pub(crate) async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        match &self.backend {
            SessionBackend::Pool(connection) => connection.execute_unprepared(sql).await,
            SessionBackend::Transaction(Some(transaction)) => {
                transaction.execute_unprepared(sql).await
            }
            SessionBackend::Transaction(None) => {
                Err(DbErr::Custom("transaction 已结束".to_string()))
            }
        }
    }

    async fn query_one(&self, statement: Statement) -> Result<Option<QueryResult>, DbErr> {
        match &self.backend {
            SessionBackend::Pool(connection) => connection.query_one_raw(statement).await,
            SessionBackend::Transaction(Some(transaction)) => {
                transaction.query_one_raw(statement).await
            }
            SessionBackend::Transaction(None) => {
                Err(DbErr::Custom("transaction 已结束".to_string()))
            }
        }
    }

    async fn query_all(&self, statement: Statement) -> Result<Vec<QueryResult>, DbErr> {
        match &self.backend {
            SessionBackend::Pool(connection) => connection.query_all_raw(statement).await,
            SessionBackend::Transaction(Some(transaction)) => {
                transaction.query_all_raw(statement).await
            }
            SessionBackend::Transaction(None) => {
                Err(DbErr::Custom("transaction 已结束".to_string()))
            }
        }
    }

    pub(crate) async fn immediate_transaction<T: Send>(
        &self,
        operation: impl for<'a> FnOnce(&'a mut Self) -> TransactionFuture<'a, T>,
    ) -> Result<T, StorageError> {
        self.run_transaction(true, operation).await
    }

    pub(crate) async fn transaction<T: Send>(
        &self,
        operation: impl for<'a> FnOnce(&'a mut Self) -> TransactionFuture<'a, T>,
    ) -> Result<T, StorageError> {
        self.run_transaction(false, operation).await
    }

    async fn run_transaction<T: Send>(
        &self,
        immediate: bool,
        operation: impl for<'a> FnOnce(&'a mut Self) -> TransactionFuture<'a, T>,
    ) -> Result<T, StorageError> {
        let options = TransactionOptions {
            sqlite_transaction_mode: immediate.then_some(sea_orm::SqliteTransactionMode::Immediate),
            ..Default::default()
        };
        let transaction = match &self.backend {
            SessionBackend::Pool(connection) => connection.begin_with_options(options).await,
            SessionBackend::Transaction(Some(parent)) => parent.begin_with_options(options).await,
            SessionBackend::Transaction(None) => {
                Err(DbErr::Custom("transaction 已结束".to_string()))
            }
        }?;
        let mut child = Self {
            backend: SessionBackend::Transaction(Some(transaction)),
        };
        let result = operation(&mut child).await;
        let SessionBackend::Transaction(Some(transaction)) = child.backend else {
            return Err(StorageError::Database("transaction 状态损坏".to_string()));
        };
        match result {
            Ok(value) => {
                transaction.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

pub(crate) async fn connect_pool(
    path: &Path,
    read_only: bool,
    max_connections: u32,
) -> Result<DatabaseConnection, StorageError> {
    let mode = if read_only { "ro" } else { "rwc" };
    let normalized = path.to_string_lossy().replace('\\', "/");
    let mut options = ConnectOptions::new(format!("sqlite://{normalized}?mode={mode}"));
    options
        .max_connections(max_connections)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options).await.map_err(database_error)
}

/// 带位置参数的 `SeaORM` raw statement builder。
pub(crate) struct SqlStatement {
    sql: String,
    values: Vec<Value>,
}

pub(crate) fn statement(sql: impl Into<String>) -> SqlStatement {
    SqlStatement {
        sql: sql.into(),
        values: Vec::new(),
    }
}

impl SqlStatement {
    pub(crate) fn bind(mut self, value: impl Into<Value>) -> Self {
        self.values.push(value.into());
        self
    }

    pub(crate) async fn execute(self, connection: &DatabaseSession) -> Result<usize, DbErr> {
        let result = connection.execute(self.into_statement()).await?;
        usize::try_from(result.rows_affected())
            .map_err(|_| DbErr::Type("rows_affected 超出 usize".to_string()))
    }

    pub(crate) async fn load<T: FromQueryResult>(
        self,
        connection: &DatabaseSession,
    ) -> Result<Vec<T>, DbErr> {
        connection
            .query_all(self.into_statement())
            .await?
            .iter()
            .map(|row| T::from_query_result(row, ""))
            .collect()
    }

    pub(crate) async fn get_result<T: FromQueryResult>(
        self,
        connection: &DatabaseSession,
    ) -> Result<T, DbErr> {
        connection
            .query_one(self.into_statement())
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("query 未返回记录".to_string()))
            .and_then(|row| T::from_query_result(&row, ""))
    }

    fn into_statement(self) -> Statement {
        Statement::from_sql_and_values(DatabaseBackend::Sqlite, self.sql, self.values)
    }
}

pub(crate) trait OptionalResultExt<T> {
    fn optional(self) -> Result<Option<T>, DbErr>;
}

impl<T> OptionalResultExt<T> for Result<T, DbErr> {
    fn optional(self) -> Result<Option<T>, DbErr> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(DbErr::RecordNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

pub(crate) fn validate_config(config: &StorageConfig) -> Result<(), StorageError> {
    if config.database_path.to_string_lossy() == ":memory:" {
        return Err(StorageError::InvalidInput(
            "Event Store 禁止使用 :memory: SQLite".to_string(),
        ));
    }
    if config.read_concurrency == 0 {
        return Err(StorageError::InvalidInput(
            "read_concurrency 必须至少为 1".to_string(),
        ));
    }
    if let Some(parent) = config.database_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| StorageError::FileSystem(error.to_string()))?;
    }
    std::fs::create_dir_all(&config.artifact_root)
        .map_err(|error| StorageError::FileSystem(error.to_string()))?;
    Ok(())
}

pub(crate) fn database_error(error: impl std::fmt::Display) -> StorageError {
    StorageError::Database(error.to_string())
}
