use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use quelle_engine::bindings::quelle::extension::http;
use quelle_engine::http::HttpExecutor;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedResponse {
    status: u16,
    headers: Option<Vec<(String, String)>>,
    data: Option<Vec<u8>>,
    timestamp: u64,
    ttl_seconds: u64,
}

impl CachedResponse {
    fn new(response: http::Response, ttl_seconds: u64) -> Self {
        Self {
            status: response.status,
            headers: response.headers,
            data: response.data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl_seconds,
        }
    }

    fn to_response(&self) -> http::Response {
        http::Response {
            status: self.status,
            headers: self.headers.clone(),
            data: self.data.clone(),
        }
    }

    fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now > self.timestamp + self.ttl_seconds
    }
}

pub struct CachingHttpExecutor {
    inner: Arc<dyn HttpExecutor>,
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
    cache_dir: Option<PathBuf>,
    default_ttl_seconds: u64,
    max_cache_size: usize,
}

impl CachingHttpExecutor {
    /// Create a new caching HTTP executor with in-memory cache only
    pub fn new(inner: Arc<dyn HttpExecutor>) -> Self {
        Self {
            inner,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_dir: None,
            default_ttl_seconds: 300, // 5 minutes default TTL
            max_cache_size: 1000,
        }
    }

    /// Create a new caching HTTP executor with file-based cache
    pub fn with_file_cache(inner: Arc<dyn HttpExecutor>, cache_dir: PathBuf) -> Self {
        Self {
            inner,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_dir: Some(cache_dir),
            default_ttl_seconds: 300,
            max_cache_size: 1000,
        }
    }

    /// Set the default TTL for cached responses
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.default_ttl_seconds = ttl_seconds;
        self
    }

    /// Set the maximum number of cached responses in memory
    pub fn with_max_cache_size(mut self, max_size: usize) -> Self {
        self.max_cache_size = max_size;
        self
    }

    /// Generate a cache key for the HTTP request
    fn generate_cache_key(&self, request: &http::Request) -> String {
        let mut hasher = Sha256::new();

        // Include method
        hasher.update(request.method.to_string().as_bytes());

        // Include URL
        hasher.update(request.url.as_bytes());

        // Include headers (sorted for consistency)
        if let Some(ref headers) = request.headers {
            let mut sorted_headers: Vec<_> = headers.iter().collect();
            sorted_headers.sort_by_key(|(k, _)| k);
            for (key, value) in sorted_headers {
                hasher.update(key.as_bytes());
                hasher.update(b":");
                hasher.update(value.as_bytes());
                hasher.update(b"\n");
            }
        }

        // Include query parameters (sorted for consistency)
        if let Some(ref params) = request.params {
            let mut sorted_params: Vec<_> = params.iter().collect();
            sorted_params.sort_by_key(|(k, _)| k);
            for (key, value) in sorted_params {
                hasher.update(key.as_bytes());
                hasher.update(b"=");
                hasher.update(value.as_bytes());
                hasher.update(b"&");
            }
        }

        // Include body data
        if let Some(ref body) = request.data {
            match body {
                http::RequestBody::Form(form_data) => {
                    let mut sorted_form: Vec<_> = form_data.iter().collect();
                    sorted_form.sort_by_key(|(k, _)| k);
                    for (key, part) in sorted_form {
                        hasher.update(key.as_bytes());
                        hasher.update(b":");
                        match part {
                            http::FormPart::Text(text) => {
                                hasher.update(b"text:");
                                hasher.update(text.as_bytes());
                            }
                            http::FormPart::Data(data) => {
                                hasher.update(b"data:");
                                if let Some(ref name) = data.name {
                                    hasher.update(name.as_bytes());
                                }
                                if let Some(ref content_type) = data.content_type {
                                    hasher.update(content_type.as_bytes());
                                }
                                hasher.update(&data.data);
                            }
                        }
                        hasher.update(b"\n");
                    }
                }
            }
        }

        format!("{:x}", hasher.finalize())
    }

    /// Check if response should be cached (only successful responses)
    fn should_cache_response(&self, response: &http::Response) -> bool {
        response.status >= 200 && response.status < 300
    }

    /// Load cached response from file if it exists
    async fn load_from_file(&self, cache_key: &str) -> Option<CachedResponse> {
        let cache_dir = self.cache_dir.as_ref()?;

        if !cache_dir.exists() {
            return None;
        }

        let file_path = cache_dir.join(format!("{}.json", cache_key));

        if !file_path.exists() {
            return None;
        }

        match tokio::fs::read_to_string(&file_path).await {
            Ok(content) => {
                match serde_json::from_str::<CachedResponse>(&content) {
                    Ok(cached) => {
                        if cached.is_expired() {
                            // Clean up expired file
                            let _ = tokio::fs::remove_file(&file_path).await;
                            None
                        } else {
                            Some(cached)
                        }
                    }
                    Err(_) => {
                        // Clean up corrupted file
                        let _ = tokio::fs::remove_file(&file_path).await;
                        None
                    }
                }
            }
            Err(_) => None,
        }
    }

    /// Save cached response to file
    async fn save_to_file(&self, cache_key: &str, cached_response: &CachedResponse) {
        if let Some(ref cache_dir) = self.cache_dir {
            if let Err(e) = tokio::fs::create_dir_all(cache_dir).await {
                tracing::warn!("Failed to create cache directory: {}", e);
                return;
            }

            let file_path = cache_dir.join(format!("{}.json", cache_key));

            match serde_json::to_string(cached_response) {
                Ok(content) => {
                    if let Err(e) = tokio::fs::write(&file_path, content).await {
                        tracing::warn!("Failed to save cached response to file: {}", e);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to serialize cached response: {}", e);
                }
            }
        }
    }

    /// Clean up expired entries from in-memory cache
    async fn cleanup_memory_cache(&self) {
        let mut cache = self.cache.write().await;

        // Remove expired entries
        cache.retain(|_, cached| !cached.is_expired());

        // If still over limit, remove oldest entries
        if cache.len() > self.max_cache_size {
            let mut entries: Vec<_> = cache
                .iter()
                .map(|(k, v)| (k.clone(), v.timestamp))
                .collect();
            entries.sort_by_key(|(_, timestamp)| *timestamp);

            let to_remove = cache.len() - self.max_cache_size;
            for (key, _) in entries.iter().take(to_remove) {
                cache.remove(key);
            }
        }
    }

    /// Clear all cached responses
    pub async fn clear_cache(&self) {
        // Clear memory cache
        self.cache.write().await.clear();

        // Clear file cache
        if let Some(ref cache_dir) = self.cache_dir
            && cache_dir.exists()
            && let Ok(mut entries) = tokio::fs::read_dir(cache_dir).await
        {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(extension) = entry.path().extension()
                    && extension == "json"
                {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let memory_count = cache.len();

        let file_count = if let Some(ref cache_dir) = self.cache_dir {
            if cache_dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(cache_dir).await {
                    let mut count = 0;
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if let Some(extension) = entry.path().extension()
                            && extension == "json"
                        {
                            count += 1;
                        }
                    }
                    count
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        (memory_count, file_count)
    }
}

#[async_trait]
impl HttpExecutor for CachingHttpExecutor {
    async fn execute(&self, request: http::Request) -> Result<http::Response, http::ResponseError> {
        // Cache GET, HEAD, and POST requests (PUT, DELETE should not be cached)
        let should_use_cache = matches!(
            request.method,
            http::Method::Get | http::Method::Head | http::Method::Post
        );

        if !should_use_cache {
            tracing::debug!(
                "Bypassing cache for non-cacheable request: {}",
                request.method
            );
            return self.inner.execute(request).await;
        }

        let cache_key = self.generate_cache_key(&request);
        let request_url = request.url.clone();

        // Check memory cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key)
                && !cached.is_expired()
            {
                tracing::debug!("Cache hit (memory) for request: {}", request_url);
                return Ok(cached.to_response());
            }
        }

        // Check file cache
        if let Some(cached) = self.load_from_file(&cache_key).await {
            tracing::debug!("Cache hit (file) for request: {}", request_url);

            // Update memory cache
            {
                let mut cache = self.cache.write().await;
                cache.insert(cache_key.clone(), cached.clone());
            }

            return Ok(cached.to_response());
        }

        tracing::debug!("Cache miss for request: {}", request_url);

        // Execute the request
        let response = self.inner.execute(request).await?;

        // Cache successful responses
        if self.should_cache_response(&response) {
            let cached_response = CachedResponse::new(response.clone(), self.default_ttl_seconds);

            // Update memory cache
            {
                let mut cache = self.cache.write().await;
                cache.insert(cache_key.clone(), cached_response.clone());

                // Trigger cleanup if cache is getting large
                if cache.len() > self.max_cache_size {
                    drop(cache); // Release write lock before cleanup
                    self.cleanup_memory_cache().await;
                }
            }

            // Save to file cache
            self.save_to_file(&cache_key, &cached_response).await;

            tracing::debug!("Cached successful response for: {}", request_url);
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use quelle_engine::bindings::quelle::extension::http::{
        Method, Request, Response, ResponseError,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio;

    // Mock HTTP executor for testing
    struct MockHttpExecutor {
        call_count: Arc<AtomicUsize>,
        response: Response,
    }

    impl MockHttpExecutor {
        fn new(response: Response) -> Self {
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
                response,
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl HttpExecutor for MockHttpExecutor {
        async fn execute(&self, _request: Request) -> Result<Response, ResponseError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            // Simulate some delay
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            Ok(self.response.clone())
        }
    }

    fn create_test_request(url: &str) -> Request {
        Request {
            method: Method::Get,
            url: url.to_string(),
            headers: None,
            params: None,
            data: None,
            wait_for_element: None,
            wait_timeout_ms: None,
        }
    }

    fn create_test_response(status: u16, data: &str) -> Response {
        Response {
            status,
            headers: Some(vec![("content-type".to_string(), "text/html".to_string())]),
            data: Some(data.as_bytes().to_vec()),
        }
    }

    #[tokio::test]
    async fn test_memory_cache_hit() {
        let mock_response = create_test_response(200, "test response");
        let mock_executor = Arc::new(MockHttpExecutor::new(mock_response.clone()));
        let caching_executor = CachingHttpExecutor::new(mock_executor.clone()).with_ttl(300);

        let request = create_test_request("https://example.com/test");

        // First request should hit the mock executor
        let response1 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1);
        assert_eq!(response1.status, 200);
        assert_eq!(response1.data, Some("test response".as_bytes().to_vec()));

        // Second identical request should hit the cache
        let response2 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1); // Still 1, no additional call
        assert_eq!(response2.status, 200);
        assert_eq!(response2.data, Some("test response".as_bytes().to_vec()));
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let mock_response = create_test_response(200, "expiring response");
        let mock_executor = Arc::new(MockHttpExecutor::new(mock_response.clone()));
        let caching_executor = CachingHttpExecutor::new(mock_executor.clone()).with_ttl(1); // 1 second TTL

        let request = create_test_request("https://example.com/expire");

        // First request
        let response1 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1);
        assert_eq!(response1.status, 200);

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Second request should hit the executor again due to expiration
        let response2 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 2);
        assert_eq!(response2.status, 200);
    }

    #[tokio::test]
    async fn test_post_requests_are_cached() {
        let mock_response = create_test_response(200, "post response");
        let mock_executor = Arc::new(MockHttpExecutor::new(mock_response.clone()));
        let caching_executor = CachingHttpExecutor::new(mock_executor.clone());

        let mut request = create_test_request("https://example.com/post");
        request.method = Method::Post;

        // POST requests should be cached
        let _response1 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1);

        let response2 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1); // Should hit cache, not executor again
        assert_eq!(response2.status, 200);
    }

    #[tokio::test]
    async fn test_non_cacheable_methods() {
        let mock_response = create_test_response(200, "put response");
        let mock_executor = Arc::new(MockHttpExecutor::new(mock_response.clone()));
        let caching_executor = CachingHttpExecutor::new(mock_executor.clone());

        let mut request = create_test_request("https://example.com/put");
        request.method = Method::Put;

        // PUT requests should not be cached
        let _response1 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1);

        let response2 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 2); // Should hit executor again
        assert_eq!(response2.status, 200);
    }

    #[tokio::test]
    async fn test_delete_requests_not_cached() {
        let mock_response = create_test_response(200, "delete response");
        let mock_executor = Arc::new(MockHttpExecutor::new(mock_response.clone()));
        let caching_executor = CachingHttpExecutor::new(mock_executor.clone());

        let mut request = create_test_request("https://example.com/delete");
        request.method = Method::Delete;

        // DELETE requests should not be cached
        let _response1 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1);

        let response2 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 2); // Should hit executor again
        assert_eq!(response2.status, 200);
    }

    #[tokio::test]
    async fn test_error_responses_not_cached() {
        let mock_response = create_test_response(404, "not found");
        let mock_executor = Arc::new(MockHttpExecutor::new(mock_response.clone()));
        let caching_executor = CachingHttpExecutor::new(mock_executor.clone());

        let request = create_test_request("https://example.com/notfound");

        // First request
        let response1 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 1);
        assert_eq!(response1.status, 404);

        // Second request should hit executor again (404 not cached)
        let response2 = caching_executor.execute(request.clone()).await.unwrap();
        assert_eq!(mock_executor.call_count(), 2);
        assert_eq!(response2.status, 404);
    }
}
