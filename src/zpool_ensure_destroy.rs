use crate::{
    command_error::CommandError,
    zpool_destroy::zpool_destroy,
    zpool_list::{ZpoolListError, zpool_list},
};

#[derive(Debug)]
pub enum ZpoolEnsureDestroyError {
    ZpoolDestroyError(CommandError),
    ZpoolListError(ZpoolListError),
}

#[derive(Debug)]
pub enum ZpoolEnsureDestroyOutput {
    Destroyed,
    DoesNotExist,
}

pub async fn zpool_ensure_destroy(
    zpool: &str,
) -> Result<ZpoolEnsureDestroyOutput, ZpoolEnsureDestroyError> {
    match zpool_destroy(zpool).await {
        Ok(()) => Ok(ZpoolEnsureDestroyOutput::Destroyed),
        Err(e) => {
            if !zpool_list()
                .await
                .map_err(ZpoolEnsureDestroyError::ZpoolListError)?
                .iter()
                .any(|existing_zpool| existing_zpool == zpool)
            {
                Ok(ZpoolEnsureDestroyOutput::DoesNotExist)
            } else {
                Err(ZpoolEnsureDestroyError::ZpoolDestroyError(e))
            }
        }
    }
}
