[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / client/src

# client/src

## Functions

### createClient()

> **createClient**\<`Schema`\>(`url`, `options?`): [`ReactorClient`](#reactorclient)\<`Schema`\>

Defined in: client/src/index.ts:103

Create a Reactor client.

#### Type Parameters

##### Schema

`Schema` *extends* [`GenericSchema`](#genericschema) = [`GenericSchema`](#genericschema)

#### Parameters

##### url

`string`

The Reactor API URL (e.g., 'https://reactor.cloud')

##### options?

[`ReactorClientOptions`](#reactorclientoptions) = `{}`

Client configuration options

#### Returns

[`ReactorClient`](#reactorclient)\<`Schema`\>

A configured Reactor client

#### Example

```ts
import { createClient } from '@reactor/client';

const reactor = createClient('https://reactor.cloud', {
  key: 'rk_pub_...',
});
```

## Interfaces

### AuthClientOptions

Defined in: auth/dist/index.d.ts:7

Configuration options for the auth client.

#### Properties

##### storage?

> `optional` **storage?**: [`StorageAdapter`](#storageadapter)

Defined in: auth/dist/index.d.ts:9

Storage adapter for session persistence

##### storageKey?

> `optional` **storageKey?**: `string`

Defined in: auth/dist/index.d.ts:11

Storage key for the session (default: 'reactor.session')

##### autoRefresh?

> `optional` **autoRefresh?**: `boolean`

Defined in: auth/dist/index.d.ts:13

Whether to automatically refresh tokens (default: true)

##### persistSession?

> `optional` **persistSession?**: `boolean`

Defined in: auth/dist/index.d.ts:15

Whether to persist sessions to storage (default: true)

##### detectSessionInUrl?

> `optional` **detectSessionInUrl?**: `boolean`

Defined in: auth/dist/index.d.ts:17

Whether to detect sessions from URL (default: true)

***

### SignUpParams

Defined in: auth/dist/index.d.ts:22

Sign up request parameters.

#### Properties

##### email

> **email**: `string`

Defined in: auth/dist/index.d.ts:23

##### password

> **password**: `string`

Defined in: auth/dist/index.d.ts:24

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: auth/dist/index.d.ts:25

***

### SignInParams

Defined in: auth/dist/index.d.ts:30

Sign in request parameters.

#### Properties

##### email

> **email**: `string`

Defined in: auth/dist/index.d.ts:31

##### password

> **password**: `string`

Defined in: auth/dist/index.d.ts:32

***

### UpdateUserParams

Defined in: auth/dist/index.d.ts:37

Update user request parameters.

#### Properties

##### email?

> `optional` **email?**: `string`

Defined in: auth/dist/index.d.ts:38

##### password?

> `optional` **password?**: `string`

Defined in: auth/dist/index.d.ts:39

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: auth/dist/index.d.ts:40

***

### ReactorClientOptions

Defined in: client/src/index.ts:50

Options for creating a Reactor client.

#### Properties

##### key?

> `optional` **key?**: `string`

Defined in: client/src/index.ts:52

Project key (anon key) - safe for browser bundles

##### org?

> `optional` **org?**: `string`

Defined in: client/src/index.ts:54

Default organization context

##### fetch?

> `optional` **fetch?**: \{(`input`, `init?`): `Promise`\<`Response`\>; (`input`, `init?`): `Promise`\<`Response`\>; \}

Defined in: client/src/index.ts:56

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

##### headers?

> `optional` **headers?**: `Record`\<`string`, `string`\>

Defined in: client/src/index.ts:58

Global headers for all requests

##### auth?

> `optional` **auth?**: [`AuthClientOptions`](#authclientoptions)

Defined in: client/src/index.ts:60

Auth-specific options

##### storage?

> `optional` **storage?**: [`StorageAdapter`](#storageadapter)

Defined in: client/src/index.ts:62

Custom storage adapter for session persistence

***

### ReactorClient

Defined in: client/src/index.ts:68

The unified Reactor client interface.

#### Type Parameters

##### Schema

`Schema` *extends* [`GenericSchema`](#genericschema) = [`GenericSchema`](#genericschema)

#### Properties

##### auth

> **auth**: `AuthClient`

Defined in: client/src/index.ts:70

Authentication client

##### from

> **from**: \<`TableName`\>(`table`) => `PostgrestQueryBuilder`\<`Schema`\[`"Tables"`\]\[`TableName`\]\[`"Row"`\]\>

Defined in: client/src/index.ts:72

Data query builder (PostgREST-style)

Start a query on a table.

###### Type Parameters

###### TableName

`TableName` *extends* `string`

###### Parameters

###### table

`TableName`

The table name

###### Returns

`PostgrestQueryBuilder`\<`Schema`\[`"Tables"`\]\[`TableName`\]\[`"Row"`\]\>

A query builder

##### rpc

> **rpc**: \<`FunctionName`, `Args`, `Returns`\>(`functionName`, `args?`) => `RpcBuilder`\<`Args`, `Returns`\>

Defined in: client/src/index.ts:74

RPC calls

Call a database function via RPC.

###### Type Parameters

###### FunctionName

`FunctionName` *extends* `string`

###### Args

`Args` *extends* `Record`\<`string`, `unknown`\>

###### Returns

`Returns` = `Schema`\[`"Functions"`\]\[`FunctionName`\]\[`"Returns"`\]

###### Parameters

###### functionName

`FunctionName`

The function name

###### args?

`Args`

Function arguments

###### Returns

`RpcBuilder`\<`Args`, `Returns`\>

RPC builder

##### storage

> **storage**: `StorageClient`

Defined in: client/src/index.ts:76

Storage client

##### functions

> **functions**: `FunctionsClient`

Defined in: client/src/index.ts:78

Functions client

##### jobs

> **jobs**: `JobsClient`

Defined in: client/src/index.ts:80

Jobs client

##### sites

> **sites**: `SitesClient`

Defined in: client/src/index.ts:82

Sites admin client

##### realtime

> **realtime**: `RealtimeClient`

Defined in: client/src/index.ts:84

Realtime client (stub)

***

### InvokeOptions

Defined in: functions/dist/index.d.ts:3

#### Properties

##### body?

> `optional` **body?**: `unknown`

Defined in: functions/dist/index.d.ts:4

##### headers?

> `optional` **headers?**: `Record`\<`string`, `string`\>

Defined in: functions/dist/index.d.ts:5

##### signal?

> `optional` **signal?**: `AbortSignal`

Defined in: functions/dist/index.d.ts:6

***

### FunctionVersion

Defined in: functions/dist/index.d.ts:8

#### Properties

##### version

> **version**: `string`

Defined in: functions/dist/index.d.ts:9

##### created\_at

> **created\_at**: `string`

Defined in: functions/dist/index.d.ts:10

##### size\_bytes

> **size\_bytes**: `number`

Defined in: functions/dist/index.d.ts:11

##### active

> **active**: `boolean`

Defined in: functions/dist/index.d.ts:12

***

### FunctionLog

Defined in: functions/dist/index.d.ts:14

#### Properties

##### timestamp

> **timestamp**: `string`

Defined in: functions/dist/index.d.ts:15

##### level

> **level**: `"debug"` \| `"info"` \| `"warn"` \| `"error"`

Defined in: functions/dist/index.d.ts:16

##### message

> **message**: `string`

Defined in: functions/dist/index.d.ts:17

***

### JobRun

Defined in: jobs/dist/index.d.ts:4

#### Properties

##### id

> **id**: `string`

Defined in: jobs/dist/index.d.ts:5

##### job\_name

> **job\_name**: `string`

Defined in: jobs/dist/index.d.ts:6

##### status

> **status**: [`RunStatus`](#runstatus)

Defined in: jobs/dist/index.d.ts:7

##### payload?

> `optional` **payload?**: `unknown`

Defined in: jobs/dist/index.d.ts:8

##### result?

> `optional` **result?**: `unknown`

Defined in: jobs/dist/index.d.ts:9

##### error?

> `optional` **error?**: `string`

Defined in: jobs/dist/index.d.ts:10

##### started\_at?

> `optional` **started\_at?**: `string`

Defined in: jobs/dist/index.d.ts:11

##### completed\_at?

> `optional` **completed\_at?**: `string`

Defined in: jobs/dist/index.d.ts:12

##### created\_at

> **created\_at**: `string`

Defined in: jobs/dist/index.d.ts:13

***

### TriggerOptions

Defined in: jobs/dist/index.d.ts:15

#### Properties

##### payload?

> `optional` **payload?**: `unknown`

Defined in: jobs/dist/index.d.ts:16

##### idempotencyKey?

> `optional` **idempotencyKey?**: `string`

Defined in: jobs/dist/index.d.ts:17

***

### WaitOptions

Defined in: jobs/dist/index.d.ts:25

#### Properties

##### timeoutMs?

> `optional` **timeoutMs?**: `number`

Defined in: jobs/dist/index.d.ts:26

##### pollIntervalMs?

> `optional` **pollIntervalMs?**: `number`

Defined in: jobs/dist/index.d.ts:27

***

### RealtimePayload

Defined in: realtime/dist/index.d.ts:10

#### Type Parameters

##### T

`T` = `unknown`

#### Properties

##### event

> **event**: [`RealtimeEvent`](#realtimeevent)

Defined in: realtime/dist/index.d.ts:11

##### schema

> **schema**: `string`

Defined in: realtime/dist/index.d.ts:12

##### table

> **table**: `string`

Defined in: realtime/dist/index.d.ts:13

##### commit\_timestamp

> **commit\_timestamp**: `string`

Defined in: realtime/dist/index.d.ts:14

##### old\_record?

> `optional` **old\_record?**: `T`

Defined in: realtime/dist/index.d.ts:15

##### new\_record?

> `optional` **new\_record?**: `T`

Defined in: realtime/dist/index.d.ts:16

***

### RealtimeChannel

Defined in: realtime/dist/index.d.ts:18

#### Methods

##### on()

> **on**(`event`, `callback`): [`RealtimeChannel`](#realtimechannel)

Defined in: realtime/dist/index.d.ts:19

###### Parameters

###### event

[`RealtimeEvent`](#realtimeevent)

###### callback

(`payload`) => `void`

###### Returns

[`RealtimeChannel`](#realtimechannel)

##### subscribe()

> **subscribe**(): `Promise`\<`void`\>

Defined in: realtime/dist/index.d.ts:20

###### Returns

`Promise`\<`void`\>

##### unsubscribe()

> **unsubscribe**(): `Promise`\<`void`\>

Defined in: realtime/dist/index.d.ts:21

###### Returns

`Promise`\<`void`\>

***

### ReactorError

Defined in: shared/dist/index.d.ts:4

Base error class for all Reactor SDK errors.

#### Extends

- `Error`

#### Extended by

- [`AuthError`](#autherror)
- [`ValidationError`](#validationerror)
- [`NotFoundError`](#notfounderror)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/dist/index.d.ts:17

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

Defined in: shared/dist/index.d.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

##### statusCode

> **statusCode**: `number`

Defined in: shared/dist/index.d.ts:8

HTTP status code

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/dist/index.d.ts:10

Optional hint for resolution

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/dist/index.d.ts:12

Original error cause

***

### AuthError

Defined in: shared/dist/index.d.ts:28

Authentication/authorization errors (401, 403).

#### Extends

- [`ReactorError`](#reactorerror)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/dist/index.d.ts:17

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

Defined in: shared/dist/index.d.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/dist/index.d.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/dist/index.d.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/dist/index.d.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### ValidationError

Defined in: shared/dist/index.d.ts:46

Validation errors (400, 422).

#### Extends

- [`ReactorError`](#reactorerror)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/dist/index.d.ts:17

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

Defined in: shared/dist/index.d.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/dist/index.d.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/dist/index.d.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/dist/index.d.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

##### fields?

> `optional` **fields?**: `Record`\<`string`, `string`[]\>

Defined in: shared/dist/index.d.ts:48

Field-level errors

***

### NotFoundError

Defined in: shared/dist/index.d.ts:58

Resource not found (404).

#### Extends

- [`ReactorError`](#reactorerror)

#### Methods

##### toJSON()

> **toJSON**(): `object`

Defined in: shared/dist/index.d.ts:17

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

Defined in: shared/dist/index.d.ts:6

Error code (e.g., 'invalid_credentials', 'not_found')

###### Inherited from

[`ReactorError`](#reactorerror).[`code`](#code)

##### statusCode

> **statusCode**: `number`

Defined in: shared/dist/index.d.ts:8

HTTP status code

###### Inherited from

[`ReactorError`](#reactorerror).[`statusCode`](#statuscode)

##### hint?

> `optional` **hint?**: `string`

Defined in: shared/dist/index.d.ts:10

Optional hint for resolution

###### Inherited from

[`ReactorError`](#reactorerror).[`hint`](#hint)

##### cause?

> `optional` **cause?**: `Error`

Defined in: shared/dist/index.d.ts:12

Original error cause

###### Inherited from

[`ReactorError`](#reactorerror).[`cause`](#cause)

***

### StorageAdapter

Defined in: shared/dist/index.d.ts:242

Storage adapter interface for session persistence.
Supports both sync and async implementations.

#### Methods

##### getItem()

> **getItem**(`key`): `string` \| `Promise`\<`string`\>

Defined in: shared/dist/index.d.ts:243

###### Parameters

###### key

`string`

###### Returns

`string` \| `Promise`\<`string`\>

##### setItem()

> **setItem**(`key`, `value`): `void` \| `Promise`\<`void`\>

Defined in: shared/dist/index.d.ts:244

###### Parameters

###### key

`string`

###### value

`string`

###### Returns

`void` \| `Promise`\<`void`\>

##### removeItem()

> **removeItem**(`key`): `void` \| `Promise`\<`void`\>

Defined in: shared/dist/index.d.ts:245

###### Parameters

###### key

`string`

###### Returns

`void` \| `Promise`\<`void`\>

***

### AuthStateSubscription

Defined in: shared/dist/index.d.ts:527

Subscription to auth state changes.

#### Properties

##### unsubscribe

> **unsubscribe**: () => `void`

Defined in: shared/dist/index.d.ts:528

###### Returns

`void`

***

### Deployment

Defined in: sites/dist/index.d.ts:3

#### Properties

##### id

> **id**: `string`

Defined in: sites/dist/index.d.ts:4

##### site\_name

> **site\_name**: `string`

Defined in: sites/dist/index.d.ts:5

##### version

> **version**: `string`

Defined in: sites/dist/index.d.ts:6

##### status

> **status**: `"pending"` \| `"failed"` \| `"building"` \| `"ready"`

Defined in: sites/dist/index.d.ts:7

##### url?

> `optional` **url?**: `string`

Defined in: sites/dist/index.d.ts:8

##### created\_at

> **created\_at**: `string`

Defined in: sites/dist/index.d.ts:9

##### completed\_at?

> `optional` **completed\_at?**: `string`

Defined in: sites/dist/index.d.ts:10

***

### Domain

Defined in: sites/dist/index.d.ts:12

#### Properties

##### id

> **id**: `string`

Defined in: sites/dist/index.d.ts:13

##### site\_name

> **site\_name**: `string`

Defined in: sites/dist/index.d.ts:14

##### domain

> **domain**: `string`

Defined in: sites/dist/index.d.ts:15

##### verified

> **verified**: `boolean`

Defined in: sites/dist/index.d.ts:16

##### created\_at

> **created\_at**: `string`

Defined in: sites/dist/index.d.ts:17

***

### Site

Defined in: sites/dist/index.d.ts:19

#### Properties

##### id

> **id**: `string`

Defined in: sites/dist/index.d.ts:20

##### name

> **name**: `string`

Defined in: sites/dist/index.d.ts:21

##### framework?

> `optional` **framework?**: `string`

Defined in: sites/dist/index.d.ts:22

##### created\_at

> **created\_at**: `string`

Defined in: sites/dist/index.d.ts:23

##### updated\_at

> **updated\_at**: `string`

Defined in: sites/dist/index.d.ts:24

***

### FileObject

Defined in: storage/dist/index.d.ts:3

#### Properties

##### name

> **name**: `string`

Defined in: storage/dist/index.d.ts:4

##### id

> **id**: `string`

Defined in: storage/dist/index.d.ts:5

##### bucket\_id

> **bucket\_id**: `string`

Defined in: storage/dist/index.d.ts:6

##### owner?

> `optional` **owner?**: `string`

Defined in: storage/dist/index.d.ts:7

##### created\_at

> **created\_at**: `string`

Defined in: storage/dist/index.d.ts:8

##### updated\_at

> **updated\_at**: `string`

Defined in: storage/dist/index.d.ts:9

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: storage/dist/index.d.ts:10

***

### Bucket

Defined in: storage/dist/index.d.ts:12

#### Properties

##### id

> **id**: `string`

Defined in: storage/dist/index.d.ts:13

##### name

> **name**: `string`

Defined in: storage/dist/index.d.ts:14

##### public

> **public**: `boolean`

Defined in: storage/dist/index.d.ts:15

##### created\_at

> **created\_at**: `string`

Defined in: storage/dist/index.d.ts:16

##### updated\_at

> **updated\_at**: `string`

Defined in: storage/dist/index.d.ts:17

***

### UploadOptions

Defined in: storage/dist/index.d.ts:19

#### Properties

##### contentType?

> `optional` **contentType?**: `string`

Defined in: storage/dist/index.d.ts:20

##### cacheControl?

> `optional` **cacheControl?**: `string`

Defined in: storage/dist/index.d.ts:21

##### upsert?

> `optional` **upsert?**: `boolean`

Defined in: storage/dist/index.d.ts:22

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: storage/dist/index.d.ts:23

***

### ListOptions

Defined in: storage/dist/index.d.ts:25

#### Properties

##### limit?

> `optional` **limit?**: `number`

Defined in: storage/dist/index.d.ts:26

##### offset?

> `optional` **offset?**: `number`

Defined in: storage/dist/index.d.ts:27

##### sortBy?

> `optional` **sortBy?**: `object`

Defined in: storage/dist/index.d.ts:28

###### column

> **column**: `string`

###### order

> **order**: `"asc"` \| `"desc"`

##### search?

> `optional` **search?**: `string`

Defined in: storage/dist/index.d.ts:32

## Type Aliases

### GenericSchema

> **GenericSchema** = `object`

Defined in: data/dist/index.d.ts:6

Generic schema for untyped queries.

#### Properties

##### Tables

> **Tables**: `Record`\<`string`, \{ `Row`: `Record`\<`string`, `unknown`\>; `Insert`: `Record`\<`string`, `unknown`\>; `Update`: `Record`\<`string`, `unknown`\>; \}\>

Defined in: data/dist/index.d.ts:7

##### Views

> **Views**: `Record`\<`string`, \{ `Row`: `Record`\<`string`, `unknown`\>; \}\>

Defined in: data/dist/index.d.ts:12

##### Functions

> **Functions**: `Record`\<`string`, \{ `Args`: `Record`\<`string`, `unknown`\>; `Returns`: `unknown`; \}\>

Defined in: data/dist/index.d.ts:15

***

### CountMode

> **CountMode** = `"exact"` \| `"planned"` \| `"estimated"`

Defined in: data/dist/index.d.ts:23

Count mode for queries.

***

### FilterOperator

> **FilterOperator** = `"eq"` \| `"neq"` \| `"gt"` \| `"gte"` \| `"lt"` \| `"lte"` \| `"like"` \| `"ilike"` \| `"in"` \| `"is"` \| `"cs"` \| `"cd"` \| `"ov"` \| `"fts"`

Defined in: data/dist/index.d.ts:31

Filter operators supported by reactor-data.

***

### RunStatus

> **RunStatus** = `"pending"` \| `"running"` \| `"succeeded"` \| `"failed"` \| `"cancelled"`

Defined in: jobs/dist/index.d.ts:3

***

### RealtimeEvent

> **RealtimeEvent** = `"INSERT"` \| `"UPDATE"` \| `"DELETE"` \| `"*"`

Defined in: realtime/dist/index.d.ts:9

@reactor/realtime - Realtime subscriptions for Reactor

This is a stub package reserving the API surface for future implementation.
Realtime subscriptions will be implemented in a future version.

***

### Result

> **Result**\<`T`, `E`\> = \{ `data`: `T`; `error`: `null`; \} \| \{ `data`: `null`; `error`: `E`; \}

Defined in: shared/dist/index.d.ts:139

Result type for SDK operations.
Always contains either data or error, never both.

#### Type Parameters

##### T

`T`

##### E

`E` = [`ReactorError`](#reactorerror)

## References

### User

Re-exports [User](../auth/src.md#user)

***

### Session

Re-exports [Session](../auth/src.md#session)

***

### Organization

Re-exports [Organization](../auth/src.md#organization)

***

### Member

Re-exports [Member](../auth/src.md#member)

***

### Role

Re-exports [Role](../auth/src.md#role)

***

### Invitation

Re-exports [Invitation](../auth/src.md#invitation)

***

### ApiKey

Re-exports [ApiKey](../auth/src.md#apikey)

***

### AuthStateEvent

Re-exports [AuthStateEvent](../auth/src.md#authstateevent)
