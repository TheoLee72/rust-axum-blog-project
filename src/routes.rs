use axum::{Router, middleware};
use tower_http::trace::TraceLayer;

use crate::{
    AppState,
    handler::{
        auth::auth_handler, comment::comment_handler, newsletter::newsletter_handler,
        post::post_handler, search::search_handler, users::users_handler,
    },
    middleware::auth,
};

/// Create the main application router with all routes and middleware
///
/// This function builds the complete routing structure for the application.
/// All routes are nested under "/api" prefix (e.g., "/api/auth/login").
///
/// Router architecture:
/// - `/api/search/*` - Search endpoints (full-text and semantic search)
/// - `/api/auth/*` - Authentication endpoints (register, login, refresh, etc.)
/// - `/api/users/*` - User management (protected by auth middleware)
/// - `/api/posts/*` - Blog post operations (CRUD)
/// - `/api/comments/*` - Comment operations
/// - `/api/newsletter/*` - Newsletter subscription management
/// Key methods:
/// - `.nest(path, router)`: Groups routes under a path prefix. Nests an entire Router.
///   Example: `.nest("/users", user_router)` makes routes like "/users/profile", "/users/:id"
///   The nested router doesn't see the prefix - it only sees the remaining path.
///   
/// - `.route(path, handler)`: Adds a single route with a specific HTTP method handler.
///   Example: `.route("/login", post(login_handler))` creates POST /login endpoint.
///   Used inside handler modules, not shown here.
///
/// The difference:
/// - `nest`: Takes a complete Router (which can have multiple routes inside)
/// - `route`: Takes a single path and method handler
///
/// Middleware layers:
/// - TraceLayer: HTTP request/response logging for observability
/// - Auth middleware: Applied specifically to /users routes (see below)
///
/// # Parameters
/// - `app_state`: Shared application state (database, Redis, config, etc.)
pub fn create_router(app_state: AppState) -> Router {
    let api_route = Router::new()
        // Search routes - public access
        // Handles both full-text search and vector similarity search
        .nest("/search", search_handler())
        // Authentication routes - public access (login, register, token refresh)
        // Pass app_state for database and Redis access
        .nest("/auth", auth_handler(app_state.clone()))
        // User management routes - PROTECTED by authentication middleware
        // The layer() applies the auth middleware to ALL routes in users_handler()
        // This means users must have a valid JWT token to access any /users/* endpoint
        //
        // Middleware execution order:
        // 1. Request comes in
        // 2. Auth middleware checks JWT token
        // 3. If valid, user info is added to request extensions
        // 4. Request proceeds to the actual handler
        // 5. If invalid, 401 error is returned immediately
        .nest(
            "/users",
            users_handler().layer(middleware::from_fn_with_state(app_state.clone(), auth)),
        )
        // Blog post routes - mixed public/protected endpoints
        // Individual handlers decide which routes require authentication
        .nest("/posts", post_handler(app_state.clone()))
        // Comment routes - typically public read, protected write
        .nest("/comments", comment_handler(app_state.clone()))
        // Newsletter subscription routes - public access
        .nest("/newsletter", newsletter_handler())
        // Apply TraceLayer middleware to ALL routes
        // This logs HTTP requests and responses for debugging and monitoring
        // Useful for production observability (request duration, status codes, etc.)
        .layer(TraceLayer::new_for_http())
        // Attach application state to the router
        // This makes app_state available to all handlers via State extractor
        //
        // Modern approach vs. legacy:
        // - Old: .layer(Extension(app_state)) - stored in request extensions
        // - New: .with_state(app_state) - type-safe, better error messages
        //
        // Note: with_state() must be called on the final Router, not on nested routers
        // That's why it can't be used with .route_layer()
        .with_state(app_state);

    // Wrap all API routes under the "/api" prefix
    // This creates a clear separation and allows for:
    // - Easier versioning (could add /api/v2 later)
    // - Clear distinction between API and other routes (e.g., static files, webhooks)
    // - Simplified reverse proxy configuration (forward all /api/* to backend)
    Router::new().nest("/api", api_route)
}
