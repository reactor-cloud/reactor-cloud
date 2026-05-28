[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / sites/src

# sites/src

## Functions

### createSitesClient()

> **createSitesClient**(`ctx`): [`SitesClient`](#sitesclient)

Defined in: sites/src/index.ts:107

#### Parameters

##### ctx

`RequestContext`

#### Returns

[`SitesClient`](#sitesclient)

## Classes

### SitesClient

Defined in: sites/src/index.ts:36

#### Accessors

##### domains

###### Get Signature

> **get** **domains**(): `object`

Defined in: sites/src/index.ts:59

Domain management

###### Returns

`object`

###### add

> **add**: (`siteName`, `domain`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`Domain`](#domain)\>\>

###### Parameters

###### siteName

`string`

###### domain

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Domain`](#domain)\>\>

###### remove

> **remove**: (`siteName`, `domain`) => `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### Parameters

###### siteName

`string`

###### domain

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

###### list

> **list**: (`siteName`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`Domain`](#domain)[]\>\>

###### Parameters

###### siteName

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Domain`](#domain)[]\>\>

##### deployments

###### Get Signature

> **get** **deployments**(): `object`

Defined in: sites/src/index.ts:73

Deployment management

###### Returns

`object`

###### list

> **list**: (`siteName`, `options?`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)[]\>\>

###### Parameters

###### siteName

`string`

###### options?

###### limit?

`number`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)[]\>\>

###### get

> **get**: (`siteName`, `deploymentId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

###### Parameters

###### siteName

`string`

###### deploymentId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

###### rollback

> **rollback**: (`siteName`, `deploymentId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

###### Parameters

###### siteName

`string`

###### deploymentId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

###### promote

> **promote**: (`siteName`, `deploymentId`) => `Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

###### Parameters

###### siteName

`string`

###### deploymentId

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

#### Constructors

##### Constructor

> **new SitesClient**(`ctx`): [`SitesClient`](#sitesclient)

Defined in: sites/src/index.ts:37

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`SitesClient`](#sitesclient)

#### Methods

##### deploy()

> **deploy**(`siteName`, `bundle`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

Defined in: sites/src/index.ts:42

Deploy a site bundle.

###### Parameters

###### siteName

`string`

###### bundle

`Blob` \| `ArrayBuffer`

###### options?

###### framework?

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Deployment`](#deployment)\>\>

##### list()

> **list**(): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Site`](#site)[]\>\>

Defined in: sites/src/index.ts:92

List all sites

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Site`](#site)[]\>\>

##### get()

> **get**(`siteName`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Site`](#site)\>\>

Defined in: sites/src/index.ts:97

Get site info

###### Parameters

###### siteName

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Site`](#site)\>\>

##### delete()

> **delete**(`siteName`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: sites/src/index.ts:102

Delete a site

###### Parameters

###### siteName

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

## Interfaces

### Deployment

Defined in: sites/src/index.ts:10

#### Properties

##### id

> **id**: `string`

Defined in: sites/src/index.ts:11

##### site\_name

> **site\_name**: `string`

Defined in: sites/src/index.ts:12

##### version

> **version**: `string`

Defined in: sites/src/index.ts:13

##### status

> **status**: `"pending"` \| `"failed"` \| `"building"` \| `"ready"`

Defined in: sites/src/index.ts:14

##### url?

> `optional` **url?**: `string`

Defined in: sites/src/index.ts:15

##### created\_at

> **created\_at**: `string`

Defined in: sites/src/index.ts:16

##### completed\_at?

> `optional` **completed\_at?**: `string`

Defined in: sites/src/index.ts:17

***

### Domain

Defined in: sites/src/index.ts:20

#### Properties

##### id

> **id**: `string`

Defined in: sites/src/index.ts:21

##### site\_name

> **site\_name**: `string`

Defined in: sites/src/index.ts:22

##### domain

> **domain**: `string`

Defined in: sites/src/index.ts:23

##### verified

> **verified**: `boolean`

Defined in: sites/src/index.ts:24

##### created\_at

> **created\_at**: `string`

Defined in: sites/src/index.ts:25

***

### Site

Defined in: sites/src/index.ts:28

#### Properties

##### id

> **id**: `string`

Defined in: sites/src/index.ts:29

##### name

> **name**: `string`

Defined in: sites/src/index.ts:30

##### framework?

> `optional` **framework?**: `string`

Defined in: sites/src/index.ts:31

##### created\_at

> **created\_at**: `string`

Defined in: sites/src/index.ts:32

##### updated\_at

> **updated\_at**: `string`

Defined in: sites/src/index.ts:33
