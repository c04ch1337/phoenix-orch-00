# Security Documentation

This document outlines the security features and configurations implemented in the Master Orchestrator.

## Authentication

### JWT-based Authentication
- JSON Web Token (JWT) based authentication has replaced the previous dev-token system
- JWTs are signed using a secret key configured via the `JWT_SECRET` environment variable
- Tokens include standard claims:
  - `sub`: Subject (user ID)
  - `exp`: Expiration timestamp
  - `iat`: Issued at timestamp
- Authentication is optional and can be enabled/disabled via the `JWT_SECRET` environment variable

### Configuration
```env
JWT_SECRET=your-secret-key-here
```

## Rate Limiting

Rate limiting is implemented to protect against abuse and DoS attacks.

### Configuration
```env
RATE_LIMIT_REQUESTS=100  # Number of requests allowed per window
RATE_LIMIT_WINDOW=60     # Time window in seconds
```

### Features
- Per-user rate limiting based on JWT claims
- Fallback to IP-based limiting for unauthenticated requests
- Configurable request quota and time window
- Returns HTTP 429 (Too Many Requests) when limit is exceeded

## Request Validation

All incoming requests are validated against JSON schemas to ensure data integrity and security.

### Validation Coverage
- Chat API requests
- WebSocket messages
- Authentication payloads

### Features
- Strict schema validation
- Prevents malformed or malicious payloads
- Detailed validation error messages
- Custom validation rules per endpoint

## Security Audit Logging

Comprehensive security event logging is implemented across the system.

### Logged Events
- Authentication attempts (success/failure)
- API access with detailed request information
- Rate limit violations
- Validation failures
- Configuration changes

### Audit Log Fields
- Event ID (UUID)
- Timestamp
- Event Type
- User ID (if authenticated)
- IP Address
- Resource/Endpoint
- Action
- Status
- Additional Details

### Integration
- Logs are written to the tracing system
- Events are stored in memory with future support for persistent storage
- Structured logging format for easy analysis

## Security Headers

The following security headers are automatically added to all HTTP responses:

```http
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Content-Security-Policy: default-src 'self'; script-src 'self'; connect-src 'self'; img-src 'self' data: https://grainy-gradients.vercel.app; style-src 'self' https://fonts.googleapis.com 'unsafe-inline'; font-src https://fonts.gstatic.com; frame-ancestors 'none';
```

## CORS Configuration

Cross-Origin Resource Sharing (CORS) is configured with the following settings:

- Allowed Origins: 
  - http://localhost:8181
  - http://127.0.0.1:8181
- Allowed Methods: GET, POST, OPTIONS
- Allowed Headers: Authorization, Content-Type
- Credentials: Supported
- Max Age: 3600 seconds

## Best Practices

1. **Environment Variables**
   - Never commit sensitive values to version control
   - Use strong, unique secrets for JWT signing
   - Rotate secrets periodically

2. **Rate Limiting**
   - Adjust limits based on your application's needs
   - Monitor rate limit violations for potential attacks

3. **Audit Logs**
   - Regularly review audit logs for suspicious activity
   - Set up alerts for security-related events
   - Implement log rotation and retention policies

4. **Authentication**
   - Use short-lived JWTs
   - Implement token refresh mechanism for long-running sessions
   - Consider implementing token revocation for critical security events

## Future Improvements

1. Persistent storage for audit logs
2. Token revocation mechanism
3. Enhanced rate limiting strategies
4. Additional security headers
5. IP allowlist/blocklist functionality