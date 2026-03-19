//! Export command handlers for converting novels to various formats.

use eyre::Result;
use quelle_export::{ExportOptions, default_export_manager};
use quelle_storage::{
    backends::filesystem::FilesystemStorage, traits::BookStorage, types::NovelFilter,
};
use std::path::PathBuf;

use crate::resolve::resolve_novel_id;

/// Handle the `export` command — validate format then export a novel (or all novels).
pub async fn handle_export_command(
    novel_input: String,
    format: String,
    output: Option<String>,
    include_images: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    // Validate format up front.
    match format.as_str() {
        "epub" => {}
        #[cfg(feature = "pdf")]
        "pdf" => {}
        _ => {
            eprintln!("Error: Unsupported format: {}", format);
            #[cfg(feature = "pdf")]
            eprintln!("Supported formats: epub, pdf");
            #[cfg(not(feature = "pdf"))]
            eprintln!("Supported formats: epub");
            return Ok(());
        }
    }

    if dry_run {
        println!("Would export novel '{}' in {} format.", novel_input, format);
        if let Some(ref output_dir) = output {
            println!("output: {}", output_dir);
        }
        println!("include_images: {}", include_images);
        return Ok(());
    }

    let export_manager = default_export_manager()?;
    if !export_manager.supports_format(&format) {
        let available_formats = export_manager.available_formats();
        eprintln!("Error: Unsupported format: {}", format);
        eprintln!(
            "Supported formats: {}",
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

    let novel_id = match resolve_novel_id(&novel_input, storage).await? {
        Some(id) => id,
        None => {
            eprintln!("Not found: '{}'", novel_input);
            return Ok(());
        }
    };

    let novel = match storage.get_novel(&novel_id).await? {
        Some(novel) => novel,
        None => {
            eprintln!("Not found: '{}'", novel_id.as_str());
            return Ok(());
        }
    };

    let chapter_list = storage.list_chapters(&novel_id).await?;
    let available_chapters: Vec<_> = chapter_list.iter().filter(|c| c.has_content()).collect();

    if available_chapters.is_empty() {
        println!("No downloaded chapters available to export.");
        return Ok(());
    }

    println!("Exporting: {}", novel.title);
    println!("chapters available: {}", available_chapters.len());

    let filename = format!("{}.{}", sanitize_filename(&novel.title), format);
    let output_path = if let Some(output_dir) = &output {
        PathBuf::from(output_dir).join(filename)
    } else {
        PathBuf::from(filename)
    };

    println!("output: {}", output_path.display());
    println!("Exporting...");

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let export_options = if include_images {
        ExportOptions::new()
    } else {
        ExportOptions::new().without_images()
    };

    let file = tokio::fs::File::create(&output_path).await?;
    let writer = Box::new(file);

    match export_manager
        .export(&format, storage, &novel_id, writer, &export_options)
        .await
    {
        Ok(result) => {
            println!(
                "Exported {} chapter(s) to {}",
                result.chapters_processed,
                output_path.display()
            );
        }
        Err(e) => {
            eprintln!("Error: Export failed: {}", e);
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
        println!("No novels in library.");
        return Ok(());
    }

    let output_dir = output.unwrap_or_else(|| "./exports".to_string());
    let output_path = PathBuf::from(&output_dir);
    std::fs::create_dir_all(&output_path)?;

    println!(
        "Exporting {} novel(s) to {}...",
        novels.len(),
        output_path.display()
    );

    let mut exported_count = 0usize;
    let mut failed_count = 0usize;
    let mut skipped_count = 0usize;

    for novel in &novels {
        let chapter_list = storage.list_chapters(&novel.id).await?;
        let available_chapters = chapter_list.iter().filter(|c| c.has_content()).count();

        if available_chapters == 0 {
            skipped_count += 1;
            continue;
        }

        let filename = format!("{}.{}", sanitize_filename(&novel.title), format);
        let novel_output_path = output_path.join(filename);

        let export_options = if include_images {
            ExportOptions::new()
        } else {
            ExportOptions::new().without_images()
        };

        let file = match tokio::fs::File::create(&novel_output_path).await {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "Error: Failed to create output file for '{}': {}",
                    novel.title, e
                );
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
                    "  {} — {} chapter(s) exported.",
                    novel.title, result.chapters_processed
                );
                exported_count += 1;
            }
            Err(e) => {
                eprintln!("Error: Failed to export '{}': {}", novel.title, e);
                failed_count += 1;
            }
        }
    }

    println!(
        "exported: {}, skipped: {}, failed: {}",
        exported_count, skipped_count, failed_count
    );

    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '_',
            c => c,
        })
        .collect()
}
