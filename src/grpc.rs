use crate::embed::EmbedRequest;
use crate::embed::embed_service_client::EmbedServiceClient;
use crate::error::HttpError;

/// gRPC client for vector embedding generation
///
/// This client communicates with a separate Python service running an embedding model
/// (specifically Google's embeddinggemma) to convert text into vector representations.
///
/// **Why gRPC instead of REST/FastAPI?**
/// gRPC provides significant advantages for ML model serving:
///
/// 1. **Lower Resource Usage**: gRPC uses HTTP/2 and Protocol Buffers (binary format)
///    which are much more efficient than JSON over HTTP/1.1. This means:
///    - Less CPU for serialization/deserialization
///    - Lower memory footprint
///    - Reduced network bandwidth
///    - Faster processing times
///
/// 2. **Better Performance**: Typical performance improvements over FastAPI:
///    - 2-3x lower latency for small payloads
///    - More efficient streaming for large requests
///    - Connection multiplexing (multiple requests over one TCP connection)
///
/// 3. **Type Safety**: Protocol Buffers provide strict schemas, catching errors at compile time
///
/// For embedding services that handle many requests, these resource savings are crucial,
/// especially when running on limited hardware or shared infrastructure.
///
/// **What are embeddings?**
/// Embeddings are dense vector representations of text that capture semantic meaning.
/// Similar texts have similar vectors (measured by cosine similarity).
/// Example: "dog" and "puppy" have close vectors, while "dog" and "car" are far apart.
///
/// **Use cases in this application:**
/// - Semantic search: Find blog posts similar in meaning, not just keyword matches
/// - Content similarity: Recommend related articles
/// - Clustering: Group similar posts automatically
#[derive(Clone)]
pub struct GRPCClient {
    /// Tonic gRPC client connected to the embedding service
    ///
    /// The Channel maintains a connection pool and handles reconnection automatically.
    /// Cloning is cheap because Channel uses Arc internally.
    pub embed_client: EmbedServiceClient<tonic::transport::Channel>,
}

impl GRPCClient {
    /// Create a new GRPCClient instance
    ///
    /// # Parameters
    /// - `embed_client`: Pre-connected gRPC client (established during app startup)
    pub fn new(embed_client: EmbedServiceClient<tonic::transport::Channel>) -> Self {
        Self { embed_client }
    }

    /// Generate embeddings for blog post documents (storage/indexing)
    ///
    /// This method converts blog post content into vector embeddings for storage
    /// in the database (pgvector column). These embeddings enable semantic search.
    ///
    /// **Document vs Query embeddings:**
    /// The `task` parameter tells the model how to generate the embedding:
    /// - Document embeddings: Optimized for storage and retrieval
    /// - Query embeddings: Optimized for searching
    ///
    /// Many embedding models (like embeddinggemma) are trained with task-specific
    /// prefixes to generate better embeddings for different use cases.
    ///
    /// # Parameters
    /// - `raw_text`: Plain text content of the blog post (HTML stripped)
    /// - `title`: Post title (used as context for better embeddings)
    ///
    /// # Returns
    /// - `Ok(Vec<f32>)`: 768-dimensional vector (embeddinggemma output size)
    /// - `Err(HttpError)`: If gRPC call fails or service is unavailable
    ///
    /// # Rust ownership notes:
    /// Why do we clone embed_client?
    /// - `self` is an immutable reference (&self)
    /// - But gRPC methods require &mut self (they modify internal state)
    /// - Solution: Clone the client (cheap because Channel uses Arc)
    /// - This gives us an owned client we can mutate
    ///
    /// This pattern exists because:
    /// 1. GRPCClient is part of AppState, which is immutable
    /// 2. AppState is shared across all request handlers
    /// 3. We can't make the entire AppState mutable (would break concurrency)
    /// 4. Cloning the client is the idiomatic solution
    pub async fn get_embedding_docs(
        &self,
        raw_text: &str,
        title: &str,
    ) -> Result<Vec<f32>, HttpError> {
        // Build gRPC request with task-specific prefix
        // The task format follows embeddinggemma's expected format:
        // "title: {title} | text" tells the model this is document content
        let request = tonic::Request::new(EmbedRequest {
            text: raw_text.to_string(),
            task: format!("title: {} | text", title),
        });

        // Clone the client to get mutable access
        // This is necessary because:
        // - embed_query() requires &mut self
        // - We only have &self (immutable reference to GRPCClient)
        // - Channel cloning is cheap (Arc-based)
        let mut client = self.embed_client.clone();

        // Make the gRPC call asynchronously
        // - embed_query is the RPC method defined in the .proto file
        // - map_err converts tonic::Status errors to our HttpError type
        // - into_inner() extracts the response message from tonic's wrapper
        let response = client
            .embed_query(request)
            .await
            .map_err(|e| HttpError::server_error(e.to_string()))?
            .into_inner();

        // Extract the embedding vector from the response
        // This is a Vec<f32> with 768 dimensions (embeddinggemma output size)
        let embedding = response.embedding;
        Ok(embedding)
    }

    /// Generate embeddings for search queries (searching)
    ///
    /// This method converts user search queries into vector embeddings for
    /// comparing against stored document embeddings in the database.
    ///
    /// **Why separate methods for documents and queries?**
    /// Some embedding models are asymmetric - they perform better when:
    /// - Documents use one task prefix
    /// - Queries use a different task prefix
    ///
    /// This improves search relevance by optimizing the vector space for
    /// the document-query relationship.
    ///
    /// # Parameters
    /// - `q`: User's search query string
    ///
    /// # Returns
    /// - `Ok(Vec<f32>)`: Query embedding vector (same dimensionality as documents)
    /// - `Err(HttpError)`: If gRPC call fails
    ///
    /// # Example usage:
    /// ```
    /// // User searches for "rust web frameworks"
    /// let query_embedding = grpc_client.get_embedding_query("rust web frameworks").await?;
    ///
    /// // Find similar posts using pgvector's <=> operator (cosine distance)
    /// let similar_posts = db.find_similar_posts(query_embedding, limit: 10).await?;
    /// ```
    pub async fn get_embedding_query(&self, q: &str) -> Result<Vec<f32>, HttpError> {
        // Build gRPC request with query-specific task prefix
        // "task: search result | query" tells embeddinggemma this is a search query
        // This generates embeddings optimized for matching against document embeddings
        let request = tonic::Request::new(EmbedRequest {
            text: q.to_string(),
            task: "task: search result | query".to_string(),
        });

        // Clone client for mutable access (same pattern as get_embedding_docs)
        let mut client = self.embed_client.clone();

        // Make the gRPC call and extract the embedding
        let response = client
            .embed_query(request)
            .await
            .map_err(|e| HttpError::server_error(e.to_string()))?
            .into_inner();

        let embedding = response.embedding;
        Ok(embedding)
    }
}
