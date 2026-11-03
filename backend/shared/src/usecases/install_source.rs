use anyhow::{anyhow, Context, Result};
use futures::{stream, StreamExt, TryStreamExt};

use crate::{model::SourceId, source_manager::SourceManager};

use super::list_available_sources::fetch_source_list;

pub async fn install_source(
    source_manager: &mut SourceManager,
    source_lists: &[url::Url],
    source_id: SourceId,
) -> Result<()> {
    let (source_list, source_list_item) = stream::iter(source_lists.iter().cloned())
        .then(|source_list| async move {
            let items = fetch_source_list(&source_list).await?;
            anyhow::Ok((source_list, items))
        })
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flat_map(|(source_list, items)| {
            items
                .into_iter()
                .map(move |item| (source_list.clone(), item))
        })
        .filter(|(_, item)| item.id == source_id)
        .max_by_key(|(_, item)| item.version)
        .ok_or_else(|| anyhow!("couldn't find source with id '{:?}'", source_id))?;

    let download_url = source_list_item
        .download_url
        .context("source list item has no download URL")?;

    // New lists provide paths relative to the list URL (e.g. `sources/en.foo-v3.aix`),
    // while legacy lists provided a file name that was relative to `sources/`.
    let aix_url = if download_url.contains("://") {
        url::Url::parse(&download_url)?
    } else if download_url.contains('/') {
        source_list
            .join(&download_url)
            .or_else(|_| source_list.join(&format!("sources/{download_url}")))?
    } else {
        source_list.join(&format!("sources/{download_url}"))?
    };

    let aix_content = reqwest::get(aix_url.clone())
        .await
        .with_context(|| format!("failed to download source from {aix_url}"))?
        .error_for_status()
        .with_context(|| format!("source download failed for {aix_url}"))?
        .bytes()
        .await?;

    source_manager.install_source(&source_id, aix_content)?;

    Ok(())
}
