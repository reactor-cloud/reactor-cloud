[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / functions/src

# functions/src

## Functions

### createFunctionsClient()

> **createFunctionsClient**(`ctx`): [`FunctionsClient`](#functionsclient)

Defined in: functions/src/index.ts:183

#### Parameters

##### ctx

`RequestContext`

#### Returns

[`FunctionsClient`](#functionsclient)

## Classes

### FunctionsClient

Defined in: functions/src/index.ts:36

#### Accessors

##### env

###### Get Signature

> **get** **env**(): `object`

Defined in: functions/src/index.ts:146

Admin: Environment variables

###### Returns

`object`

###### set

> **set**: (`name`, `vars`) => `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### Parameters

###### name

`string`

###### vars

`Record`\<`string`, `string`\>

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### list

> **list**: (`name`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`EnvVar`](#envvar)[]\>\>

###### Parameters

###### name

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`EnvVar`](#envvar)[]\>\>

###### unset

> **unset**: (`name`, `keys`) => `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### Parameters

###### name

`string`

###### keys

`string`[]

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### logs

###### Get Signature

> **get** **logs**(): `object`

Defined in: functions/src/index.ts:160

Admin: Logs

###### Returns

`object`

###### list

> **list**: (`name`, `options?`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`FunctionLog`](#functionlog)[]\>\>

###### Parameters

###### name

`string`

###### options?

###### since?

`string`

###### limit?

`number`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`FunctionLog`](#functionlog)[]\>\>

##### versions

###### Get Signature

> **get** **versions**(): `object`

Defined in: functions/src/index.ts:172

Admin: Versions

###### Returns

`object`

###### list

> **list**: (`name`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`FunctionVersion`](#functionversion)[]\>\>

###### Parameters

###### name

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`FunctionVersion`](#functionversion)[]\>\>

###### rollback

> **rollback**: (`name`, `version`) => `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### Parameters

###### name

`string`

###### version

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

#### Constructors

##### Constructor

> **new FunctionsClient**(`ctx`): [`FunctionsClient`](#functionsclient)

Defined in: functions/src/index.ts:37

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`FunctionsClient`](#functionsclient)

#### Methods

##### invoke()

> **invoke**\<`T`\>(`name`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<`T`\>\>

Defined in: functions/src/index.ts:42

Invoke a function and return JSON response.

###### Type Parameters

###### T

`T` = `unknown`

###### Parameters

###### name

`string`

###### options?

[`InvokeOptions`](#invokeoptions)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`T`\>\>

##### invokeStream()

> **invokeStream**(`name`, `options?`): `AsyncIterable`\<`string`\>

Defined in: functions/src/index.ts:58

Invoke a function and stream the response (SSE).

###### Parameters

###### name

`string`

###### options?

[`InvokeOptions`](#invokeoptions)

###### Returns

`AsyncIterable`\<`string`\>

##### invokeRaw()

> **invokeRaw**(`name`, `options?`): `Promise`\<`Response`\>

Defined in: functions/src/index.ts:103

Invoke a function and return raw Response.

###### Parameters

###### name

`string`

###### options?

[`InvokeOptions`](#invokeoptions)

###### Returns

`Promise`\<`Response`\>

##### deploy()

> **deploy**(`name`, `bundle`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `version`: `string`; \}\>\>

Defined in: functions/src/index.ts:130

Admin: Deploy a function

###### Parameters

###### name

`string`

###### bundle

`Blob` \| `ArrayBuffer`

###### options?

###### version?

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `version`: `string`; \}\>\>

## Interfaces

### InvokeOptions

Defined in: functions/src/index.ts:10

#### Properties

##### body?

> `optional` **body?**: `unknown`

Defined in: functions/src/index.ts:11

##### headers?

> `optional` **headers?**: `Record`\<`string`, `string`\>

Defined in: functions/src/index.ts:12

##### signal?

> `optional` **signal?**: `AbortSignal`

Defined in: functions/src/index.ts:13

***

### FunctionVersion

Defined in: functions/src/index.ts:16

#### Properties

##### version

> **version**: `string`

Defined in: functions/src/index.ts:17

##### created\_at

> **created\_at**: `string`

Defined in: functions/src/index.ts:18

##### size\_bytes

> **size\_bytes**: `number`

Defined in: functions/src/index.ts:19

##### active

> **active**: `boolean`

Defined in: functions/src/index.ts:20

***

### FunctionLog

Defined in: functions/src/index.ts:23

#### Properties

##### timestamp

> **timestamp**: `string`

Defined in: functions/src/index.ts:24

##### level

> **level**: `"debug"` \| `"info"` \| `"warn"` \| `"error"`

Defined in: functions/src/index.ts:25

##### message

> **message**: `string`

Defined in: functions/src/index.ts:26

***

### EnvVar

Defined in: functions/src/index.ts:29

#### Properties

##### name

> **name**: `string`

Defined in: functions/src/index.ts:30

##### value?

> `optional` **value?**: `string`

Defined in: functions/src/index.ts:31

##### created\_at

> **created\_at**: `string`

Defined in: functions/src/index.ts:32

##### updated\_at

> **updated\_at**: `string`

Defined in: functions/src/index.ts:33
