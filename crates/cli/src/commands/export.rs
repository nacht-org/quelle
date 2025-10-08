//! Export command handlers for converting novels to various formats.

use eyre::Result;
use quelle_export::{ExportOptions, default_export_manager};
use quelle_storage::{
    backends::filesystem::FilesystemStorage, traits::BookStorage, types::NovelFilter,
};
use std::path::PathBuf;

use crate::utils::resolve_novel_id;

pub async fn handle_export(
    novel_input: String,
    format: String,
    output: Option<String>,
    include_images: bool,
    storage: &FilesystemStorage,
    _dry_run: bool,
) -> Result<()> {
    let export_manager = default_export_manager()?;
    if !export_manager.supports_format(&format) {
        println!("Unsupported format: {}", format);
        let available_formats = export_manager.available_formats();
        println!(
            "Supported: {}",
            available_formats
                .iter()
                .map(|f| f.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return Ok(());
    }

    if novel_input == "all" {
        return export_all_novels(&format, output, include_images, storage, &export_manager).await;
    }

    // Resolve novel input (ID, URL, or title)
    let novel_id = match resolve_novel_id(&novel_input, storage).await? {
        Some(id) => id,
        None => {
            println!("Novel not found: {}", novel_input);
            return Ok(());
        }
    };

    let novel = match storage.get_novel(&novel_id).await? {
        Some(novel) => novel,
        None => {
            println!("Novel not found: {}", novel_id.as_str());
            return Ok(());
        }
    };

    println!("Exporting: {}", novel.title);

    // List chapters to export
    let chapter_list = storage.list_chapters(&novel_id).await?;
    let available_chapters: Vec<_> = chapter_list.iter().filter(|c| c.has_content()).collect();

    if available_chapters.is_empty() {
        println!("No content available for export");
        return Ok(());
    }

    println!(
        "  Chapters: {} available for export",
        available_chapters.len()
    );

    // Determine output path
    let filename = format!("{}.{}", sanitize_filename(&novel.title), format);
    let output_path = if let Some(output_dir) = &output {
        PathBuf::from(output_dir).join(filename)
    } else {
        PathBuf::from(filename)
    };

    println!("Output: {}", output_path.display());

    let export_options = if include_images {
        ExportOptions::new()
    } else {
        ExportOptions::new().without_images()
    };

    println!("Exporting to {}...", format);

    let file = tokio::fs::File::create(&output_path).await?;
    let writer = Box::new(file);

    match export_manager
        .export(&format, storage, &novel_id, writer, &export_options)
        .await
    {
        Ok(result) => {
            println!(
                "Exported {} chapters to {}",
                result.chapters_processed,
                output_path.display()
            );
        }
        Err(e) => {
            eprintln!("Export failed: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn export_all_novels(
    format: &str,
    output: Option<String>,
    include_images: bool,
    storage: &FilesystemStorage,
    export_manager: &quelle_export::ExportManager,
) -> Result<()> {
    let novels = storage.list_novels(&NovelFilter::default()).await?;
    if novels.is_empty() {
        println!("No novels in library");
        return Ok(());
    }

    println!(
        "Exporting {} novels to {}",
        novels.len(),
        format.to_uppercase()
    );

    // Determine output directory
    let output_dir = output.unwrap_or_else(|| "./exports".to_string());
    let output_path = PathBuf::from(&output_dir);

    std::fs::create_dir_all(&output_path)?;
    println!("Output directory: {}", output_path.display());

    let mut exported_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;

    for novel in &novels {
        let chapter_list = storage.list_chapters(&novel.id).await?;
        let available_chapters = chapter_list.iter().filter(|c| c.has_content()).count();

        if available_chapters == 0 {
            skipped_count += 1;
            continue;
        }

        let filename = format!("{}.epub", sanitize_filename(&novel.title));
        let novel_output_path = output_path.join(filename);

        println!(
            "  📖 Exporting {} ({} chapters)...",
            novel.title, available_chapters
        );

        let export_options = if include_images {
            ExportOptions::new()
        } else {
            ExportOptions::new().without_images()
        };

        let file = match tokio::fs::File::create(&novel_output_path).await {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to create file: {}", e);
                failed_count += 1;
                continue;
            }
        };
        let writer = Box::new(file);

        match export_manager
            .export(format, storage, &novel.id, writer, &export_options)
            .await
        {
            Ok(result) => {
                println!(
                    "    Exported {} chapters to: {}",
                    result.chapters_processed,
                    novel_output_path.display()
                );
                exported_count += 1;
            }
            Err(e) => {
                eprintln!("Failed: {}", e);
                failed_count += 1;
            }
        }
    }

    println!(
        "Export complete: {} exported, {} skipped, {} failed",
        exported_count, skipped_count, failed_count
    );

    Ok(())
}

// Backward compatibility function for EPUB export
#[allow(dead_code)]
pub async fn handle_export_epub(
    novel_input: String,
    output: Option<String>,
    include_images: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    handle_export(
        novel_input,
        "epub".to_string(),
        output,
        include_images,
        storage,
        dry_run,
    )
    .await
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '_',
            c => c,
        })
        .collect()
}
