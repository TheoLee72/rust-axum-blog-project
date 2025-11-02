# Rust Axum Blog Backend

A production-ready, feature-rich blog backend API built with Rust, Axum, and PostgreSQL. This project demonstrates modern web development practices in Rust and serves as a comprehensive learning resource for intermediate to advanced Rust developers.

## 🚀 Features

### Core Functionality

- **User Authentication & Authorization**

  - JWT-based authentication with refresh tokens
  - Email verification system
  - Password reset functionality
  - Role-based access control (Admin/User)
  - Rate limiting on login attempts via Redis
  - Secure password hashing with Argon2

- **Blog Post Management**

  - Full CRUD operations for blog posts
  - Image upload support (multipart form data)
  - Automatic text extraction from HTML content
  - AI-powered content summarization
  - Tag system for categorization
  - Pagination and filtering

- **Advanced Search**

  - Hybrid search combining full-text and semantic search
  - PostgreSQL full-text search with tsvector
  - Vector similarity search using pgvector
  - Real-time embedding generation via gRPC service

- **Comments System**

  - Nested comments support
  - Comment moderation capabilities
  - User-specific comment management

- **Newsletter Management**
  - Email subscription system
  - Automated welcome emails
  - Unsubscribe functionality

### Technical Features

- **Async/Await**: Fully asynchronous with Tokio runtime
- **Connection Pooling**: Efficient database and Redis connection management
- **Middleware**: Custom authentication and IP extraction middleware
- **CORS Configuration**: Flexible cross-origin resource sharing
- **Structured Logging**: Request tracing with tracing-subscriber
- **Error Handling**: Comprehensive custom error types
- **Database Migrations**: Version-controlled schema with SQLx migrations
- **Background Tasks**: Scheduled cleanup jobs with tokio-cron-scheduler
- **gRPC Integration**: Communication with external embedding service
- **Email Service**: HTML email templates with Lettre
- **HTML Sanitization**: XSS protection with Ammonia

## 📋 Prerequisites

- **Rust**: 1.70+ (edition 2024)
- **PostgreSQL**: 14+ with pgvector extension
- **Redis**: 6.0+
- **gRPC Embedding Service**: External service for text embeddings (768-dimensional vectors)

## 🛠️ Installation

### 1. Clone the Repository

```bash
git clone https://github.com/TheoLee72/rust-axum-blog-project.git
cd rust-axum-blog-project
```

### 2. Set Up Environment Variables

Create a `.env` file in the project root:

### 3. Database Setup

```bash
# Install SQLx CLI
cargo install sqlx-cli --no-default-features --features postgres

# Create database
createdb dbname

# Run migrations
sqlx migrate run
```

The migrations will create:

- Users table with role-based access
- Posts table with vector embeddings
- Comments table with nested structure
- Tags system
- Newsletter subscriptions
- Full-text search indexes

### 4. Install Dependencies

```bash
cargo build --release
```

### 5. Run the Server

**Development mode:**

```bash
cargo run
```

**Production mode:**

```bash
cargo run --release
```

The server will start at `http://localhost:8000` (or your configured PORT).

## 📚 API Documentation

Base URL: `http://localhost:8000/api`

### Authentication Endpoints (`/api/auth`)

| Method | Endpoint           | Description               | Auth Required |
| ------ | ------------------ | ------------------------- | ------------- |
| POST   | `/register`        | Create new user account   | No            |
| POST   | `/login`           | Login with credentials    | No            |
| GET    | `/verify`          | Verify email address      | No            |
| POST   | `/forgot-password` | Request password reset    | No            |
| POST   | `/reset-password`  | Reset password with token | No            |
| POST   | `/refresh`         | Refresh access token      | No            |

### User Management (`/api/users`)

| Method | Endpoint     | Description              | Auth Required |
| ------ | ------------ | ------------------------ | ------------- |
| GET    | `/me`        | Get current user profile | Yes           |
| GET    | `/users`     | Get all users (admin)    | Yes           |
| PUT    | `/username`  | Update username          | Yes           |
| PUT    | `/role`      | Update user role (admin) | Yes           |
| PUT    | `/password`  | Change password          | Yes           |
| PUT    | `/email`     | Update email address     | Yes           |
| POST   | `/logout`    | Logout user              | Yes           |
| DELETE | `/delete-me` | Delete account           | Yes           |

### Blog Posts (`/api/posts`)

| Method | Endpoint                           | Description            | Auth Required     |
| ------ | ---------------------------------- | ---------------------- | ----------------- |
| GET    | `/?page=2&limit=5&user_username=3` | List posts (paginated) | No                |
| GET    | `/:id`                             | Get single post        | No                |
| POST   | `/`                                | Create new post        | Yes               |
| PUT    | `/:id`                             | Update post            | Yes (owner/admin) |
| DELETE | `/:id`                             | Delete post            | Yes (owner/admin) |
| POST   | `/uploads`                         | Upload image           | Yes (admin)       |

### Comments (`/api`)

| Method | Endpoint                                                        | Description       | Auth Required |
| ------ | --------------------------------------------------------------- | ----------------- | ------------- |
| GET    | `/posts/:post_id/comments?page=1&limit=10&sort=created_at_desc` | Get post comments | No            |
| POST   | `/posts/:post_id/comments`                                      | Create comment    | Yes           |
| PUT    | `/comments/:comment_id`                                         | Edit comment      | Yes (owner)   |
| DELETE | `/comments/:comment_id`                                         | Delete comment    | Yes (owner)   |

### Search (`/api/search`)

| Method | Endpoint                     | Description                          | Auth Required |
| ------ | ---------------------------- | ------------------------------------ | ------------- |
| GET    | `/?q=memory&page=1&limit=10` | Hybrid search (full-text + semantic) | No            |

Query parameters:

- `q`: Search query string
- `page`: Page number (default: 1)
- `limit`: Results per page (default: 10)

### Newsletter (`/api/newsletter`)

| Method | Endpoint | Description                 | Auth Required |
| ------ | -------- | --------------------------- | ------------- |
| POST   | `/`      | Subscribe to newsletter     | No            |
| DELETE | `/`      | Unsubscribe from newsletter | No            |

## 🏗️ Project Structure

```
blog_backend/
├── src/
│   ├── main.rs              # Application entry point & server setup
│   ├── config.rs            # Environment configuration
│   ├── routes.rs            # Route definitions
│   ├── models.rs            # Database models
│   ├── dtos.rs              # Data transfer objects
│   ├── error.rs             # Error handling
│   ├── db.rs                # Database client wrapper
│   ├── redisdb.rs           # Redis client wrapper
│   ├── grpc.rs              # gRPC client for embeddings
│   ├── http.rs              # HTTP client wrapper
│   ├── middleware.rs        # Custom middleware (auth, etc.)
│   ├── tracing_config.rs    # Logging configuration
│   ├── utils.rs             # Utility functions
│   ├── handler/             # Request handlers
│   │   ├── auth.rs          # Authentication logic
│   │   ├── users.rs         # User management
│   │   ├── post.rs          # Blog post operations
│   │   ├── comment.rs       # Comment handling
│   │   ├── search.rs        # Search functionality
│   │   └── newsletter.rs    # Newsletter management
│   ├── db/                  # Database operations
│   │   ├── user.rs          # User queries
│   │   ├── post.rs          # Post queries
│   │   ├── comment.rs       # Comment queries
│   │   ├── newsletter.rs    # Newsletter queries
│   │   └── scheduler.rs     # Background tasks
│   ├── mail/                # Email functionality
│   │   ├── sendmail.rs      # Email sending logic
│   │   ├── mails.rs         # Email templates
│   │   └── templates/       # HTML email templates
│   └── utils/
│       ├── password.rs      # Password hashing
│       └── token.rs         # JWT token management
├── migrations/              # Database migrations
├── proto/                   # Protocol buffer definitions
│   └── embed.proto          # Embedding service proto
├── Cargo.toml              # Dependencies
├── build.rs                # Build script (proto compilation)
└── .env                    # Environment variables (not in repo)
```

## 🔧 Key Technologies

- **[Axum](https://github.com/tokio-rs/axum)**: Modern web framework
- **[Tokio](https://tokio.rs/)**: Async runtime
- **[SQLx](https://github.com/launchbadge/sqlx)**: Async SQL toolkit
- **[pgvector](https://github.com/pgvector/pgvector)**: Vector similarity search
- **[Redis](https://redis.io/)**: Caching and session management
- **[Tonic](https://github.com/hyperium/tonic)**: gRPC framework
- **[Lettre](https://github.com/lettre/lettre)**: Email library
- **[jsonwebtoken](https://github.com/Keats/jsonwebtoken)**: JWT implementation
- **[Argon2](https://github.com/RustCrypto/password-hashes)**: Password hashing
- **[tracing](https://github.com/tokio-rs/tracing)**: Structured logging
- **[tower-http](https://github.com/tower-rs/tower-http)**: HTTP middleware

## 🧪 Development

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

### Database Migrations

Create a new migration:

```bash
sqlx migrate add <migration_name>
```

Apply migrations:

```bash
sqlx migrate run
```

Revert last migration:

```bash
sqlx migrate revert
```

## 🔒 Security Features

- **Password Security**: Argon2 hashing with salt
- **JWT Tokens**: Secure token generation with expiration
- **Rate Limiting**: Login attempt limiting via Redis
- **HTML Sanitization**: XSS protection with Ammonia
- **SQL Injection Prevention**: Parameterized queries with SQLx
- **CORS Configuration**: Controlled cross-origin access
- **Role-Based Access**: Admin/User role separation

## 📝 Environment-Specific Behavior

### Development Mode

- IP extraction from socket connection info

### Production Mode

- IP extraction from Cloudflare headers (`CF-Connecting-IP`)

- Change frontend url when you are using it for production.

## 🤝 Contributing

Contributions are welcome! This project is designed to help Rust learners understand modern web development practices.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 👨‍💻 Author

**TheoLee72**

## 🙏 Acknowledgments

This project was created as a learning resource for the Rust community. It demonstrates:

- Production-ready Rust web application architecture
- Modern async/await patterns
- Database integration with migrations
- Authentication and authorization
- API design best practices
- Integration with external services (gRPC, Redis)

Perfect for developers learning:

- Rust web development
- Axum framework
- Async programming in Rust
- Database operations with SQLx
- JWT authentication
- Microservices communication

## 📞 Support

If you find this project helpful, please consider giving it a ⭐ on GitHub!

For questions or issues, please open an issue on the GitHub repository.
