[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / storage/src

# storage/src

## Functions

### createStorageClient()

> **createStorageClient**(`ctx`): [`StorageClient`](#storageclient)

Defined in: storage/src/index.ts:175

#### Parameters

##### ctx

`RequestContext`

#### Returns

[`StorageClient`](#storageclient)

## Classes

### StorageBucketClient

Defined in: storage/src/index.ts:47

#### Constructors

##### Constructor

> **new StorageBucketClient**(`ctx`, `bucketId`): [`StorageBucketClient`](#storagebucketclient)

Defined in: storage/src/index.ts:48

###### Parameters

###### ctx

`RequestContext`

###### bucketId

`string`

###### Returns

[`StorageBucketClient`](#storagebucketclient)

#### Methods

##### upload()

> **upload**(`path`, `file`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `path`: `string`; `id`: `string`; \}\>\>

Defined in: storage/src/index.ts:53

###### Parameters

###### path

`string`

###### file

`Blob` \| `ArrayBuffer` \| `File`

###### options?

[`UploadOptions`](#uploadoptions)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `path`: `string`; `id`: `string`; \}\>\>

##### download()

> **download**(`path`): `Promise`\<[`Result`](../client/src.md#result-1)\<`Blob`\>\>

Defined in: storage/src/index.ts:78

###### Parameters

###### path

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`Blob`\>\>

##### createSignedUrl()

> **createSignedUrl**(`path`, `expiresIn`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `signedUrl`: `string`; \}\>\>

Defined in: storage/src/index.ts:86

###### Parameters

###### path

`string`

###### expiresIn

`number`

###### options?

[`SignedUrlOptions`](#signedurloptions)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `signedUrl`: `string`; \}\>\>

##### createSignedUrls()

> **createSignedUrls**(`paths`, `expiresIn`): `Promise`\<[`Result`](../client/src.md#result-1)\<`object`[]\>\>

Defined in: storage/src/index.ts:98

###### Parameters

###### paths

`string`[]

###### expiresIn

`number`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`object`[]\>\>

##### getPublicUrl()

> **getPublicUrl**(`path`): `string`

Defined in: storage/src/index.ts:108

###### Parameters

###### path

`string`

###### Returns

`string`

##### list()

> **list**(`prefix?`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`FileObject`](#fileobject)[]\>\>

Defined in: storage/src/index.ts:112

###### Parameters

###### prefix?

`string`

###### options?

[`ListOptions`](#listoptions)

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`FileObject`](#fileobject)[]\>\>

##### remove()

> **remove**(`paths`): `Promise`\<[`Result`](../client/src.md#result-1)\<`object`[]\>\>

Defined in: storage/src/index.ts:125

###### Parameters

###### paths

`string`[]

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`object`[]\>\>

##### move()

> **move**(`from`, `to`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `message`: `string`; \}\>\>

Defined in: storage/src/index.ts:131

###### Parameters

###### from

`string`

###### to

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `message`: `string`; \}\>\>

##### copy()

> **copy**(`from`, `to`): `Promise`\<[`Result`](../client/src.md#result-1)\<\{ `path`: `string`; \}\>\>

Defined in: storage/src/index.ts:139

###### Parameters

###### from

`string`

###### to

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<\{ `path`: `string`; \}\>\>

***

### StorageClient

Defined in: storage/src/index.ts:148

#### Constructors

##### Constructor

> **new StorageClient**(`ctx`): [`StorageClient`](#storageclient)

Defined in: storage/src/index.ts:149

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`StorageClient`](#storageclient)

#### Methods

##### from()

> **from**(`bucketId`): [`StorageBucketClient`](#storagebucketclient)

Defined in: storage/src/index.ts:151

###### Parameters

###### bucketId

`string`

###### Returns

[`StorageBucketClient`](#storagebucketclient)

##### createBucket()

> **createBucket**(`id`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Bucket`](#bucket)\>\>

Defined in: storage/src/index.ts:155

###### Parameters

###### id

`string`

###### options?

###### public?

`boolean`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Bucket`](#bucket)\>\>

##### deleteBucket()

> **deleteBucket**(`id`): `Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

Defined in: storage/src/index.ts:162

###### Parameters

###### id

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`void`\>\>

##### listBuckets()

> **listBuckets**(): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Bucket`](#bucket)[]\>\>

Defined in: storage/src/index.ts:166

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Bucket`](#bucket)[]\>\>

##### getBucket()

> **getBucket**(`id`): `Promise`\<[`Result`](../client/src.md#result-1)\<[`Bucket`](#bucket)\>\>

Defined in: storage/src/index.ts:170

###### Parameters

###### id

`string`

###### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<[`Bucket`](#bucket)\>\>

## Interfaces

### FileObject

Defined in: storage/src/index.ts:10

#### Properties

##### name

> **name**: `string`

Defined in: storage/src/index.ts:11

##### id

> **id**: `string`

Defined in: storage/src/index.ts:12

##### bucket\_id

> **bucket\_id**: `string`

Defined in: storage/src/index.ts:13

##### owner?

> `optional` **owner?**: `string`

Defined in: storage/src/index.ts:14

##### created\_at

> **created\_at**: `string`

Defined in: storage/src/index.ts:15

##### updated\_at

> **updated\_at**: `string`

Defined in: storage/src/index.ts:16

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: storage/src/index.ts:17

***

### Bucket

Defined in: storage/src/index.ts:20

#### Properties

##### id

> **id**: `string`

Defined in: storage/src/index.ts:21

##### name

> **name**: `string`

Defined in: storage/src/index.ts:22

##### public

> **public**: `boolean`

Defined in: storage/src/index.ts:23

##### created\_at

> **created\_at**: `string`

Defined in: storage/src/index.ts:24

##### updated\_at

> **updated\_at**: `string`

Defined in: storage/src/index.ts:25

***

### UploadOptions

Defined in: storage/src/index.ts:28

#### Properties

##### contentType?

> `optional` **contentType?**: `string`

Defined in: storage/src/index.ts:29

##### cacheControl?

> `optional` **cacheControl?**: `string`

Defined in: storage/src/index.ts:30

##### upsert?

> `optional` **upsert?**: `boolean`

Defined in: storage/src/index.ts:31

##### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

Defined in: storage/src/index.ts:32

***

### ListOptions

Defined in: storage/src/index.ts:35

#### Properties

##### limit?

> `optional` **limit?**: `number`

Defined in: storage/src/index.ts:36

##### offset?

> `optional` **offset?**: `number`

Defined in: storage/src/index.ts:37

##### sortBy?

> `optional` **sortBy?**: `object`

Defined in: storage/src/index.ts:38

###### column

> **column**: `string`

###### order

> **order**: `"asc"` \| `"desc"`

##### search?

> `optional` **search?**: `string`

Defined in: storage/src/index.ts:39

***

### SignedUrlOptions

Defined in: storage/src/index.ts:42

#### Properties

##### download?

> `optional` **download?**: `string` \| `boolean`

Defined in: storage/src/index.ts:43

##### transform?

> `optional` **transform?**: `object`

Defined in: storage/src/index.ts:44

###### width?

> `optional` **width?**: `number`

###### height?

> `optional` **height?**: `number`

###### quality?

> `optional` **quality?**: `number`
