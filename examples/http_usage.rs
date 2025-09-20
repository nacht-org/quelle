// Example showing how to use the new HTTP parameters and headers API

use quelle_extension::prelude::*;

// Example of making a GET request with parameters
fn example_get_with_params() -> Result<(), eyre::Report> {
    let client = Client::new();

    let response = Request::get("https://api.example.com/search")
        .param("q", "rust programming")
        .param("limit", "10")
        .param("offset", "0")
        .header("User-Agent", "Quelle Extension/1.0")
        .header("Accept", "application/json")
        .send(&client)?;

    if let Some(text) = response.text()? {
        println!("Response: {}", text);
    }

    Ok(())
}

// Example of making a POST request with headers
fn example_post_with_headers() -> Result<(), eyre::Report> {
    let client = Client::new();

    let form_data = RequestFormBuilder::new()
        .param("username", "john_doe")
        .param("password", "secret123")
        .build();

    let response = Request::post("https://api.example.com/login")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", "Bearer token123")
        .header("X-API-Version", "v2")
        .body(form_data)
        .send(&client)?;

    match response.error_for_status() {
        Ok(resp) => {
            println!("Login successful! Status: {}", resp.status);
            if let Some(text) = resp.text()? {
                println!("Response body: {}", text);
            }
        }
        Err(e) => {
            println!("Login failed: {}", e);
        }
    }

    Ok(())
}

// Example of setting multiple parameters and headers at once
fn example_bulk_params_and_headers() -> Result<(), eyre::Report> {
    let client = Client::new();

    let params = vec![
        ("category".to_string(), "books".to_string()),
        ("author".to_string(), "tolkien".to_string()),
        ("year".to_string(), "1954".to_string()),
    ];

    let headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("Accept-Language".to_string(), "en-US".to_string()),
        ("Cache-Control".to_string(), "no-cache".to_string()),
    ];

    let response = Request::get("https://api.bookstore.com/books")
        .params(params)
        .headers(headers)
        .send(&client)?;

    println!("Status: {}", response.status);
    if response.is_success() {
        if let Some(text) = response.text()? {
            println!("Books found: {}", text);
        }
    }

    Ok(())
}

// Example showing the builder pattern flexibility
fn example_conditional_params() -> Result<(), eyre::Report> {
    let client = Client::new();

    let mut request =
        Request::get("https://api.example.com/data").header("User-Agent", "Quelle/1.0");

    // Conditionally add parameters
    let include_metadata = true;
    if include_metadata {
        request = request.param("include_meta", "true");
    }

    let page_size = Some(20);
    if let Some(size) = page_size {
        request = request.param("page_size", size.to_string());
    }

    // Add authentication header if available
    let api_key = std::env::var("API_KEY").ok();
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = request.send(&client)?;
    println!("Request completed with status: {}", response.status);

    Ok(())
}

fn main() -> Result<(), eyre::Report> {
    println!("=== HTTP Parameters and Headers Examples ===\n");

    println!("1. GET request with parameters:");
    example_get_with_params()?;

    println!("\n2. POST request with headers:");
    example_post_with_headers()?;

    println!("\n3. Bulk parameters and headers:");
    example_bulk_params_and_headers()?;

    println!("\n4. Conditional parameters:");
    example_conditional_params()?;

    Ok(())
}
