pub mod homebrew_executor;
pub mod homebrew_formula_executor;
pub mod mas_executor;
pub mod delegated_executor;
pub mod sparkle_executor;
pub mod microsoft_autoupdate_executor;

use crate::models::UpdateResult;
use crate::utils::AppResult;

pub trait UpdateExecutor: Send + Sync {
    async fn execute(
        &self,
        bundle_id: &str,
        app_path: &str,
        on_progress: &(dyn Fn(u8, &str, Option<(u64, Option<u64>)>) + Send + Sync),
    ) -> AppResult<UpdateResult>;
}
