[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / auth/src

# auth/src

## Functions

### detectSessionInUrl()

> **detectSessionInUrl**(): [`DetectedToken`](#detectedtoken)

Defined in: auth/src/url-detect.ts:24

Detect session or verification tokens from URL.

Supports:
- Query parameter: ?token=... (email verification, password reset)
- Query parameter: ?invite_token=... (invitation acceptance)
- Hash fragment: #access_token=...&refresh_token=... (OAuth)

#### Returns

[`DetectedToken`](#detectedtoken)

***

### cleanUrlAfterDetection()

> **cleanUrlAfterDetection**(): `void`

Defined in: auth/src/url-detect.ts:74

Clean detected tokens from the current URL.
Updates browser history without reloading.

#### Returns

`void`

***

### detectAndClean()

> **detectAndClean**(`cleanUrl?`): [`DetectedToken`](#detectedtoken)

Defined in: auth/src/url-detect.ts:108

Detect and return token info, optionally cleaning the URL.

#### Parameters

##### cleanUrl?

`boolean` = `true`

#### Returns

[`DetectedToken`](#detectedtoken)

## Classes

### ApiKeysClient

Defined in: auth/src/api-keys.ts:16

API keys client for managing user API keys.

#### Constructors

##### Constructor

> **new ApiKeysClient**(`ctx`): [`ApiKeysClient`](#apikeysclient)

Defined in: auth/src/api-keys.ts:17

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`ApiKeysClient`](#apikeysclient)

#### Methods

##### list()

> **list**(`params?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`ApiKey`](#apikey)[]\>\>

Defined in: auth/src/api-keys.ts:22

List API keys for the current user.

###### Parameters

###### params?

`PaginationParams`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`ApiKey`](#apikey)[]\>\>

##### create()

> **create**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`ApiKey`](#apikey) & `object`\>\>

Defined in: auth/src/api-keys.ts:37

Create a new API key.
Returns the full key value only once - store it securely.

###### Parameters

###### params

[`CreateApiKeyParams`](#createapikeyparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`ApiKey`](#apikey) & `object`\>\>

##### revoke()

> **revoke**(`keyId`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/api-keys.ts:44

Revoke an API key.

###### Parameters

###### keyId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

***

### AuthClient

Defined in: auth/src/client.ts:42

Authentication client for Reactor.

Handles user authentication, session management, token refresh, and multi-tab sync.

#### Constructors

##### Constructor

> **new AuthClient**(`ctx`, `options?`): [`AuthClient`](#authclient)

Defined in: auth/src/client.ts:54

###### Parameters

###### ctx

`RequestContext`

###### options?

[`AuthClientOptions`](#authclientoptions) = `{}`

###### Returns

[`AuthClient`](#authclient)

#### Methods

##### initialize()

> **initialize**(`detectUrl?`): `Promise`\<`void`\>

Defined in: auth/src/client.ts:103

Initialize the auth client.
Loads session from storage and handles URL tokens.

###### Parameters

###### detectUrl?

`boolean` = `true`

###### Returns

`Promise`\<`void`\>

##### signUp()

> **signUp**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `user`: [`User`](#user); `session`: [`Session`](#session); \}\>\>

Defined in: auth/src/client.ts:177

Sign up a new user.

###### Parameters

###### params

[`SignUpParams`](#signupparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `user`: [`User`](#user); `session`: [`Session`](#session); \}\>\>

##### signIn()

> **signIn**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `user`: [`User`](#user); `session`: [`Session`](#session); \}\>\>

Defined in: auth/src/client.ts:204

Sign in with email and password.

###### Parameters

###### params

[`SignInParams`](#signinparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `user`: [`User`](#user); `session`: [`Session`](#session); \}\>\>

##### signOut()

> **signOut**(): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/client.ts:231

Sign out the current user.
Revokes the refresh token server-side.

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### getSession()

> **getSession**(): `Promise`\<[`Session`](#session)\>

Defined in: auth/src/client.ts:250

Get the current session.
Refreshes automatically if near expiry.

###### Returns

`Promise`\<[`Session`](#session)\>

##### getUser()

> **getUser**(): `Promise`\<[`Result`](../client/src.md#result-1)\<[`User`](#user)\>\>

Defined in: auth/src/client.ts:259

Get the current user.
Fetches from server if not cached.

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`User`](#user)\>\>

##### updateUser()

> **updateUser**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`User`](#user)\>\>

Defined in: auth/src/client.ts:285

Update the current user's profile.

###### Parameters

###### params

[`UpdateUserParams`](#updateuserparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`User`](#user)\>\>

##### verifyEmail()

> **verifyEmail**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `verified`: `boolean`; \}\>\>

Defined in: auth/src/client.ts:306

Verify an email address with a token.

###### Parameters

###### params

###### token

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `verified`: `boolean`; \}\>\>

##### resendVerification()

> **resendVerification**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/client.ts:333

Resend email verification.

###### Parameters

###### params

[`ResendVerificationParams`](#resendverificationparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### requestPasswordReset()

> **requestPasswordReset**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/client.ts:340

Request a password reset email.

###### Parameters

###### params

[`RequestPasswordResetParams`](#requestpasswordresetparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### confirmPasswordReset()

> **confirmPasswordReset**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/client.ts:349

Confirm a password reset with the token and new password.

###### Parameters

###### params

[`ConfirmPasswordResetParams`](#confirmpasswordresetparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### refreshSession()

> **refreshSession**(): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Session`](#session)\>\>

Defined in: auth/src/client.ts:359

Manually refresh the session.

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Session`](#session)\>\>

##### setSession()

> **setSession**(`session`): `Promise`\<`void`\>

Defined in: auth/src/client.ts:402

Set the session manually (for SSR scenarios).

###### Parameters

###### session

[`Session`](#session)

###### Returns

`Promise`\<`void`\>

##### onAuthStateChange()

> **onAuthStateChange**(`callback`): [`AuthStateSubscription`](../client/src.md#authstatesubscription)

Defined in: auth/src/client.ts:409

Subscribe to auth state changes.

###### Parameters

###### callback

[`AuthStateChangeCallback`](#authstatechangecallback)

###### Returns

[`AuthStateSubscription`](../client/src.md#authstatesubscription)

##### getAccessToken()

> **getAccessToken**(): `string`

Defined in: auth/src/client.ts:417

Get the access token for making authenticated requests.
Used internally by the request context.

###### Returns

`string`

##### deleteUser()

> **deleteUser**(): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/client.ts:424

Delete the current user's account.

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

#### Properties

##### orgs

> `readonly` **orgs**: [`OrgsClient`](#orgsclient)

Defined in: auth/src/client.ts:48

Organizations client

##### permissions

> `readonly` **permissions**: [`PermissionsClient`](#permissionsclient)

Defined in: auth/src/client.ts:50

Permissions client

##### apiKeys

> `readonly` **apiKeys**: [`ApiKeysClient`](#apikeysclient)

Defined in: auth/src/client.ts:52

API keys client

***

### OrgsClient

Defined in: auth/src/orgs.ts:25

Organizations client for managing organizations, members, and invitations.

#### Accessors

##### invitations

###### Get Signature

> **get** **invitations**(): [`InvitationsClient`](#invitationsclient)

Defined in: auth/src/orgs.ts:84

Get the invitations client.

###### Returns

[`InvitationsClient`](#invitationsclient)

#### Constructors

##### Constructor

> **new OrgsClient**(`ctx`): [`OrgsClient`](#orgsclient)

Defined in: auth/src/orgs.ts:26

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`OrgsClient`](#orgsclient)

#### Methods

##### list()

> **list**(`params?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)[]\>\>

Defined in: auth/src/orgs.ts:31

List organizations the current user belongs to.

###### Parameters

###### params?

`PaginationParams`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)[]\>\>

##### get()

> **get**(`idOrSlug`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)\>\>

Defined in: auth/src/orgs.ts:45

Get an organization by ID or slug.

###### Parameters

###### idOrSlug

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)\>\>

##### create()

> **create**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)\>\>

Defined in: auth/src/orgs.ts:52

Create a new organization.

###### Parameters

###### params

[`CreateOrgParams`](#createorgparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)\>\>

##### update()

> **update**(`idOrSlug`, `params`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)\>\>

Defined in: auth/src/orgs.ts:59

Update an organization.

###### Parameters

###### idOrSlug

`string`

###### params

[`UpdateOrgParams`](#updateorgparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Organization`](#organization)\>\>

##### delete()

> **delete**(`idOrSlug`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/orgs.ts:70

Delete an organization.

###### Parameters

###### idOrSlug

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### members()

> **members**(`orgId`): [`MembersClient`](#membersclient)

Defined in: auth/src/orgs.ts:77

Get a members client scoped to an organization.

###### Parameters

###### orgId

`string`

###### Returns

[`MembersClient`](#membersclient)

##### listRoles()

> **listRoles**(): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Role`](#role)[]\>\>

Defined in: auth/src/orgs.ts:91

List available roles.

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Role`](#role)[]\>\>

***

### MembersClient

Defined in: auth/src/orgs.ts:99

Members client for managing organization members.

#### Constructors

##### Constructor

> **new MembersClient**(`ctx`, `orgId`): [`MembersClient`](#membersclient)

Defined in: auth/src/orgs.ts:100

###### Parameters

###### ctx

`RequestContext`

###### orgId

`string`

###### Returns

[`MembersClient`](#membersclient)

#### Methods

##### list()

> **list**(`params?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)[]\>\>

Defined in: auth/src/orgs.ts:108

List members of the organization.

###### Parameters

###### params?

`PaginationParams`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)[]\>\>

##### get()

> **get**(`userId`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)\>\>

Defined in: auth/src/orgs.ts:122

Get a specific member.

###### Parameters

###### userId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)\>\>

##### invite()

> **invite**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Invitation`](#invitation)\>\>

Defined in: auth/src/orgs.ts:132

Invite a user to the organization.

###### Parameters

###### params

[`CreateInvitationParams`](#createinvitationparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Invitation`](#invitation)\>\>

##### updateRole()

> **updateRole**(`userId`, `params`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)\>\>

Defined in: auth/src/orgs.ts:143

Update a member's role.

###### Parameters

###### userId

`string`

###### params

[`UpdateMemberParams`](#updatememberparams)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)\>\>

##### remove()

> **remove**(`userId`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/orgs.ts:154

Remove a member from the organization.

###### Parameters

###### userId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

***

### InvitationsClient

Defined in: auth/src/orgs.ts:165

Invitations client for managing organization invitations.

#### Constructors

##### Constructor

> **new InvitationsClient**(`ctx`): [`InvitationsClient`](#invitationsclient)

Defined in: auth/src/orgs.ts:166

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`InvitationsClient`](#invitationsclient)

#### Methods

##### list()

> **list**(`params?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Invitation`](#invitation)[]\>\>

Defined in: auth/src/orgs.ts:171

List pending invitations for the current user.

###### Parameters

###### params?

`PaginationParams`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Invitation`](#invitation)[]\>\>

##### accept()

> **accept**(`params`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)\>\>

Defined in: auth/src/orgs.ts:185

Accept an invitation.

###### Parameters

###### params

###### token

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Member`](#member)\>\>

##### revoke()

> **revoke**(`invitationId`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: auth/src/orgs.ts:192

Revoke an invitation (org admin only).

###### Parameters

###### invitationId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

***

### PermissionsClient

Defined in: auth/src/permissions.ts:13

Permissions client for checking user permissions.

#### Constructors

##### Constructor

> **new PermissionsClient**(`ctx`): [`PermissionsClient`](#permissionsclient)

Defined in: auth/src/permissions.ts:14

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`PermissionsClient`](#permissionsclient)

#### Methods

##### get()

> **get**(`options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`PermissionsResponse`](#permissionsresponse)\>\>

Defined in: auth/src/permissions.ts:20

Get all permissions for the current user.
Optionally scoped to a specific organization.

###### Parameters

###### options?

###### org?

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`PermissionsResponse`](#permissionsresponse)\>\>

##### check()

> **check**(`permissions`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `allowed`: `boolean`; `missing?`: `string`[]; \}\>\>

Defined in: auth/src/permissions.ts:36

Check if the current user has specific permissions.
Returns true if all requested permissions are granted.

###### Parameters

###### permissions

`string`[]

###### options?

###### org?

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `allowed`: `boolean`; `missing?`: `string`[]; \}\>\>

##### resolveContext()

> **resolveContext**(): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `org?`: \{ `id`: `string`; `slug`: `string`; `role_id`: `string`; \}; \}\>\>

Defined in: auth/src/permissions.ts:54

Resolve the current organization context.
Returns the organization determined by the X-Reactor-Org header or default.

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `org?`: \{ `id`: `string`; `slug`: `string`; `role_id`: `string`; \}; \}\>\>

***

### AuthStateManager

Defined in: auth/src/state.ts:18

Manages auth state including session storage, token refresh, and multi-tab sync.

#### Constructors

##### Constructor

> **new AuthStateManager**(`storage`, `storageKey?`, `autoRefresh?`, `persistSession?`, `onRefresh?`): [`AuthStateManager`](#authstatemanager)

Defined in: auth/src/state.ts:25

###### Parameters

###### storage

[`StorageAdapter`](../client/src.md#storageadapter)

###### storageKey?

`string` = `STORAGE_KEY_DEFAULT`

###### autoRefresh?

`boolean` = `true`

###### persistSession?

`boolean` = `true`

###### onRefresh?

() => `Promise`\<[`Session`](#session)\>

###### Returns

[`AuthStateManager`](#authstatemanager)

#### Methods

##### initialize()

> **initialize**(): `Promise`\<[`Session`](#session)\>

Defined in: auth/src/state.ts:36

Initialize state from storage.

###### Returns

`Promise`\<[`Session`](#session)\>

##### getSession()

> **getSession**(): [`Session`](#session)

Defined in: auth/src/state.ts:74

Get current session.

###### Returns

[`Session`](#session)

##### getUser()

> **getUser**(): [`User`](#user)

Defined in: auth/src/state.ts:81

Get current user.

###### Returns

[`User`](#user)

##### setSession()

> **setSession**(`session`, `event`): `Promise`\<`void`\>

Defined in: auth/src/state.ts:88

Set the current session.

###### Parameters

###### session

[`Session`](#session)

###### event

[`AuthStateEvent`](#authstateevent)

###### Returns

`Promise`\<`void`\>

##### clearSession()

> **clearSession**(): `Promise`\<`void`\>

Defined in: auth/src/state.ts:120

Clear the current session.

###### Returns

`Promise`\<`void`\>

##### onAuthStateChange()

> **onAuthStateChange**(`callback`): `object`

Defined in: auth/src/state.ts:127

Subscribe to auth state changes.

###### Parameters

###### callback

[`AuthStateChangeCallback`](#authstatechangecallback)

###### Returns

`object`

###### unsubscribe

> **unsubscribe**: () => `void`

###### Returns

`void`

##### setRefreshCallback()

> **setRefreshCallback**(`onRefresh`): `void`

Defined in: auth/src/state.ts:145

Set the refresh callback.

###### Parameters

###### onRefresh

() => `Promise`\<[`Session`](#session)\>

###### Returns

`void`

##### refresh()

> **refresh**(): `Promise`\<[`Session`](#session)\>

Defined in: auth/src/state.ts:157

Manually trigger a refresh.

###### Returns

`Promise`\<[`Session`](#session)\>

##### destroy()

> **destroy**(): `void`

Defined in: auth/src/state.ts:270

Cleanup resources.

###### Returns

`void`

## Interfaces

### AuthClientOptions

Defined in: auth/src/types.ts:18

Configuration options for the auth client.

#### Properties

##### storage?

> `optional` **storage?**: [`StorageAdapter`](../client/src.md#storageadapter)

Defined in: auth/src/types.ts:20

Storage adapter for session persistence

##### storageKey?

> `optional` **storageKey?**: `string`

Defined in: auth/src/types.ts:22

Storage key for the session (default: 'reactor.session')

##### autoRefresh?

> `optional` **autoRefresh?**: `boolean`

Defined in: auth/src/types.ts:24

Whether to automatically refresh tokens (default: true)

##### persistSession?

> `optional` **persistSession?**: `boolean`

Defined in: auth/src/types.ts:26

Whether to persist sessions to storage (default: true)

##### detectSessionInUrl?

> `optional` **detectSessionInUrl?**: `boolean`

Defined in: auth/src/types.ts:28

Whether to detect sessions from URL (default: true)

***

### SignUpParams

Defined in: auth/src/types.ts:42

Sign up request parameters.

#### Properties

##### email

> **email**: `string`

Defined in: auth/src/types.ts:43

##### password

> **password**: `string`

Defined in: auth/src/types.ts:44

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: auth/src/types.ts:45

***

### SignInParams

Defined in: auth/src/types.ts:51

Sign in request parameters.

#### Properties

##### email

> **email**: `string`

Defined in: auth/src/types.ts:52

##### password

> **password**: `string`

Defined in: auth/src/types.ts:53

***

### UpdateUserParams

Defined in: auth/src/types.ts:59

Update user request parameters.

#### Properties

##### email?

> `optional` **email?**: `string`

Defined in: auth/src/types.ts:60

##### password?

> `optional` **password?**: `string`

Defined in: auth/src/types.ts:61

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: auth/src/types.ts:62

***

### RequestPasswordResetParams

Defined in: auth/src/types.ts:68

Password reset request parameters.

#### Properties

##### email

> **email**: `string`

Defined in: auth/src/types.ts:69

***

### ConfirmPasswordResetParams

Defined in: auth/src/types.ts:75

Password reset confirmation parameters.

#### Properties

##### token

> **token**: `string`

Defined in: auth/src/types.ts:76

##### newPassword

> **newPassword**: `string`

Defined in: auth/src/types.ts:77

***

### VerifyEmailParams

Defined in: auth/src/types.ts:83

Email verification parameters.

#### Properties

##### token

> **token**: `string`

Defined in: auth/src/types.ts:84

***

### ResendVerificationParams

Defined in: auth/src/types.ts:90

Resend verification parameters.

#### Properties

##### email

> **email**: `string`

Defined in: auth/src/types.ts:91

***

### CreateOrgParams

Defined in: auth/src/types.ts:97

Organization creation parameters.

#### Properties

##### slug

> **slug**: `string`

Defined in: auth/src/types.ts:98

##### name

> **name**: `string`

Defined in: auth/src/types.ts:99

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: auth/src/types.ts:100

***

### UpdateOrgParams

Defined in: auth/src/types.ts:106

Organization update parameters.

#### Properties

##### name?

> `optional` **name?**: `string`

Defined in: auth/src/types.ts:107

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: auth/src/types.ts:108

***

### CreateInvitationParams

Defined in: auth/src/types.ts:114

Invitation creation parameters.

#### Properties

##### email

> **email**: `string`

Defined in: auth/src/types.ts:115

##### roleId?

> `optional` **roleId?**: `string`

Defined in: auth/src/types.ts:116

***

### UpdateMemberParams

Defined in: auth/src/types.ts:122

Member update parameters.

#### Properties

##### roleId

> **roleId**: `string`

Defined in: auth/src/types.ts:123

***

### CreateApiKeyParams

Defined in: auth/src/types.ts:129

API key creation parameters.

#### Properties

##### name

> **name**: `string`

Defined in: auth/src/types.ts:130

##### scopes?

> `optional` **scopes?**: `string`[]

Defined in: auth/src/types.ts:131

##### expiresAt?

> `optional` **expiresAt?**: `string`

Defined in: auth/src/types.ts:132

***

### PermissionsResponse

Defined in: auth/src/types.ts:177

Permissions response.

#### Properties

##### permissions

> **permissions**: `string`[]

Defined in: auth/src/types.ts:178

##### org?

> `optional` **org?**: `object`

Defined in: auth/src/types.ts:179

###### id

> **id**: `string`

###### slug

> **slug**: `string`

###### role\_id

> **role\_id**: `string`

***

### DetectedToken

Defined in: auth/src/url-detect.ts:9

Result of URL token detection.

#### Properties

##### type

> **type**: [`DetectedTokenType`](#detectedtokentype)

Defined in: auth/src/url-detect.ts:10

##### token

> **token**: `string`

Defined in: auth/src/url-detect.ts:11

##### params?

> `optional` **params?**: `Record`\<`string`, `string`\>

Defined in: auth/src/url-detect.ts:13

Additional parameters from the URL

***

### User

Defined in: shared/dist/index.d.ts:431

User object returned from auth endpoints.

#### Properties

##### id

> **id**: `string`

Defined in: shared/dist/index.d.ts:432

##### email

> **email**: `string`

Defined in: shared/dist/index.d.ts:433

##### email\_verified

> **email\_verified**: `boolean`

Defined in: shared/dist/index.d.ts:434

##### metadata

> **metadata**: `Record`\<`string`, `unknown`\>

Defined in: shared/dist/index.d.ts:435

##### created\_at

> **created\_at**: `string`

Defined in: shared/dist/index.d.ts:436

***

### Session

Defined in: shared/dist/index.d.ts:441

Session object containing tokens.

#### Properties

##### access\_token

> **access\_token**: `string`

Defined in: shared/dist/index.d.ts:442

##### refresh\_token

> **refresh\_token**: `string`

Defined in: shared/dist/index.d.ts:443

##### expires\_at

> **expires\_at**: `string`

Defined in: shared/dist/index.d.ts:444

##### user

> **user**: [`User`](#user)

Defined in: shared/dist/index.d.ts:445

***

### Organization

Defined in: shared/dist/index.d.ts:450

Organization object.

#### Properties

##### id

> **id**: `string`

Defined in: shared/dist/index.d.ts:451

##### slug

> **slug**: `string`

Defined in: shared/dist/index.d.ts:452

##### name

> **name**: `string`

Defined in: shared/dist/index.d.ts:453

##### metadata

> **metadata**: `Record`\<`string`, `unknown`\>

Defined in: shared/dist/index.d.ts:454

##### created\_at

> **created\_at**: `string`

Defined in: shared/dist/index.d.ts:455

##### updated\_at

> **updated\_at**: `string`

Defined in: shared/dist/index.d.ts:456

***

### Member

Defined in: shared/dist/index.d.ts:461

Organization membership.

#### Properties

##### id

> **id**: `string`

Defined in: shared/dist/index.d.ts:462

##### user\_id

> **user\_id**: `string`

Defined in: shared/dist/index.d.ts:463

##### org\_id

> **org\_id**: `string`

Defined in: shared/dist/index.d.ts:464

##### role\_id

> **role\_id**: `string`

Defined in: shared/dist/index.d.ts:465

##### user

> **user**: [`User`](#user)

Defined in: shared/dist/index.d.ts:466

##### created\_at

> **created\_at**: `string`

Defined in: shared/dist/index.d.ts:467

##### updated\_at

> **updated\_at**: `string`

Defined in: shared/dist/index.d.ts:468

***

### Role

Defined in: shared/dist/index.d.ts:473

Organization role.

#### Properties

##### id

> **id**: `string`

Defined in: shared/dist/index.d.ts:474

##### name

> **name**: `string`

Defined in: shared/dist/index.d.ts:475

##### description?

> `optional` **description?**: `string`

Defined in: shared/dist/index.d.ts:476

##### permissions

> **permissions**: `string`[]

Defined in: shared/dist/index.d.ts:477

##### is\_default

> **is\_default**: `boolean`

Defined in: shared/dist/index.d.ts:478

***

### Invitation

Defined in: shared/dist/index.d.ts:483

Organization invitation.

#### Properties

##### id

> **id**: `string`

Defined in: shared/dist/index.d.ts:484

##### org\_id

> **org\_id**: `string`

Defined in: shared/dist/index.d.ts:485

##### email

> **email**: `string`

Defined in: shared/dist/index.d.ts:486

##### role\_id

> **role\_id**: `string`

Defined in: shared/dist/index.d.ts:487

##### status

> **status**: `"pending"` \| `"accepted"` \| `"expired"` \| `"revoked"`

Defined in: shared/dist/index.d.ts:488

##### expires\_at

> **expires\_at**: `string`

Defined in: shared/dist/index.d.ts:489

##### created\_at

> **created\_at**: `string`

Defined in: shared/dist/index.d.ts:490

***

### ApiKey

Defined in: shared/dist/index.d.ts:495

API key.

#### Properties

##### id

> **id**: `string`

Defined in: shared/dist/index.d.ts:496

##### name

> **name**: `string`

Defined in: shared/dist/index.d.ts:497

##### key\_prefix

> **key\_prefix**: `string`

Defined in: shared/dist/index.d.ts:498

##### scopes

> **scopes**: `string`[]

Defined in: shared/dist/index.d.ts:499

##### last\_used\_at?

> `optional` **last\_used\_at?**: `string`

Defined in: shared/dist/index.d.ts:500

##### expires\_at?

> `optional` **expires\_at?**: `string`

Defined in: shared/dist/index.d.ts:501

##### created\_at

> **created\_at**: `string`

Defined in: shared/dist/index.d.ts:502

## Type Aliases

### AuthStateChangeCallback

> **AuthStateChangeCallback** = (`event`, `session`) => `void`

Defined in: auth/src/types.ts:138

Auth state change callback.

#### Parameters

##### event

[`AuthStateEvent`](#authstateevent)

##### session

[`Session`](#session) \| `null`

#### Returns

`void`

***

### DetectedTokenType

> **DetectedTokenType** = `"verify"` \| `"password_reset"` \| `"oauth"` \| `"invite"`

Defined in: auth/src/url-detect.ts:4

Token types that can be detected from URLs.

***

### AuthStateEvent

> **AuthStateEvent** = `"INITIAL_SESSION"` \| `"SIGNED_IN"` \| `"SIGNED_OUT"` \| `"TOKEN_REFRESHED"` \| `"USER_UPDATED"`

Defined in: shared/dist/index.d.ts:523

Auth state change events.
