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
            chapters,
            output,
            template,
            combine_volumes,
            updated,
        } => {
            handle_export_epub(
                novel_id,
                chapters,
                output,
                template,
                combine_volumes,
                updated,
                storage,
                dry_run,
            )
            .await
        }
        ExportCommands::Pdf { novel_id, output } => {
            handle_export_pdf(novel_id, output, dry_run).await
        }
        ExportCommands::Html { novel_id, output } => {
            handle_export_html(novel_id, output, dry_run).await
        }
        ExportCommands::Txt { novel_id, output } => {
            handle_export_txt(novel_id, output, dry_run).await
        }
    }
}

async fn handle_export_epub(
    novel_id: String,
    chapters: Option<String>,
    output: Option<String>,
    template: Option<String>,
    combine_volumes: bool,
    updated: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would export to EPUB: {}", novel_id);
        if let Some(ref chapters_filter) = chapters {
            println!("  Chapters: {}", chapters_filter);
        }
        if let Some(ref output_dir) = output {
            println!("  Output dir: {}", output_dir);
        }
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
                let export_options = ExportOptions::new();

                if let Some(_template_path) = template {
                    println!("  📋 Custom template support not yet implemented");
                }

                if combine_volumes {
                    println!("  📋 Volume combining not yet implemented");
                }

                if updated {
                    println!("  📋 Updated-only export not yet implemented");
                }

                // TODO: Parse chapters filter like "1-10", "5", "1,3,5-10"
                if let Some(_chapters_filter) = chapters {
                    println!("  📋 Chapter filtering not yet implemented, exporting all chapters");
                }

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

            let export_options = ExportOptions::new();

            if combine_volumes {
                println!("    📋 Volume combining not yet implemented");
            }

            if updated {
                println!("    📋 Updated-only export not yet implemented");
            }

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

async fn handle_export_pdf(novel_id: String, _output: Option<String>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would export to PDF: {}", novel_id);
    } else {
        println!("🚧 PDF export is not yet implemented");
        println!("📄 Novel ID: {}", novel_id);
    }
    Ok(())
}

async fn handle_export_html(
    novel_id: String,
    _output: Option<String>,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would export to HTML: {}", novel_id);
    } else {
        println!("🚧 HTML export is not yet implemented");
        println!("🌐 Novel ID: {}", novel_id);
    }
    Ok(())
}

async fn handle_export_txt(novel_id: String, _output: Option<String>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would export to TXT: {}", novel_id);
    } else {
        println!("🚧 TXT export is not yet implemented");
        println!("📝 Novel ID: {}", novel_id);
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
