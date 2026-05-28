# @reactor/auth

Authentication client for Reactor. Handles user signup, signin, session management, and organization membership.

## Installation

```bash
npm install @reactor/auth @reactor/shared
```

Or use the unified client:

```bash
npm install @reactor/client
```

## Quick Start

```typescript
import { AuthClient } from '@reactor/auth';

const auth = new AuthClient(ctx, {
  persistSession: true,
  autoRefresh: true,
});

// Sign up
const { data, error } = await auth.signUp({
  email: 'user@example.com',
  password: 'securepassword',
});

// Sign in
const { data } = await auth.signIn({
  email: 'user@example.com',
  password: 'password',
});

// Get current user
const user = await auth.getUser();

// Sign out
await auth.signOut();
```

## Features

- **Session Management** - Automatic token refresh and persistence
- **Multi-tab Sync** - Session changes sync across browser tabs
- **Organization Support** - Create and manage organizations
- **Permissions** - Check user permissions
- **API Keys** - Create and manage API keys
- **Password Reset** - Request and confirm password resets
- **Email Verification** - Verify user email addresses

## API Reference

### Authentication

- `signUp(params)` - Create a new user account
- `signIn(params)` - Sign in with email/password
- `signOut()` - Sign out and revoke session
- `getSession()` - Get current session
- `getUser()` - Get current user
- `updateUser(params)` - Update user profile
- `deleteUser()` - Delete user account

### Password Reset

- `requestPasswordReset({ email })` - Request a reset email
- `confirmPasswordReset({ token, newPassword })` - Set new password

### Email Verification

- `verifyEmail({ token })` - Verify email with token
- `resendVerification({ email })` - Resend verification email

### Organizations

- `auth.orgs.create(params)` - Create organization
- `auth.orgs.list()` - List user's organizations
- `auth.orgs.get(id)` - Get organization
- `auth.orgs.update(id, params)` - Update organization
- `auth.orgs.delete(id)` - Delete organization

### Members

- `auth.orgs.members.list(orgId)` - List members
- `auth.orgs.members.update(orgId, userId, params)` - Update member
- `auth.orgs.members.remove(orgId, userId)` - Remove member

### API Keys

- `auth.apiKeys.list()` - List API keys
- `auth.apiKeys.create(params)` - Create API key
- `auth.apiKeys.revoke(id)` - Revoke API key

## Documentation

- [Authentication Guide](https://reactor.cloud/docs/auth)
- [Organizations](https://reactor.cloud/docs/auth#organizations)
- [API Reference](https://reactor.cloud/docs)

## License

MIT
