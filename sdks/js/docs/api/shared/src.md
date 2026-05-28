[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / shared/src

# shared/src

## Functions

### isErrorEnvelope()

> **isErrorEnvelope**(`obj`): `obj is ErrorEnvelope`

Defined in: shared/src/errors.ts:176

Check if an object looks like a server error envelope.

#### Parameters

##### obj

`unknown`

#### Returns

`obj is ErrorEnvelope`

***

### errorFromResponse()

> **errorFromResponse**(`status`, `body`): [`ReactorError`](#reactorerror)

Defined in: shared/src/errors.ts:194

Create an appropriate error from an HTTP response.

#### Parameters

##### status

`number`

##### body

`unknown`

#### Returns

[`ReactorError`](#reactorerror)

***

### request()

> **request**\<`T`\>(`ctx`, `path`, `options?`): `Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

Defined in: shared/src/fetch.ts:83

Make an HTTP request with the SDK conventions.

#### Type Parameters

##### T

`T`

#### Parameters

##### ctx

[`RequestContext`](#requestcontext)

##### path

`string`

##### options?

[`RequestOptions`](#requestoptions) = `{}`

#### Returns

`Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

***

### get()

> **get**\<`T`\>(`ctx`, `path`, `options?`): `Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

Defined in: shared/src/fetch.ts:254

Helper for GET requests.

#### Type Parameters

##### T

`T`

#### Parameters

##### ctx

[`RequestContext`](#requestcontext)

##### path

`string`

##### options?

`Omit`\<[`RequestOptions`](#requestoptions), `"method"` \| `"body"`\>

#### Returns

`Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

***

### post()

> **post**\<`T`\>(`ctx`, `path`, `body?`, `options?`): `Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

Defined in: shared/src/fetch.ts:265

Helper for POST requests.

#### Type Parameters

##### T

`T`

#### Parameters

##### ctx

[`RequestContext`](#requestcontext)

##### path

`string`

##### body?

`unknown`

##### options?

`Omit`\<[`RequestOptions`](#requestoptions), `"method"` \| `"body"`\>

#### Returns

`Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

***

### put()

> **put**\<`T`\>(`ctx`, `path`, `body?`, `options?`): `Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

Defined in: shared/src/fetch.ts:277

Helper for PUT requests.

#### Type Parameters

##### T

`T`

#### Parameters

##### ctx

[`RequestContext`](#requestcontext)

##### path

`string`

##### body?

`unknown`

##### options?

`Omit`\<[`RequestOptions`](#requestoptions), `"method"` \| `"body"`\>

#### Returns

`Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

***

### patch()

> **patch**\<`T`\>(`ctx`, `path`, `body?`, `options?`): `Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

Defined in: shared/src/fetch.ts:289

Helper for PATCH requests.

#### Type Parameters

##### T

`T`

#### Parameters

##### ctx

[`RequestContext`](#requestcontext)

##### path

`string`

##### body?

`unknown`

##### options?

`Omit`\<[`RequestOptions`](#requestoptions), `"method"` \| `"body"`\>

#### Returns

`Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

***

### del()

> **del**\<`T`\>(`ctx`, `path`, `options?`): `Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

Defined in: shared/src/fetch.ts:301

Helper for DELETE requests.

#### Type Parameters

##### T

`T`

#### Parameters

##### ctx

[`RequestContext`](#requestcontext)

##### path

`string`

##### options?

`Omit`\<[`RequestOptions`](#requestoptions), `"method"`\>

#### Returns

`Promise`\<[`Result`](#result)\<`T`, [`ReactorError`](#reactorerror)\>\>

***

### decodeJwt()

> **decodeJwt**(`token`): [`JWTPayload`](#jwtpayload)

Defined in: shared/src/jwt.ts:39

Decode a JWT without verifying the signature.
For client-side use only - server should always verify.

#### Parameters

##### token

`string`

JWT string

#### Returns

[`JWTPayload`](#jwtpayload)

Decoded payload or null if invalid

***

### isJwtExpired()

> **isJwtExpired**(`token`, `bufferSeconds?`): `boolean`

Defined in: shared/src/jwt.ts:79

Check if a JWT is expired.

#### Parameters

##### token

`string` \| [`JWTPayload`](#jwtpayload)

JWT string or decoded payload

##### bufferSeconds?

`number` = `0`

Seconds before actual expiry to consider it expired (default: 0)

#### Returns

`boolean`

True if expired or will expire within buffer

***

### getJwtExpiry()

> **getJwtExpiry**(`token`): `Date`

Defined in: shared/src/jwt.ts:95

Get the expiration date of a JWT.

#### Parameters

##### token

`string` \| [`JWTPayload`](#jwtpayload)

JWT string or decoded payload

#### Returns

`Date`

Date object or null if invalid

***

### getJwtTimeRemaining()

> **getJwtTimeRemaining**(`token`): `number`

Defined in: shared/src/jwt.ts:110

Get seconds until JWT expiration.

#### Parameters

##### token

`string` \| [`JWTPayload`](#jwtpayload)

JWT string or decoded payload

#### Returns

`number`

Seconds remaining (negative if expired) or null if invalid

***

### encodeFilterValue()

> **encodeFilterValue**(`value`): `string`

Defined in: shared/src/query.ts:37

Encode a value for use in a filter expression.

#### Parameters

##### value

[`FilterValue`](#filtervalue)

#### Returns

`string`

***

### buildFilterExpression()

> **buildFilterExpression**(`op`, `value`, `negated?`): `string`

Defined in: shared/src/query.ts:66

Build a filter expression in PostgREST format.

#### Parameters

##### op

[`FilterOperator`](#filteroperator)

##### value

[`FilterValue`](#filtervalue)

##### negated?

`boolean` = `false`

#### Returns

`string`

***

### buildOrderExpression()

> **buildOrderExpression**(`column`, `options?`): `string`

Defined in: shared/src/query.ts:89

Build an order expression.

#### Parameters

##### column

`string`

##### options?

###### ascending?

`boolean`

###### nullsFirst?

`boolean`

#### Returns

`string`

***

### queryParamsToSearchParams()

> **queryParamsToSearchParams**(`params`): `URLSearchParams`

Defined in: shared/src/query.ts:124

Convert QueryParams to URLSearchParams.

#### Parameters

##### params

[`QueryParams`](#queryparams)

#### Returns

`URLSearchParams`

***

### buildUrl()

> **buildUrl**(`baseUrl`, `path`, `params?`): `string`

Defined in: shared/src/query.ts:153

Build a full URL with query parameters.

#### Parameters

##### baseUrl

`string`

##### path

`string`

##### params?

[`QueryParams`](#queryparams)

#### Returns

`string`

***

### parseContentRange()

> **parseContentRange**(`header`): `object`

Defined in: shared/src/query.ts:170

Parse the Content-Range header for count information.
Format: "0-24/1234" or star/1234

#### Parameters

##### header

`string`

#### Returns

`object`

##### from?

> `optional` **from?**: `number`

##### to?

> `optional` **to?**: `number`

##### total?

> `optional` **total?**: `number`

***

### parseSelectColumns()

> **parseSelectColumns**(`select`): `string`[]

Defined in: shared/src/query.ts:193

Parse a select string and extract column names.
Handles embedded relations like "author:users(name)".

#### Parameters

##### select

`string`

#### Returns

`string`[]

***

### encodePathSegment()

> **encodePathSegment**(`value`): `string`

Defined in: shared/src/query.ts:223

Encode a value for embedding in a URL path segment.

#### Parameters

##### value

`string`

#### Returns

`string`

***

### ok()

> **ok**\<`T`\>(`data`): [`Result`](#result)\<`T`\>

Defined in: shared/src/result.ts:14

Create a successful result.

#### Type Parameters

##### T

`T`

#### Parameters

##### data

`T`

#### Returns

[`Result`](#result)\<`T`\>

***

### err()

> **err**\<`E`\>(`error`): [`Result`](#result)\<`never`, `E`\>

Defined in: shared/src/result.ts:21

Create an error result.

#### Type Parameters

##### E

`E` *extends* [`ReactorError`](#reactorerror)

#### Parameters

##### error

`E`

#### Returns

[`Result`](#result)\<`never`, `E`\>

***

### withThrowOnError()

> **withThrowOnError**\<`T`, `E`\>(`promise`): [`ResultPromise`](#resultpromise)\<`T`, `E`\>

Defined in: shared/src/result.ts:44

Wrap a promise returning a Result with the throwOnError mixin.

#### Type Parameters

##### T

`T`

##### E

`E` *extends* [`ReactorError`](#reactorerror)

#### Parameters

##### promise

`Promise`\<[`Result`](#result)\<`T`, `E`\>\>

#### Returns

[`ResultPromise`](#resultpromise)\<`T`, `E`\>

***

### createResultPromise()

> **createResultPromise**\<`T`\>(`operation`, `errorHandler?`): [`ResultPromise`](#resultpromise)\<`T`\>

Defined in: shared/src/result.ts:63

Helper to create a ResultPromise from an async operation.

#### Type Parameters

##### T

`T`

#### Parameters

##### operation

() => `Promise`\<`T`\>

##### errorHandler?

(`e`) => [`ReactorError`](#reactorerror)

#### Returns

[`ResultPromise`](#resultpromise)\<`T`\>

***

### memoryAdapter()

> **memoryAdapter**(): [`StorageAdapter`](#storageadapter)

Defined in: shared/src/storage-adapter.ts:15

In-memory storage adapter.
Useful for server-side rendering, testing, or when no persistent storage is available.

#### Returns

[`StorageAdapter`](#storageadapter)

***

### localStorageAdapter()

> **localStorageAdapter**(): [`StorageAdapter`](#storageadapter)

Defined in: shared/src/storage-adapter.ts:35

localStorage adapter for browsers.
Falls back gracefully if localStorage is unavailable.

#### Returns

[`StorageAdapter`](#storageadapter)

***

### sessionStorageAdapter()

> **sessionStorageAdapter**(): [`StorageAdapter`](#storageadapter)

Defined in: shared/src/storage-adapter.ts:86

sessionStorage adapter for browsers.
Data persists only for the session.

#### Returns

[`StorageAdapter`](#storageadapter)

***

### cookieAdapter()

> **cookieAdapter**(`options?`): [`StorageAdapter`](#storageadapter)

Defined in: shared/src/storage-adapter.ts:137

Cookie-based storage adapter.
Useful for SSR scenarios where cookies are accessible server-side.

#### Parameters

##### options?

###### path?

`string`

Cookie path (default: '/')

###### domain?

`string`

Cookie domain

###### secure?

`boolean`

Secure flag (default: true in production)

###### sameSite?

`"strict"` \| `"lax"` \| `"none"`

SameSite attribute (default: 'lax')

###### maxAge?

`number`

Max age in seconds

#### Returns

[`StorageAdapter`](#storageadapter)

***

### detectStorageAdapter()

> **detectStorageAdapter**(): [`StorageAdapter`](#storageadapter)

Defined in: shared/src/storage-adapter.ts:215

Detect and return the best available storage adapter.

#### Returns

[`StorageAdapter`](#storageadapter)

## Classes

### ReactorError

Defined in: shared/src/errors.ts:4

Base error class for all Reactor SDK errors.

#### Extends

- `Error`

#### Extended by

- [`AuthError`](#autherror)
- [`ForbiddenError`](#forbiddenerror)
- [`ValidationError`](#validationerror)
- [`NotFoundError`](#notfounderror)
- [`ConflictError`](#conflicterror)
- [`RateLimitError`](#ratelimiterror)
- [`ServerError`](#servererror)
- [`NetworkError`](#networkerror)
- [`AbortError`](#aborterror)
- [`TimeoutError`](#timeouterror)

#### Constructors

##### Constructor

> **new ReactorError**(`message`, `code`, `statusCode`, `options?`): [`ReactorError`](#reactorerror)

Defined in: shared/src/errors.ts:14

###### Parameters

###### message

`string`

###### code

`string`

###### statusCode

`number`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### Returns

[`ReactorError`](#reactorerror)

###### Overrides

`Error.constructor`

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

***

### AuthError

Defined in: shared/src/errors.ts:47

Authentication/authorization errors (401, 403).

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new AuthError**(`message`, `code`, `options?`): [`AuthError`](#autherror)

Defined in: shared/src/errors.ts:48

###### Parameters

###### message

`string`

###### code

`string`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### Returns

[`AuthError`](#autherror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### ForbiddenError

Defined in: shared/src/errors.ts:57

Forbidden error (403).

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new ForbiddenError**(`message`, `code`, `options?`): [`ForbiddenError`](#forbiddenerror)

Defined in: shared/src/errors.ts:58

###### Parameters

###### message

`string`

###### code

`string`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### Returns

[`ForbiddenError`](#forbiddenerror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### ValidationError

Defined in: shared/src/errors.ts:67

Validation errors (400, 422).

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new ValidationError**(`message`, `code`, `options?`): [`ValidationError`](#validationerror)

Defined in: shared/src/errors.ts:71

###### Parameters

###### message

`string`

###### code

`string`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### fields?

`Record`\<`string`, `string`[]\>

###### Returns

[`ValidationError`](#validationerror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

##### fields?

> `optional` **fields?**: `Record`\<`string`, `string`[]\>

Defined in: shared/src/errors.ts:69

Field-level errors

***

### NotFoundError

Defined in: shared/src/errors.ts:85

Resource not found (404).

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new NotFoundError**(`message`, `code?`, `options?`): [`NotFoundError`](#notfounderror)

Defined in: shared/src/errors.ts:86

###### Parameters

###### message

`string`

###### code?

`string` = `'not_found'`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### Returns

[`NotFoundError`](#notfounderror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### ConflictError

Defined in: shared/src/errors.ts:95

Conflict error (409).

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new ConflictError**(`message`, `code?`, `options?`): [`ConflictError`](#conflicterror)

Defined in: shared/src/errors.ts:96

###### Parameters

###### message

`string`

###### code?

`string` = `'conflict'`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### Returns

[`ConflictError`](#conflicterror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### RateLimitError

Defined in: shared/src/errors.ts:105

Rate limit exceeded (429).

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new RateLimitError**(`message`, `code?`, `options?`): [`RateLimitError`](#ratelimiterror)

Defined in: shared/src/errors.ts:109

###### Parameters

###### message

`string`

###### code?

`string` = `'rate_limited'`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### retryAfter?

`number`

###### Returns

[`RateLimitError`](#ratelimiterror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

##### retryAfter?

> `optional` **retryAfter?**: `number`

Defined in: shared/src/errors.ts:107

Seconds until retry is allowed

***

### ServerError

Defined in: shared/src/errors.ts:123

Server error (5xx).

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new ServerError**(`message`, `code?`, `options?`): [`ServerError`](#servererror)

Defined in: shared/src/errors.ts:124

###### Parameters

###### message

`string`

###### code?

`string` = `'server_error'`

###### options?

###### hint?

`string`

###### cause?

`Error`

###### Returns

[`ServerError`](#servererror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### NetworkError

Defined in: shared/src/errors.ts:133

Network/connection errors.

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new NetworkError**(`message`, `options?`): [`NetworkError`](#networkerror)

Defined in: shared/src/errors.ts:134

###### Parameters

###### message

`string`

###### options?

###### cause?

`Error`

###### Returns

[`NetworkError`](#networkerror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### AbortError

Defined in: shared/src/errors.ts:143

Request aborted by user.

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new AbortError**(`message?`): [`AbortError`](#aborterror)

Defined in: shared/src/errors.ts:144

###### Parameters

###### message?

`string` = `'Request was aborted'`

###### Returns

[`AbortError`](#aborterror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### TimeoutError

Defined in: shared/src/errors.ts:153

Request timeout.

#### Extends

- [`ReactorError`](#reactorerror)

#### Constructors

##### Constructor

> **new TimeoutError**(`message?`): [`TimeoutError`](#timeouterror)

Defined in: shared/src/errors.ts:154

###### Parameters

###### message?

`string` = `'Request timed out'`

###### Returns

[`TimeoutError`](#timeouterror)

###### Overrides

[`ReactorError`](#reactorerror).[`constructor`](#constructor)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/src/errors.ts:33

###### Returns

`object`

###### name

> **name**: `string`

###### message

> **message**: `string`

###### code

> **code**: `string`

###### statusCode

> **statusCode**: `number`

###### hint

> **hint**: `string`

###### Inherited from

[`ReactorError`](#reactorerror).[`toJSON`](#tojson)

#### Properties

##### code

> **code**: `string`

Defined in: shared/src/errors.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/src/errors.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/src/errors.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/src/errors.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

## Interfaces

### ErrorEnvelope

Defined in: shared/src/errors.ts:163

Server error response envelope.

#### Properties

##### error

> **error**: `object`

Defined in: shared/src/errors.ts:164

###### code

> **code**: `string`

###### message

> **message**: `string`

###### status?

> `optional` **status?**: `number`

###### hint?

> `optional` **hint?**: `string`

###### fields?

> `optional` **fields?**: `Record`\<`string`, `string`[]\>

***

### RequestOptions

Defined in: shared/src/fetch.ts:20

Request options for the fetch wrapper.

#### Properties

##### method?

> `optional` **method?**: `"DELETE"` \| `"GET"` \| `"POST"` \| `"PUT"` \| `"PATCH"`

Defined in: shared/src/fetch.ts:22

HTTP method

##### body?

> `optional` **body?**: `unknown`

Defined in: shared/src/fetch.ts:24

Request body (will be JSON-stringified)

##### headers?

> `optional` **headers?**: `Record`\<`string`, `string`\>

Defined in: shared/src/fetch.ts:26

Additional headers

##### signal?

> `optional` **signal?**: `AbortSignal`

Defined in: shared/src/fetch.ts:28

AbortSignal for cancellation

##### timeout?

> `optional` **timeout?**: `number`

Defined in: shared/src/fetch.ts:30

Timeout in milliseconds (default: 30000)

##### retries?

> `optional` **retries?**: `number`

Defined in: shared/src/fetch.ts:32

Number of retries on 5xx/network errors (default: 3)

##### responseType?

> `optional` **responseType?**: `"json"` \| `"text"` \| `"blob"` \| `"stream"`

Defined in: shared/src/fetch.ts:34

Expected response type

***

### RequestContext

Defined in: shared/src/fetch.ts:40

Request context shared across all SDK operations.

#### Properties

##### baseUrl

> **baseUrl**: `string`

Defined in: shared/src/fetch.ts:42

Base URL for API requests

##### projectKey?

> `optional` **projectKey?**: `string`

Defined in: shared/src/fetch.ts:44

Project key (anon key)

##### getAccessToken?

> `optional` **getAccessToken?**: () => `string` \| `Promise`\<`string`\>

Defined in: shared/src/fetch.ts:46

Current access token (JWT)

###### Returns

`string` \| `Promise`\<`string`\>

##### fetch?

> `optional` **fetch?**: \{(`input`, `init?`): `Promise`\<`Response`\>; (`input`, `init?`): `Promise`\<`Response`\>; \}

Defined in: shared/src/fetch.ts:48

Custom fetch implementation

###### Call Signature

> (`input`, `init?`): `Promise`\<`Response`\>

[MDN Reference](https://developer.mozilla.org/docs/Web/API/Window/fetch)

###### Parameters

###### input

`RequestInfo` \| `URL`

###### init?

`RequestInit`

###### Returns

`Promise`\<`Response`\>

###### Call Signature

> (`input`, `init?`): `Promise`\<`Response`\>

[MDN Reference](https://developer.mozilla.org/docs/Web/API/Window/fetch)

###### Parameters

###### input

`string` \| `Request` \| `URL`

###### init?

`RequestInit`

###### Returns

`Promise`\<`Response`\>

##### defaultHeaders?

> `optional` **defaultHeaders?**: `Record`\<`string`, `string`\>

Defined in: shared/src/fetch.ts:50

Default headers to include in all requests

##### defaultTimeout?

> `optional` **defaultTimeout?**: `number`

Defined in: shared/src/fetch.ts:52

Default timeout in milliseconds

##### defaultRetries?

> `optional` **defaultRetries?**: `number`

Defined in: shared/src/fetch.ts:54

Default number of retries

***

### JWTPayload

Defined in: shared/src/jwt.ts:4

Decoded JWT payload structure for Reactor tokens.

#### Indexable

> \[`key`: `string`\]: `unknown`

Any additional claims

#### Properties

##### sub

> **sub**: `string`

Defined in: shared/src/jwt.ts:6

Subject (user ID)

##### email?

> `optional` **email?**: `string`

Defined in: shared/src/jwt.ts:8

Email address

##### email\_verified?

> `optional` **email\_verified?**: `boolean`

Defined in: shared/src/jwt.ts:10

Whether email is verified

##### exp

> **exp**: `number`

Defined in: shared/src/jwt.ts:12

Expiration timestamp (seconds since epoch)

##### iat

> **iat**: `number`

Defined in: shared/src/jwt.ts:14

Issued at timestamp (seconds since epoch)

##### iss?

> `optional` **iss?**: `string`

Defined in: shared/src/jwt.ts:16

Issuer

##### aud?

> `optional` **aud?**: `string` \| `string`[]

Defined in: shared/src/jwt.ts:18

Audience

##### orgs?

> `optional` **orgs?**: `object`[]

Defined in: shared/src/jwt.ts:20

Organization memberships

###### id

> **id**: `string`

###### slug

> **slug**: `string`

###### role\_id

> **role\_id**: `string`

###### permissions

> **permissions**: `string`[]

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: shared/src/jwt.ts:27

User metadata

***

### QueryParams

Defined in: shared/src/query.ts:112

Parameters collected by the query builder.

#### Properties

##### select?

> `optional` **select?**: `string`

Defined in: shared/src/query.ts:113

##### filters

> **filters**: `object`[]

Defined in: shared/src/query.ts:114

###### column

> **column**: `string`

###### expression

> **expression**: `string`

##### order?

> `optional` **order?**: `string`[]

Defined in: shared/src/query.ts:115

##### limit?

> `optional` **limit?**: `number`

Defined in: shared/src/query.ts:116

##### offset?

> `optional` **offset?**: `number`

Defined in: shared/src/query.ts:117

##### count?

> `optional` **count?**: `"exact"` \| `"planned"` \| `"estimated"`

Defined in: shared/src/query.ts:118

***

### ThrowOnError

Defined in: shared/src/result.ts:28

Mixin for adding throwOnError to promises.

#### Type Parameters

##### T

`T`

#### Methods

##### throwOnError()

> **throwOnError**(): `Promise`\<`T`\>

Defined in: shared/src/result.ts:33

Throws if the result contains an error, otherwise returns the data.
Useful for `await reactor.from('posts').select('*').throwOnError()` pattern.

###### Returns

`Promise`\<`T`\>

***

### StorageAdapter

Defined in: shared/src/storage-adapter.ts:5

Storage adapter interface for session persistence.
Supports both sync and async implementations.

#### Methods

##### getItem()

> **getItem**(`key`): `string` \| `Promise`\<`string`\>

Defined in: shared/src/storage-adapter.ts:6

###### Parameters

###### key

`string`

###### Returns

`string` \| `Promise`\<`string`\>

##### setItem()

> **setItem**(`key`, `value`): `void` \| `Promise`\<`void`\>

Defined in: shared/src/storage-adapter.ts:7

###### Parameters

###### key

`string`

###### value

`string`

###### Returns

`void` \| `Promise`\<`void`\>

##### removeItem()

> **removeItem**(`key`): `void` \| `Promise`\<`void`\>

Defined in: shared/src/storage-adapter.ts:8

###### Parameters

###### key

`string`

###### Returns

`void` \| `Promise`\<`void`\>

***

### User

Defined in: shared/src/types.ts:8

User object returned from auth endpoints.

#### Properties

##### id

> **id**: `string`

Defined in: shared/src/types.ts:9

##### email

> **email**: `string`

Defined in: shared/src/types.ts:10

##### email\_verified

> **email\_verified**: `boolean`

Defined in: shared/src/types.ts:11

##### metadata

> **metadata**: `Record`\<`string`, `unknown`\>

Defined in: shared/src/types.ts:12

##### created\_at

> **created\_at**: `string`

Defined in: shared/src/types.ts:13

***

### Session

Defined in: shared/src/types.ts:19

Session object containing tokens.

#### Properties

##### access\_token

> **access\_token**: `string`

Defined in: shared/src/types.ts:20

##### refresh\_token

> **refresh\_token**: `string`

Defined in: shared/src/types.ts:21

##### expires\_at

> **expires\_at**: `string`

Defined in: shared/src/types.ts:22

##### user

> **user**: [`User`](#user)

Defined in: shared/src/types.ts:23

***

### Organization

Defined in: shared/src/types.ts:29

Organization object.

#### Properties

##### id

> **id**: `string`

Defined in: shared/src/types.ts:30

##### slug

> **slug**: `string`

Defined in: shared/src/types.ts:31

##### name

> **name**: `string`

Defined in: shared/src/types.ts:32

##### metadata

> **metadata**: `Record`\<`string`, `unknown`\>

Defined in: shared/src/types.ts:33

##### created\_at

> **created\_at**: `string`

Defined in: shared/src/types.ts:34

##### updated\_at

> **updated\_at**: `string`

Defined in: shared/src/types.ts:35

***

### Member

Defined in: shared/src/types.ts:41

Organization membership.

#### Properties

##### id

> **id**: `string`

Defined in: shared/src/types.ts:42

##### user\_id

> **user\_id**: `string`

Defined in: shared/src/types.ts:43

##### org\_id

> **org\_id**: `string`

Defined in: shared/src/types.ts:44

##### role\_id

> **role\_id**: `string`

Defined in: shared/src/types.ts:45

##### user

> **user**: [`User`](#user)

Defined in: shared/src/types.ts:46

##### created\_at

> **created\_at**: `string`

Defined in: shared/src/types.ts:47

##### updated\_at

> **updated\_at**: `string`

Defined in: shared/src/types.ts:48

***

### Role

Defined in: shared/src/types.ts:54

Organization role.

#### Properties

##### id

> **id**: `string`

Defined in: shared/src/types.ts:55

##### name

> **name**: `string`

Defined in: shared/src/types.ts:56

##### description?

> `optional` **description?**: `string`

Defined in: shared/src/types.ts:57

##### permissions

> **permissions**: `string`[]

Defined in: shared/src/types.ts:58

##### is\_default

> **is\_default**: `boolean`

Defined in: shared/src/types.ts:59

***

### Invitation

Defined in: shared/src/types.ts:65

Organization invitation.

#### Properties

##### id

> **id**: `string`

Defined in: shared/src/types.ts:66

##### org\_id

> **org\_id**: `string`

Defined in: shared/src/types.ts:67

##### email

> **email**: `string`

Defined in: shared/src/types.ts:68

##### role\_id

> **role\_id**: `string`

Defined in: shared/src/types.ts:69

##### status

> **status**: `"pending"` \| `"accepted"` \| `"expired"` \| `"revoked"`

Defined in: shared/src/types.ts:70

##### expires\_at

> **expires\_at**: `string`

Defined in: shared/src/types.ts:71

##### created\_at

> **created\_at**: `string`

Defined in: shared/src/types.ts:72

***

### ApiKey

Defined in: shared/src/types.ts:78

API key.

#### Properties

##### id

> **id**: `string`

Defined in: shared/src/types.ts:79

##### name

> **name**: `string`

Defined in: shared/src/types.ts:80

##### key\_prefix

> **key\_prefix**: `string`

Defined in: shared/src/types.ts:81

##### scopes

> **scopes**: `string`[]

Defined in: shared/src/types.ts:82

##### last\_used\_at?

> `optional` **last\_used\_at?**: `string`

Defined in: shared/src/types.ts:83

##### expires\_at?

> `optional` **expires\_at?**: `string`

Defined in: shared/src/types.ts:84

##### created\_at

> **created\_at**: `string`

Defined in: shared/src/types.ts:85

***

### PaginationParams

Defined in: shared/src/types.ts:91

Generic pagination parameters.

#### Properties

##### limit?

> `optional` **limit?**: `number`

Defined in: shared/src/types.ts:92

##### offset?

> `optional` **offset?**: `number`

Defined in: shared/src/types.ts:93

***

### PaginatedResponse

Defined in: shared/src/types.ts:99

Paginated response wrapper.

#### Type Parameters

##### T

`T`

#### Properties

##### data

> **data**: `T`[]

Defined in: shared/src/types.ts:100

##### total?

> `optional` **total?**: `number`

Defined in: shared/src/types.ts:101

##### limit

> **limit**: `number`

Defined in: shared/src/types.ts:102

##### offset

> **offset**: `number`

Defined in: shared/src/types.ts:103

***

### AuthStateSubscription

Defined in: shared/src/types.ts:119

Subscription to auth state changes.

#### Properties

##### unsubscribe

> **unsubscribe**: () => `void`

Defined in: shared/src/types.ts:120

###### Returns

`void`

***

### DatabaseSchema

Defined in: shared/src/types.ts:127

Database type structure for type-safe queries.
This is the shape generated by `reactor types generate`.

#### Properties

##### public

> **public**: `object`

Defined in: shared/src/types.ts:128

###### Tables

> **Tables**: `object`

###### Index Signature

\[`tableName`: `string`\]: `object`

###### Views?

> `optional` **Views?**: `object`

###### Index Signature

\[`viewName`: `string`\]: `object`

###### Functions?

> `optional` **Functions?**: `object`

###### Index Signature

\[`functionName`: `string`\]: `object`

###### Enums?

> `optional` **Enums?**: `object`

###### Index Signature

\[`enumName`: `string`\]: `string`

## Type Aliases

### FilterOperator

> **FilterOperator** = `"eq"` \| `"neq"` \| `"gt"` \| `"gte"` \| `"lt"` \| `"lte"` \| `"like"` \| `"ilike"` \| `"in"` \| `"is"` \| `"cs"` \| `"cd"` \| `"ov"` \| `"fts"`

Defined in: shared/src/query.ts:8

Filter operator types matching reactor-data dialect.

***

### FilterValue

> **FilterValue** = `string` \| `number` \| `boolean` \| `null` \| (`string` \| `number` \| `boolean`)[]

Defined in: shared/src/query.ts:27

Primitive value types for filters.

***

### OrderDirection

> **OrderDirection** = `"asc"` \| `"desc"`

Defined in: shared/src/query.ts:79

Order direction.

***

### OrderNulls

> **OrderNulls** = `"nullsfirst"` \| `"nullslast"`

Defined in: shared/src/query.ts:84

Order nulls position.

***

### Result

> **Result**\<`T`, `E`\> = \{ `data`: `T`; `error`: `null`; \} \| \{ `data`: `null`; `error`: `E`; \}

Defined in: shared/src/result.ts:7

Result type for SDK operations.
Always contains either data or error, never both.

#### Type Parameters

##### T

`T`

##### E

`E` = [`ReactorError`](#reactorerror)

***

### ResultPromise

> **ResultPromise**\<`T`, `E`\> = `Promise`\<[`Result`](#result)\<`T`, `E`\>\> & [`ThrowOnError`](#throwonerror)\<`T`\>

Defined in: shared/src/result.ts:39

A promise that resolves to a Result, with a throwOnError method.

#### Type Parameters

##### T

`T`

##### E

`E` = [`ReactorError`](#reactorerror)

***

### AuthStateEvent

> **AuthStateEvent** = `"INITIAL_SESSION"` \| `"SIGNED_IN"` \| `"SIGNED_OUT"` \| `"TOKEN_REFRESHED"` \| `"USER_UPDATED"`

Defined in: shared/src/types.ts:109

Auth state change events.

***

### GenericDatabase

> **GenericDatabase** = [`DatabaseSchema`](#databaseschema)

Defined in: shared/src/types.ts:162

Generic database schema for when types are not generated.

***

### TableRow

> **TableRow**\<`DB`, `T`\> = `DB`\[`"public"`\]\[`"Tables"`\]\[`T`\]\[`"Row"`\]

Defined in: shared/src/types.ts:167

Helper to extract table row type.

#### Type Parameters

##### DB

`DB` *extends* [`DatabaseSchema`](#databaseschema)

##### T

`T` *extends* keyof `DB`\[`"public"`\]\[`"Tables"`\]

***

### TableInsert

> **TableInsert**\<`DB`, `T`\> = `DB`\[`"public"`\]\[`"Tables"`\]\[`T`\]\[`"Insert"`\]

Defined in: shared/src/types.ts:175

Helper to extract table insert type.

#### Type Parameters

##### DB

`DB` *extends* [`DatabaseSchema`](#databaseschema)

##### T

`T` *extends* keyof `DB`\[`"public"`\]\[`"Tables"`\]

***

### TableUpdate

> **TableUpdate**\<`DB`, `T`\> = `DB`\[`"public"`\]\[`"Tables"`\]\[`T`\]\[`"Update"`\]

Defined in: shared/src/types.ts:183

Helper to extract table update type.

#### Type Parameters

##### DB

`DB` *extends* [`DatabaseSchema`](#databaseschema)

##### T

`T` *extends* keyof `DB`\[`"public"`\]\[`"Tables"`\]

## Variables

### SDK\_VERSION

> `const` **SDK\_VERSION**: `"0.1.0"` = `'0.1.0'`

Defined in: shared/src/fetch.ts:15

SDK version - injected at build time.
