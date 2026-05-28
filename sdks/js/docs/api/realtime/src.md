[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / realtime/src

# realtime/src

## Functions

### createRealtimeClient()

> **createRealtimeClient**(`ctx`): [`RealtimeClient`](#realtimeclient)

Defined in: realtime/src/index.ts:75

#### Parameters

##### ctx

`RequestContext`

#### Returns

[`RealtimeClient`](#realtimeclient)

## Classes

### RealtimeClient

Defined in: realtime/src/index.ts:44

Realtime client stub.
Full implementation coming in a future version.

#### Constructors

##### Constructor

> **new RealtimeClient**(`_ctx`): [`RealtimeClient`](#realtimeclient)

Defined in: realtime/src/index.ts:46

###### Parameters

###### \_ctx

`RequestContext`

###### Returns

[`RealtimeClient`](#realtimeclient)

#### Methods

##### channel()

> **channel**(`_name`): [`RealtimeChannel`](#realtimechannel)

Defined in: realtime/src/index.ts:52

Create a channel subscription.

###### Parameters

###### \_name

`string`

###### Returns

[`RealtimeChannel`](#realtimechannel)

###### Stub

- Not yet implemented

##### removeChannel()

> **removeChannel**(`_channel`): `void`

Defined in: realtime/src/index.ts:62

Remove a channel subscription.

###### Parameters

###### \_channel

[`RealtimeChannel`](#realtimechannel)

###### Returns

`void`

###### Stub

- Not yet implemented

##### removeAllChannels()

> **removeAllChannels**(): `void`

Defined in: realtime/src/index.ts:70

Remove all channel subscriptions.

###### Returns

`void`

###### Stub

- Not yet implemented

## Interfaces

### RealtimePayload

Defined in: realtime/src/index.ts:12

#### Type Parameters

##### T

`T` = `unknown`

#### Properties

##### event

> **event**: [`RealtimeEvent`](#realtimeevent)

Defined in: realtime/src/index.ts:13

##### schema

> **schema**: `string`

Defined in: realtime/src/index.ts:14

##### table

> **table**: `string`

Defined in: realtime/src/index.ts:15

##### commit\_timestamp

> **commit\_timestamp**: `string`

Defined in: realtime/src/index.ts:16

##### old\_record?

> `optional` **old\_record?**: `T`

Defined in: realtime/src/index.ts:17

##### new\_record?

> `optional` **new\_record?**: `T`

Defined in: realtime/src/index.ts:18

***

### RealtimeChannel

Defined in: realtime/src/index.ts:21

#### Extended by

- [`RealtimePresenceChannel`](#realtimepresencechannel)

#### Methods

##### on()

> **on**(`event`, `callback`): [`RealtimeChannel`](#realtimechannel)

Defined in: realtime/src/index.ts:22

###### Parameters

###### event

[`RealtimeEvent`](#realtimeevent)

###### callback

(`payload`) => `void`

###### Returns

[`RealtimeChannel`](#realtimechannel)

##### subscribe()

> **subscribe**(): `Promise`\<`void`\>

Defined in: realtime/src/index.ts:26

###### Returns

`Promise`\<`void`\>

##### unsubscribe()

> **unsubscribe**(): `Promise`\<`void`\>

Defined in: realtime/src/index.ts:27

###### Returns

`Promise`\<`void`\>

***

### PresenceState

Defined in: realtime/src/index.ts:30

#### Indexable

> \[`key`: `string`\]: `unknown`[]

***

### RealtimePresenceChannel

Defined in: realtime/src/index.ts:34

#### Extends

- [`RealtimeChannel`](#realtimechannel)

#### Methods

##### on()

> **on**(`event`, `callback`): [`RealtimeChannel`](#realtimechannel)

Defined in: realtime/src/index.ts:22

###### Parameters

###### event

[`RealtimeEvent`](#realtimeevent)

###### callback

(`payload`) => `void`

###### Returns

[`RealtimeChannel`](#realtimechannel)

###### Inherited from

[`RealtimeChannel`](#realtimechannel).[`on`](#on)

##### subscribe()

> **subscribe**(): `Promise`\<`void`\>

Defined in: realtime/src/index.ts:26

###### Returns

`Promise`\<`void`\>

###### Inherited from

[`RealtimeChannel`](#realtimechannel).[`subscribe`](#subscribe)

##### unsubscribe()

> **unsubscribe**(): `Promise`\<`void`\>

Defined in: realtime/src/index.ts:27

###### Returns

`Promise`\<`void`\>

###### Inherited from

[`RealtimeChannel`](#realtimechannel).[`unsubscribe`](#unsubscribe)

##### track()

> **track**(`state`): `Promise`\<`void`\>

Defined in: realtime/src/index.ts:35

###### Parameters

###### state

`Record`\<`string`, `unknown`\>

###### Returns

`Promise`\<`void`\>

##### untrack()

> **untrack**(): `Promise`\<`void`\>

Defined in: realtime/src/index.ts:36

###### Returns

`Promise`\<`void`\>

##### presenceState()

> **presenceState**(): [`PresenceState`](#presencestate)

Defined in: realtime/src/index.ts:37

###### Returns

[`PresenceState`](#presencestate)

## Type Aliases

### RealtimeEvent

> **RealtimeEvent** = `"INSERT"` \| `"UPDATE"` \| `"DELETE"` \| `"*"`

Defined in: realtime/src/index.ts:10

@reactor/realtime - Realtime subscriptions for Reactor

This is a stub package reserving the API surface for future implementation.
Realtime subscriptions will be implemented in a future version.
