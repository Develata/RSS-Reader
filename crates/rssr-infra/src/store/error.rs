use rssr_domain::DomainError;
use rusqlite::ErrorCode;

pub type StoreResult<T> = Result<T, StoreError>;

/// Storage failure as the rest of the program sees it: a classification, plus the SQLite error
/// kept as the source for diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("未找到请求的资源")]
    NotFound,
    #[error("违反数据约束")]
    Conflict(#[source] rusqlite::Error),
    #[error("数据库被其他连接占用")]
    Busy(#[source] rusqlite::Error),
    #[error("数据库操作超过时限被中断")]
    Interrupted,
    #[error("数据库文件已损坏")]
    Corrupt(#[source] rusqlite::Error),
    #[error("不是 RSS-Reader 资料库")]
    NotALibrary,
    #[error("资料库已存在：{0}")]
    LibraryExists(String),
    #[error("资料库由更新版本的程序创建（结构版本 {0}）")]
    NewerLibrary(i64),
    #[error("资料库记录的结构迁移 {0} 与本程序的不一致")]
    MigrationModified(i64),
    #[error("资料库的结构迁移记录不连续")]
    MigrationGap,
    #[error("等待数据库连接超时")]
    ConnectionTimeout,
    #[error("数据库任务异常结束")]
    TaskFailed,
    #[error("文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("数据库错误：{0}")]
    Sqlite(#[source] rusqlite::Error),
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        let code = match &error {
            rusqlite::Error::SqliteFailure(failure, _) => failure.code,
            _ => return Self::Sqlite(error),
        };
        match code {
            ErrorCode::ConstraintViolation => Self::Conflict(error),
            ErrorCode::DatabaseBusy => Self::Busy(error),
            ErrorCode::OperationInterrupted => Self::Interrupted,
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => Self::Corrupt(error),
            _ => Self::Sqlite(error),
        }
    }
}

impl From<StoreError> for DomainError {
    /// Keeps what a caller can act on: a constraint violation means another write got there
    /// first (read again), busy / interrupted / timed out may pass when retried.
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => Self::NotFound,
            StoreError::Domain(error) => error,
            StoreError::Conflict(_) => Self::Conflict(error_chain(&error)),
            StoreError::Busy(_) | StoreError::Interrupted | StoreError::ConnectionTimeout => {
                Self::Unavailable(error_chain(&error))
            }
            other => Self::Persistence(error_chain(&other)),
        }
    }
}

/// The error and its sources on one line, as `anyhow`'s `{:#}` would print them.
fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let cause_text = cause.to_string();
        if !text.contains(&cause_text) {
            text.push_str(": ");
            text.push_str(&cause_text);
        }
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use rssr_domain::DomainError;
    use rusqlite::ffi;

    use super::StoreError;

    fn sqlite(code: i32) -> StoreError {
        rusqlite::Error::SqliteFailure(ffi::Error::new(code), None).into()
    }

    /// What a caller can act on survives the conversion: read again after a conflict, retry
    /// when busy; the rest is a storage failure.
    #[test]
    fn domain_errors_keep_the_actionable_categories() {
        assert!(matches!(
            DomainError::from(sqlite(ffi::SQLITE_CONSTRAINT)),
            DomainError::Conflict(_)
        ));
        assert!(matches!(DomainError::from(sqlite(ffi::SQLITE_BUSY)), DomainError::Unavailable(_)));
        assert!(matches!(DomainError::from(StoreError::Interrupted), DomainError::Unavailable(_)));
        assert!(matches!(
            DomainError::from(StoreError::ConnectionTimeout),
            DomainError::Unavailable(_)
        ));
        assert!(matches!(
            DomainError::from(sqlite(ffi::SQLITE_IOERR)),
            DomainError::Persistence(_)
        ));
        assert!(matches!(DomainError::from(StoreError::NotFound), DomainError::NotFound));
        assert!(matches!(
            DomainError::from(StoreError::Domain(DomainError::InvalidInput("x".into()))),
            DomainError::InvalidInput(_)
        ));
    }
}
