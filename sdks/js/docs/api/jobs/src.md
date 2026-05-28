[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / jobs/src

# jobs/src

## Functions

### createJobsClient()

> **createJobsClient**(`ctx`): [`JobsClient`](#jobsclient)

Defined in: jobs/src/index.ts:163

#### Parameters

##### ctx

`RequestContext`

#### Returns

[`JobsClient`](#jobsclient)

## Classes

### JobsClient

Defined in: jobs/src/index.ts:58

#### Accessors

##### runs

###### Get Signature

> **get** **runs**(): `object`

Defined in: jobs/src/index.ts:79

Job runs management

###### Returns

`object`

###### get

> **get**: (`runId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`JobRun`](#jobrun)\>\>

###### Parameters

###### runId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`JobRun`](#jobrun)\>\>

###### list

> **list**: (`options?`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`JobRun`](#jobrun)[]\>\>

###### Parameters

###### options?

[`ListRunsOptions`](#listrunsoptions)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`JobRun`](#jobrun)[]\>\>

###### cancel

> **cancel**: (`runId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### Parameters

###### runId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### wait

> **wait**: (`runId`, `options?`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`JobRun`](#jobrun)\>\>

###### Parameters

###### runId

`string`

###### options?

[`WaitOptions`](#waitoptions)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`JobRun`](#jobrun)\>\>

##### dlq

###### Get Signature

> **get** **dlq**(): `object`

Defined in: jobs/src/index.ts:126

Dead letter queue management

###### Returns

`object`

###### list

> **list**: (`options?`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`DlqEntry`](#dlqentry)[]\>\>

###### Parameters

###### options?

###### limit?

`number`

###### offset?

`number`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`DlqEntry`](#dlqentry)[]\>\>

###### retry

> **retry**: (`entryId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `runId`: `string`; \}\>\>

###### Parameters

###### entryId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `runId`: `string`; \}\>\>

###### remove

> **remove**: (`entryId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### Parameters

###### entryId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### triggers

###### Get Signature

> **get** **triggers**(): `object`

Defined in: jobs/src/index.ts:144

Trigger management (admin)

###### Returns

`object`

###### create

> **create**: (`jobName`, `config`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`JobTrigger`](#jobtrigger)\>\>

###### Parameters

###### jobName

`string`

###### config

###### cron?

`string`

###### webhook?

`boolean`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`JobTrigger`](#jobtrigger)\>\>

###### list

> **list**: (`jobName?`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`JobTrigger`](#jobtrigger)[]\>\>

###### Parameters

###### jobName?

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`JobTrigger`](#jobtrigger)[]\>\>

###### delete

> **delete**: (`triggerId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### Parameters

###### triggerId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

#### Constructors

##### Constructor

> **new JobsClient**(`ctx`): [`JobsClient`](#jobsclient)

Defined in: jobs/src/index.ts:59

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`JobsClient`](#jobsclient)

#### Methods

##### trigger()

> **trigger**(`name`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `runId`: `string`; \}\>\>

Defined in: jobs/src/index.ts:64

Trigger a job run.

###### Parameters

###### name

`string`

###### options?

[`TriggerOptions`](#triggeroptions)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `runId`: `string`; \}\>\>

## Interfaces

### JobRun

Defined in: jobs/src/index.ts:12

#### Properties

##### id

> **id**: `string`

Defined in: jobs/src/index.ts:13

##### job\_name

> **job\_name**: `string`

Defined in: jobs/src/index.ts:14

##### status

> **status**: [`RunStatus`](#runstatus)

Defined in: jobs/src/index.ts:15

##### payload?

> `optional` **payload?**: `unknown`

Defined in: jobs/src/index.ts:16

##### result?

> `optional` **result?**: `unknown`

Defined in: jobs/src/index.ts:17

##### error?

> `optional` **error?**: `string`

Defined in: jobs/src/index.ts:18

##### started\_at?

> `optional` **started\_at?**: `string`

Defined in: jobs/src/index.ts:19

##### completed\_at?

> `optional` **completed\_at?**: `string`

Defined in: jobs/src/index.ts:20

##### created\_at

> **created\_at**: `string`

Defined in: jobs/src/index.ts:21

***

### TriggerOptions

Defined in: jobs/src/index.ts:24

#### Properties

##### payload?

> `optional` **payload?**: `unknown`

Defined in: jobs/src/index.ts:25

##### idempotencyKey?

> `optional` **idempotencyKey?**: `string`

Defined in: jobs/src/index.ts:26

***

### ListRunsOptions

Defined in: jobs/src/index.ts:29

#### Properties

##### jobName?

> `optional` **jobName?**: `string`

Defined in: jobs/src/index.ts:30

##### status?

> `optional` **status?**: [`RunStatus`](#runstatus)

Defined in: jobs/src/index.ts:31

##### limit?

> `optional` **limit?**: `number`

Defined in: jobs/src/index.ts:32

##### offset?

> `optional` **offset?**: `number`

Defined in: jobs/src/index.ts:33

***

### WaitOptions

Defined in: jobs/src/index.ts:36

#### Properties

##### timeoutMs?

> `optional` **timeoutMs?**: `number`

Defined in: jobs/src/index.ts:37

##### pollIntervalMs?

> `optional` **pollIntervalMs?**: `number`

Defined in: jobs/src/index.ts:38

***

### JobTrigger

Defined in: jobs/src/index.ts:41

#### Properties

##### id

> **id**: `string`

Defined in: jobs/src/index.ts:42

##### job\_name

> **job\_name**: `string`

Defined in: jobs/src/index.ts:43

##### cron?

> `optional` **cron?**: `string`

Defined in: jobs/src/index.ts:44

##### webhook?

> `optional` **webhook?**: `boolean`

Defined in: jobs/src/index.ts:45

##### created\_at

> **created\_at**: `string`

Defined in: jobs/src/index.ts:46

***

### DlqEntry

Defined in: jobs/src/index.ts:49

#### Properties

##### id

> **id**: `string`

Defined in: jobs/src/index.ts:50

##### job\_name

> **job\_name**: `string`

Defined in: jobs/src/index.ts:51

##### payload

> **payload**: `unknown`

Defined in: jobs/src/index.ts:52

##### error

> **error**: `string`

Defined in: jobs/src/index.ts:53

##### attempts

> **attempts**: `number`

Defined in: jobs/src/index.ts:54

##### created\_at

> **created\_at**: `string`

Defined in: jobs/src/index.ts:55

## Type Aliases

### RunStatus

> **RunStatus** = `"pending"` \| `"running"` \| `"succeeded"` \| `"failed"` \| `"cancelled"`

Defined in: jobs/src/index.ts:10
