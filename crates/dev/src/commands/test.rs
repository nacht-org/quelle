//! Test command for extensions

use eyre::{Result, eyre};
use quelle_engine::ExtensionEngine;
use quelle_engine::bindings::quelle::extension::novel::SimpleSearchQuery;
use quelle_engine::http::GhostwireExecutor;
use std::sync::Arc;
use url::Url;

use crate::utils::{find_extension_path, find_project_root};

pub async fn run(extension_name: String, url: Option<Url>, query: Option<String>) -> Result<()> {
    if url.is_none() && query.is_none() {
        eprintln!("Provide at least one of --url or --query");
        eprintln!("  --url <URL>      Fetch novel info from URL");
        eprintln!("  --query <TEXT>   Search for novels");
        return Ok(());
    }

    let extension_path = find_extension_path(&extension_name)?;

    println!("Building extension '{}'...", extension_name);
    let output = tokio::process::Command::new("cargo")
        .args([
            "component",
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            &format!("extension_{}", extension_name),
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Build failed:\n{}", stderr));
    }

    let project_root = find_project_root(&extension_path)?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", extension_name));

    if !wasm_path.exists() {
        return Err(eyre!("WASM file not found: {}", wasm_path.display()));
    }

    let executor = Arc::new(GhostwireExecutor::new()?);
    let engine = ExtensionEngine::new(executor)?;
    let wasm_str = wasm_path.to_str().unwrap().to_string();

    if let Some(url) = url {
        println!("Fetching novel info for: {}", url);
        let runner = engine.new_runner_from_file(&wasm_str).await?;
        let (_, result) = runner.fetch_novel_info(url.as_ref()).await?;
        match result {
            Ok(novel) => {
                println!("Title: {}", novel.title);
                println!(
                    "Authors: {}",
                    if novel.authors.is_empty() {
                        "Unknown".to_string()
                    } else {
                        novel.authors.join(", ")
                    }
                );
                let desc = novel.description.join(" ");
                println!(
                    "Description: {}",
                    if desc.len() > 100 {
                        format!("{}...", &desc[..100])
                    } else {
                        desc
                    }
                );
                println!("Status: {:?}", novel.status);
                let total_chapters: usize = novel.volumes.iter().map(|v| v.chapters.len()).sum();
                println!(
                    "Volumes: {}, Chapters: {}",
                    novel.volumes.len(),
                    total_chapters
                );
            }
            Err(e) => {
                let chain = e
                    .frames
                    .iter()
                    .map(|f| f.message.as_str())
                    .collect::<Vec<_>>()
                    .join(": ");
                eprintln!("Error: {}", chain);
            }
        }
    }

    if let Some(query) = query {
        println!("Searching for: '{}'", query);
        let runner = engine.new_runner_from_file(&wasm_str).await?;
        let search_query = SimpleSearchQuery {
            query: query.clone(),
            page: Some(1),
            limit: Some(10),
        };
        let (_, result) = runner.simple_search(&search_query).await?;
        match result {
            Ok(results) => {
                println!("Found {} novels", results.novels.len());
                for (i, novel) in results.novels.iter().enumerate() {
                    println!("  {}. {} — {}", i + 1, novel.title, novel.url);
                }
            }
            Err(e) => {
                let chain = e
                    .frames
                    .iter()
                    .map(|f| f.message.as_str())
                    .collect::<Vec<_>>()
                    .join(": ");
                eprintln!("Error: {}", chain);
            }
        }
    }

    Ok(())
}
