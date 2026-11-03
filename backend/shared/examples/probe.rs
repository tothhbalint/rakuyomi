use std::path::PathBuf;

use anyhow::{Context, Result};
use shared::{settings::Settings, source::Source};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().context("usage: probe <source.aix> <query>")?);
    let query = args.next().context("usage: probe <source.aix> <query>")?;
    let download = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?;

    let source = Source::from_aix_file(&path, Settings::default())?;
    println!(
        "source: {} (version {})",
        source.manifest().info.name,
        source.manifest().info.version
    );

    let mangas = source
        .search_mangas(CancellationToken::new(), query.clone())
        .await?;
    println!("found {} results", mangas.len());
    for (index, manga) in mangas.iter().take(25).enumerate() {
        println!(
            "{index:2}: {} [{}]",
            manga.title.as_deref().unwrap_or("(untitled)"),
            manga.id
        );
    }

    let exact = mangas.iter().find(|manga| {
        manga
            .title
            .as_deref()
            .is_some_and(|title| title.trim().eq_ignore_ascii_case("death note"))
    });
    let Some(manga) = exact
        .or_else(|| {
            mangas.iter().find(|manga| {
                manga
                    .title
                    .as_deref()
                    .is_some_and(|title| title.to_lowercase().contains("death note"))
            })
        })
        .or_else(|| mangas.first())
    else {
        return Ok(());
    };

    println!("\nselected: {:?} [{}]", manga.title, manga.id);
    if let Some(cover) = &manga.cover_url {
        println!("cover: {cover}");
    }
    if let Some(description) = &manga.description {
        println!(
            "description: {}",
            description.chars().take(200).collect::<String>()
        );
    }

    let chapters = source
        .get_chapter_list(CancellationToken::new(), manga.id.clone())
        .await?;
    println!("\nchapters: {}", chapters.len());
    for chapter in chapters.iter().take(3) {
        println!(
            "  {} (num={:?}, vol={:?}, lang={}, scanlator={:?})",
            chapter.title.as_deref().unwrap_or(&chapter.id),
            chapter.chapter_num,
            chapter.volume_num,
            chapter.lang,
            chapter.scanlator
        );
    }
    for chapter in chapters.iter().rev().take(3) {
        println!(
            "  (oldest) {} (num={:?}, vol={:?}, lang={}, scanlator={:?})",
            chapter.title.as_deref().unwrap_or(&chapter.id),
            chapter.chapter_num,
            chapter.volume_num,
            chapter.lang,
            chapter.scanlator
        );
    }

    let chapter = chapters
        .iter()
        .find(|chapter| chapter.chapter_num == Some(1.0))
        .or_else(|| {
            chapters.iter().min_by(|a, b| {
                a.chapter_num
                    .unwrap_or(f32::MAX)
                    .partial_cmp(&b.chapter_num.unwrap_or(f32::MAX))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .or_else(|| chapters.first());

    if let Some(chapter) = chapter {
        let pages = source
            .get_page_list(
                CancellationToken::new(),
                manga.id.clone(),
                chapter.id.clone(),
                chapter.chapter_num.map(f64::from),
            )
            .await?;
        println!("\npages in first chapter: {}", pages.len());
        for page in pages.iter().take(5) {
            println!("  {}: {:?}", page.index, page.image_url);
        }

        if let Some(download) = download {
            if let Some(page) = pages.get(download) {
                let image_url = page.image_url.clone().context("page has no image URL")?;
                let request = source
                    .get_image_request(image_url.clone(), page.context.clone())
                    .await?;
                let client = reqwest::Client::builder()
                    .danger_accept_invalid_certs(true)
                    .build()?;
                let response = client.execute(request).await?;
                let status = response.status();
                let bytes = response.bytes().await?;
                let output = format!("/tmp/rakuyomi-probe-{download}.img");
                std::fs::write(&output, &bytes)?;
                println!(
                    "\ndownloaded page {download} from {image_url}: HTTP {status}, {} bytes -> {output}",
                    bytes.len()
                );
            }
        }
    }

    Ok(())
}
