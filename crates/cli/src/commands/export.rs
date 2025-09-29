use eyre::Result;
use quelle_export::{ExportOptions, default_export_manager};
use quelle_storage::{
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};
use std::path::PathBuf;

use crate::cli::ExportCommands;

pub async fn handle_export_command(
    cmd: ExportCommands,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        ExportCommands::Epub {
            novel_id,
            output,
            include_images,
        } => handle_export_epub(novel_id, output, include_images, storage, dry_run).await,
    }
}

async fn handle_export_epub(
    novel_id: String,
    output: Option<String>,
    include_images: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would export to EPUB: {}", novel_id);
        if let Some(ref output_dir) = output {
            println!("  Output dir: {}", output_dir);
        }
        println!("  Include images: {}", include_images);
        return Ok(());
    }

    // Initialize export manager
    let export_manager = default_export_manager()?;

    // Check if novel exists
    if novel_id != "all" {
        let id = NovelId::new(novel_id.clone());
        match storage.get_novel(&id).await? {
            Some(novel) => {
                println!("📚 Exporting novel: {}", novel.title);
                println!("  Authors: {}", novel.authors.join(", "));

                // List chapters to export
                let chapter_list = storage.list_chapters(&id).await?;
                let available_chapters: Vec<_> =
                    chapter_list.iter().filter(|c| c.has_content()).collect();

                if available_chapters.is_empty() {
                    println!("❌ No chapter content available for export");
                    println!(
                        "💡 Use 'quelle fetch chapters {}' to download content first",
                        id.as_str()
                    );
                    return Ok(());
                }

                println!(
                    "  Chapters: {} available for export",
                    available_chapters.len()
                );

                // Determine output path
                let filename = format!("{}.epub", sanitize_filename(&novel.title));
                let output_path = if let Some(output_dir) = &output {
                    PathBuf::from(output_dir).join(filename)
                } else {
                    PathBuf::from(filename)
                };

                println!("  Output: {}", output_path.display());

                // Create export options
                let export_options = if include_images {
                    ExportOptions::new()
                } else {
                    ExportOptions::new().without_images()
                };

                // Export the novel
                println!("📖 Starting EPUB export...");

                // Create the output file
                let file = tokio::fs::File::create(&output_path).await?;
                let writer = Box::new(file);

                match export_manager
                    .export("epub", storage, &id, writer, &export_options)
                    .await
                {
                    Ok(result) => {
                        println!("✅ Successfully exported to: {}", output_path.display());
                        println!("  📄 Chapters processed: {}", result.chapters_processed);
                        println!("  📁 File size: {} bytes", result.total_size);
                        println!("  ⏱️  Export time: {:?}", result.export_duration);
                    }
                    Err(e) => {
                        eprintln!("❌ Export failed: {}", e);
                        return Err(e.into());
                    }
                }
            }
            None => {
                println!("❌ Novel not found: {}", id.as_str());
            }
        }
    } else {
        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("📚 No novels found in library");
            return Ok(());
        }

        println!("📚 Exporting {} novels to EPUB", novels.len());

        // Determine output directory
        let output_dir = output.unwrap_or_else(|| "./exports".to_string());
        let output_path = PathBuf::from(&output_dir);

        // Create output directory if it doesn't exist
        std::fs::create_dir_all(&output_path)?;
        println!("  📁 Output directory: {}", output_path.display());

        let mut exported_count = 0;
        let mut failed_count = 0;
        let mut skipped_count = 0;

        for novel in &novels {
            let chapter_list = storage.list_chapters(&novel.id).await?;
            let available_chapters = chapter_list.iter().filter(|c| c.has_content()).count();

            if available_chapters == 0 {
                println!("  ⏭️ {} (no content, skipped)", novel.title);
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

            // Create the output file
            let file = match tokio::fs::File::create(&novel_output_path).await {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("    ❌ Failed to create file: {}", e);
                    failed_count += 1;
                    continue;
                }
            };
            let writer = Box::new(file);

            match export_manager
                .export("epub", storage, &novel.id, writer, &export_options)
                .await
            {
                Ok(result) => {
                    println!(
                        "    ✅ Exported {} chapters to: {}",
                        result.chapters_processed,
                        novel_output_path.display()
                    );
                    exported_count += 1;
                }
                Err(e) => {
                    eprintln!("    ❌ Failed: {}", e);
                    failed_count += 1;
                }
            }
        }

        println!("\n📊 Bulk export complete:");
        println!("  ✅ Exported: {}", exported_count);
        println!("  ⏭️ Skipped (no content): {}", skipped_count);
        if failed_count > 0 {
            println!("  ❌ Failed: {}", failed_count);
        }
    }
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
