[**@reactor/sdk-workspace**](../README.md)

***

[@reactor/sdk-workspace](../README.md) / data/src

# data/src

## Functions

### createDataClient()

> **createDataClient**\<`Schema`\>(`ctx`): [`DataClient`](#dataclient)\<`Schema`\>

Defined in: data/src/client.ts:82

Create a data client with typed schema.

#### Type Parameters

##### Schema

`Schema` *extends* [`GenericSchema`](#genericschema) = [`GenericSchema`](#genericschema)

#### Parameters

##### ctx

`RequestContext`

#### Returns

[`DataClient`](#dataclient)\<`Schema`\>

***

### rpc()

> **rpc**\<`Args`, `Returns`\>(`ctx`, `functionName`, `args`, `options?`): `Promise`\<[`Result`](../client/src.md#result-1)\<`Returns`\>\>

Defined in: data/src/rpc.ts:10

Call a database function via RPC.

#### Type Parameters

##### Args

`Args` *extends* `Record`\<`string`, `unknown`\>

##### Returns

`Returns`

#### Parameters

##### ctx

`RequestContext`

##### functionName

`string`

##### args

`Args`

##### options?

###### signal?

`AbortSignal`

###### headers?

`Record`\<`string`, `string`\>

#### Returns

`Promise`\<[`Result`](../client/src.md#result-1)\<`Returns`\>\>

## Classes

### PostgrestFilterBuilder

Defined in: data/src/builder.ts:22

PostgrestFilterBuilder provides methods for building filter queries.

#### Extended by

- [`PostgrestQueryBuilder`](#postgrestquerybuilder)

#### Type Parameters

##### T

`T`

##### ResultType

`ResultType` = `T`[]

#### Constructors

##### Constructor

> **new PostgrestFilterBuilder**\<`T`, `ResultType`\>(`ctx`, `table`): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `ResultType`\>

Defined in: data/src/builder.ts:39

###### Parameters

###### ctx

`RequestContext`

###### table

`string`

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `ResultType`\>

#### Methods

##### eq()

> **eq**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:45

Equal to

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

##### neq()

> **neq**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:51

Not equal to

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

##### gt()

> **gt**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:57

Greater than

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

##### gte()

> **gte**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:63

Greater than or equal

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

##### lt()

> **lt**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:69

Less than

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

##### lte()

> **lte**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:75

Less than or equal

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

##### like()

> **like**\<`K`\>(`column`, `pattern`): `this`

Defined in: data/src/builder.ts:81

Pattern match (LIKE)

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### pattern

`string`

###### Returns

`this`

##### ilike()

> **ilike**\<`K`\>(`column`, `pattern`): `this`

Defined in: data/src/builder.ts:87

Case-insensitive pattern match (ILIKE)

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### pattern

`string`

###### Returns

`this`

##### is()

> **is**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:93

Is NULL or boolean

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`boolean`

###### Returns

`this`

##### in()

> **in**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:99

In list

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`T`\[`K`\][]

###### Returns

`this`

##### contains()

> **contains**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:105

Array contains

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`unknown`[]

###### Returns

`this`

##### containedBy()

> **containedBy**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:111

Array contained by

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`unknown`[]

###### Returns

`this`

##### overlaps()

> **overlaps**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:117

Array overlaps

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`unknown`[]

###### Returns

`this`

##### textSearch()

> **textSearch**\<`K`\>(`column`, `query`, `options?`): `this`

Defined in: data/src/builder.ts:123

Full-text search

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### query

`string`

###### options?

[`TextSearchOptions`](#textsearchoptions)

###### Returns

`this`

##### match()

> **match**(`query`): `this`

Defined in: data/src/builder.ts:131

Match multiple conditions (shorthand for multiple eq)

###### Parameters

###### query

`Partial`\<`T`\>

###### Returns

`this`

##### not()

> **not**\<`K`\>(`column`, `operator`, `value`): `this`

Defined in: data/src/builder.ts:141

Negate a filter

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### operator

[`FilterOperator`](#filteroperator)

###### value

[`FilterValue`](#filtervalue)

###### Returns

`this`

##### or()

> **or**(`conditions`, `options?`): `this`

Defined in: data/src/builder.ts:147

OR condition (raw string format)

###### Parameters

###### conditions

`string`

###### options?

###### foreignTable?

`string`

###### Returns

`this`

##### filter()

> **filter**\<`K`\>(`column`, `operator`, `value`): `this`

Defined in: data/src/builder.ts:154

Generic filter (escape hatch)

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### operator

[`FilterOperator`](#filteroperator)

###### value

[`FilterValue`](#filtervalue)

###### Returns

`this`

##### order()

> **order**\<`K`\>(`column`, `options?`): `this`

Defined in: data/src/builder.ts:160

Order results

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### options?

[`OrderOptions`](#orderoptions)

###### Returns

`this`

##### limit()

> **limit**(`count`, `options?`): `this`

Defined in: data/src/builder.ts:177

Limit results

###### Parameters

###### count

`number`

###### options?

###### foreignTable?

`string`

###### Returns

`this`

##### range()

> **range**(`from`, `to`, `options?`): `this`

Defined in: data/src/builder.ts:187

Offset results (for pagination)

###### Parameters

###### from

`number`

###### to

`number`

###### options?

###### foreignTable?

`string`

###### Returns

`this`

##### abortSignal()

> **abortSignal**(`signal`): `this`

Defined in: data/src/builder.ts:199

Provide an AbortSignal

###### Parameters

###### signal

`AbortSignal`

###### Returns

`this`

##### csv()

> **csv**(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `string`\>

Defined in: data/src/builder.ts:205

Return CSV instead of JSON

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `string`\>

##### explain()

> **explain**(`options?`): `this`

Defined in: data/src/builder.ts:211

Return query execution plan

###### Parameters

###### options?

###### analyze?

`boolean`

###### verbose?

`boolean`

###### costs?

`boolean`

###### buffers?

`boolean`

###### Returns

`this`

##### returns()

> **returns**\<`R`\>(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`R`, `R`[]\>

Defined in: data/src/builder.ts:217

Override return type

###### Type Parameters

###### R

`R`

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`R`, `R`[]\>

##### single()

> **single**(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

Defined in: data/src/builder.ts:222

Execute and return exactly one row (throws if not exactly one)

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

##### maybeSingle()

> **maybeSingle**(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

Defined in: data/src/builder.ts:228

Execute and return zero or one row

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

##### then()

> **then**\<`TResult1`, `TResult2`\>(`onfulfilled?`, `_onrejected?`): `Promise`\<`TResult1` \| `TResult2`\>

Defined in: data/src/builder.ts:292

Execute the query

###### Type Parameters

###### TResult1

`TResult1` = [`Result`](../client/src.md#result-1)\<`ResultType`\>

###### TResult2

`TResult2` = `never`

###### Parameters

###### onfulfilled?

(`value`) => `TResult1` \| `PromiseLike`\<`TResult1`\>

###### \_onrejected?

(`reason`) => `TResult2` \| `PromiseLike`\<`TResult2`\>

###### Returns

`Promise`\<`TResult1` \| `TResult2`\>

##### throwOnError()

> **throwOnError**(): `Promise`\<`ResultType`\>

Defined in: data/src/builder.ts:328

Throw on error instead of returning { data, error }

###### Returns

`Promise`\<`ResultType`\>

***

### PostgrestQueryBuilder

Defined in: data/src/builder.ts:346

Builder for SELECT queries with column selection.

#### Extends

- [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

#### Type Parameters

##### T

`T`

#### Constructors

##### Constructor

> **new PostgrestQueryBuilder**\<`T`\>(`ctx`, `table`): [`PostgrestQueryBuilder`](#postgrestquerybuilder)\<`T`\>

Defined in: data/src/builder.ts:39

###### Parameters

###### ctx

`RequestContext`

###### table

`string`

###### Returns

[`PostgrestQueryBuilder`](#postgrestquerybuilder)\<`T`\>

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`constructor`](#constructor)

#### Methods

##### eq()

> **eq**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:45

Equal to

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`eq`](#eq)

##### neq()

> **neq**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:51

Not equal to

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`neq`](#neq)

##### gt()

> **gt**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:57

Greater than

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`gt`](#gt)

##### gte()

> **gte**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:63

Greater than or equal

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`gte`](#gte)

##### lt()

> **lt**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:69

Less than

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`lt`](#lt)

##### lte()

> **lte**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:75

Less than or equal

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`T`\[`K`\]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`lte`](#lte)

##### like()

> **like**\<`K`\>(`column`, `pattern`): `this`

Defined in: data/src/builder.ts:81

Pattern match (LIKE)

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### pattern

`string`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`like`](#like)

##### ilike()

> **ilike**\<`K`\>(`column`, `pattern`): `this`

Defined in: data/src/builder.ts:87

Case-insensitive pattern match (ILIKE)

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### pattern

`string`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`ilike`](#ilike)

##### is()

> **is**\<`K`\>(`column`, `value`): `this`

Defined in: data/src/builder.ts:93

Is NULL or boolean

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### value

`boolean`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`is`](#is)

##### in()

> **in**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:99

In list

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`T`\[`K`\][]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`in`](#in)

##### contains()

> **contains**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:105

Array contains

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`unknown`[]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`contains`](#contains)

##### containedBy()

> **containedBy**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:111

Array contained by

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`unknown`[]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`containedBy`](#containedby)

##### overlaps()

> **overlaps**\<`K`\>(`column`, `values`): `this`

Defined in: data/src/builder.ts:117

Array overlaps

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### values

`unknown`[]

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`overlaps`](#overlaps)

##### textSearch()

> **textSearch**\<`K`\>(`column`, `query`, `options?`): `this`

Defined in: data/src/builder.ts:123

Full-text search

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### query

`string`

###### options?

[`TextSearchOptions`](#textsearchoptions)

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`textSearch`](#textsearch)

##### match()

> **match**(`query`): `this`

Defined in: data/src/builder.ts:131

Match multiple conditions (shorthand for multiple eq)

###### Parameters

###### query

`Partial`\<`T`\>

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`match`](#match)

##### not()

> **not**\<`K`\>(`column`, `operator`, `value`): `this`

Defined in: data/src/builder.ts:141

Negate a filter

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### operator

[`FilterOperator`](#filteroperator)

###### value

[`FilterValue`](#filtervalue)

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`not`](#not)

##### or()

> **or**(`conditions`, `options?`): `this`

Defined in: data/src/builder.ts:147

OR condition (raw string format)

###### Parameters

###### conditions

`string`

###### options?

###### foreignTable?

`string`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`or`](#or)

##### filter()

> **filter**\<`K`\>(`column`, `operator`, `value`): `this`

Defined in: data/src/builder.ts:154

Generic filter (escape hatch)

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### operator

[`FilterOperator`](#filteroperator)

###### value

[`FilterValue`](#filtervalue)

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`filter`](#filter)

##### order()

> **order**\<`K`\>(`column`, `options?`): `this`

Defined in: data/src/builder.ts:160

Order results

###### Type Parameters

###### K

`K` *extends* `string`

###### Parameters

###### column

`K`

###### options?

[`OrderOptions`](#orderoptions)

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`order`](#order)

##### limit()

> **limit**(`count`, `options?`): `this`

Defined in: data/src/builder.ts:177

Limit results

###### Parameters

###### count

`number`

###### options?

###### foreignTable?

`string`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`limit`](#limit)

##### range()

> **range**(`from`, `to`, `options?`): `this`

Defined in: data/src/builder.ts:187

Offset results (for pagination)

###### Parameters

###### from

`number`

###### to

`number`

###### options?

###### foreignTable?

`string`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`range`](#range)

##### abortSignal()

> **abortSignal**(`signal`): `this`

Defined in: data/src/builder.ts:199

Provide an AbortSignal

###### Parameters

###### signal

`AbortSignal`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`abortSignal`](#abortsignal)

##### csv()

> **csv**(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `string`\>

Defined in: data/src/builder.ts:205

Return CSV instead of JSON

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `string`\>

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`csv`](#csv)

##### explain()

> **explain**(`options?`): `this`

Defined in: data/src/builder.ts:211

Return query execution plan

###### Parameters

###### options?

###### analyze?

`boolean`

###### verbose?

`boolean`

###### costs?

`boolean`

###### buffers?

`boolean`

###### Returns

`this`

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`explain`](#explain)

##### returns()

> **returns**\<`R`\>(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`R`, `R`[]\>

Defined in: data/src/builder.ts:217

Override return type

###### Type Parameters

###### R

`R`

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`R`, `R`[]\>

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`returns`](#returns)

##### single()

> **single**(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

Defined in: data/src/builder.ts:222

Execute and return exactly one row (throws if not exactly one)

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`single`](#single)

##### maybeSingle()

> **maybeSingle**(): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

Defined in: data/src/builder.ts:228

Execute and return zero or one row

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`, `T`\>

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`maybeSingle`](#maybesingle)

##### then()

> **then**\<`TResult1`, `TResult2`\>(`onfulfilled?`, `_onrejected?`): `Promise`\<`TResult1` \| `TResult2`\>

Defined in: data/src/builder.ts:292

Execute the query

###### Type Parameters

###### TResult1

`TResult1` = [`Result`](../client/src.md#result-1)\<`T`[]\>

###### TResult2

`TResult2` = `never`

###### Parameters

###### onfulfilled?

(`value`) => `TResult1` \| `PromiseLike`\<`TResult1`\>

###### \_onrejected?

(`reason`) => `TResult2` \| `PromiseLike`\<`TResult2`\>

###### Returns

`Promise`\<`TResult1` \| `TResult2`\>

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`then`](#then)

##### throwOnError()

> **throwOnError**(): `Promise`\<`T`[]\>

Defined in: data/src/builder.ts:328

Throw on error instead of returning { data, error }

###### Returns

`Promise`\<`T`[]\>

###### Inherited from

[`PostgrestFilterBuilder`](#postgrestfilterbuilder).[`throwOnError`](#throwonerror)

##### select()

> **select**\<`Columns`\>(`columns?`, `options?`): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

Defined in: data/src/builder.ts:348

Select specific columns

###### Type Parameters

###### Columns

`Columns` *extends* `string` = `"*"`

###### Parameters

###### columns?

`Columns`

###### options?

###### count?

[`CountMode`](#countmode)

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

##### insert()

> **insert**(`values`, `options?`): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

Defined in: data/src/builder.ts:359

Insert row(s)

###### Parameters

###### values

`Partial`\<`T`\> \| `Partial`\<`T`\>[]

###### options?

###### count?

[`CountMode`](#countmode)

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

##### upsert()

> **upsert**(`values`, `options?`): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

Defined in: data/src/builder.ts:367

Upsert row(s)

###### Parameters

###### values

`Partial`\<`T`\> \| `Partial`\<`T`\>[]

###### options?

[`UpsertOptions`](#upsertoptions)

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

##### update()

> **update**(`values`, `options?`): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

Defined in: data/src/builder.ts:379

Update row(s)

###### Parameters

###### values

`Partial`\<`T`\>

###### options?

###### count?

[`CountMode`](#countmode)

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

##### delete()

> **delete**(`options?`): [`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

Defined in: data/src/builder.ts:387

Delete row(s)

###### Parameters

###### options?

###### count?

[`CountMode`](#countmode)

###### Returns

[`PostgrestFilterBuilder`](#postgrestfilterbuilder)\<`T`\>

***

### DataClient

Defined in: data/src/client.ts:32

Data client for Reactor - PostgREST-style query builder.

#### Example

```ts
const client = new DataClient(ctx);

// Select with filters
const { data, error } = await client
  .from('posts')
  .select('id, title, author:users(name)')
  .eq('published', true)
  .order('created_at', { ascending: false })
  .limit(10);

// Insert
const { data } = await client
  .from('posts')
  .insert({ title: 'Hello', body: 'World' })
  .select()
  .single();

// RPC
const { data } = await client.rpc('search', { query: 'rust' });
```

#### Type Parameters

##### Schema

`Schema` *extends* [`GenericSchema`](#genericschema) = [`GenericSchema`](#genericschema)

#### Constructors

##### Constructor

> **new DataClient**\<`Schema`\>(`ctx`): [`DataClient`](#dataclient)\<`Schema`\>

Defined in: data/src/client.ts:33

###### Parameters

###### ctx

`RequestContext`

###### Returns

[`DataClient`](#dataclient)\<`Schema`\>

#### Methods

##### from()

> **from**\<`TableName`\>(`table`): [`PostgrestQueryBuilder`](#postgrestquerybuilder)\<`Schema`\[`"Tables"`\]\[`TableName`\]\[`"Row"`\]\>

Defined in: data/src/client.ts:41

Start a query on a table.

###### Type Parameters

###### TableName

`TableName` *extends* `string`

###### Parameters

###### table

`TableName`

The table name

###### Returns

[`PostgrestQueryBuilder`](#postgrestquerybuilder)\<`Schema`\[`"Tables"`\]\[`TableName`\]\[`"Row"`\]\>

A query builder

##### rpc()

> **rpc**\<`FunctionName`, `Args`, `Returns`\>(`functionName`, `args?`): [`RpcBuilder`](#rpcbuilder)\<`Args`, `Returns`\>

Defined in: data/src/client.ts:54

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

[`RpcBuilder`](#rpcbuilder)\<`Args`, `Returns`\>

RPC builder

##### schema()

> **schema**(`_name`): [`DataClient`](#dataclient)\<`Schema`\>

Defined in: data/src/client.ts:73

Access a schema (for multi-schema support).
Currently returns self as we only support public schema.

###### Parameters

###### \_name

`string`

###### Returns

[`DataClient`](#dataclient)\<`Schema`\>

***

### RpcBuilder

Defined in: data/src/rpc.ts:30

RPC builder for type-safe function calls.

#### Type Parameters

##### Args

`Args` *extends* `Record`\<`string`, `unknown`\>

##### Returns

`Returns`

#### Constructors

##### Constructor

> **new RpcBuilder**\<`Args`, `Returns`\>(`ctx`, `functionName`): [`RpcBuilder`](#rpcbuilder)\<`Args`, `Returns`\>

Defined in: data/src/rpc.ts:35

###### Parameters

###### ctx

`RequestContext`

###### functionName

`string`

###### Returns

[`RpcBuilder`](#rpcbuilder)\<`Args`, `Returns`\>

#### Methods

##### call()

> **call**(`args`): `this`

Defined in: data/src/rpc.ts:41

Set function arguments

###### Parameters

###### args

`Args`

###### Returns

`this`

##### abortSignal()

> **abortSignal**(`signal`): `this`

Defined in: data/src/rpc.ts:47

Provide an AbortSignal

###### Parameters

###### signal

`AbortSignal`

###### Returns

`this`

##### headers()

> **headers**(`headers`): `this`

Defined in: data/src/rpc.ts:53

Set custom headers

###### Parameters

###### headers

`Record`\<`string`, `string`\>

###### Returns

`this`

##### then()

> **then**\<`TResult1`, `TResult2`\>(`onfulfilled?`, `_onrejected?`): `Promise`\<`TResult1` \| `TResult2`\>

Defined in: data/src/rpc.ts:59

Execute the RPC call

###### Type Parameters

###### TResult1

`TResult1` = [`Result`](../client/src.md#result-1)\<`Returns`\>

###### TResult2

`TResult2` = `never`

###### Parameters

###### onfulfilled?

(`value`) => `TResult1` \| `PromiseLike`\<`TResult1`\>

###### \_onrejected?

(`reason`) => `TResult2` \| `PromiseLike`\<`TResult2`\>

###### Returns

`Promise`\<`TResult1` \| `TResult2`\>

##### throwOnError()

> **throwOnError**(): `Promise`\<`Returns`\>

Defined in: data/src/rpc.ts:80

Throw on error

###### Returns

`Promise`\<`Returns`\>

## Interfaces

### QueryOptions

Defined in: data/src/types.ts:47

Query execution options.

#### Properties

##### signal?

> `optional` **signal?**: `AbortSignal`

Defined in: data/src/types.ts:49

AbortSignal for cancellation

##### count?

> `optional` **count?**: [`CountMode`](#countmode)

Defined in: data/src/types.ts:51

Count mode

##### headers?

> `optional` **headers?**: `Record`\<`string`, `string`\>

Defined in: data/src/types.ts:53

Custom headers

##### csv?

> `optional` **csv?**: `boolean`

Defined in: data/src/types.ts:55

Return result as CSV

##### explain?

> `optional` **explain?**: `boolean` \| \{ `analyze?`: `boolean`; `verbose?`: `boolean`; `costs?`: `boolean`; `buffers?`: `boolean`; \}

Defined in: data/src/types.ts:57

Return query execution plan

***

### OrderOptions

Defined in: data/src/types.ts:63

Order options.

#### Properties

##### ascending?

> `optional` **ascending?**: `boolean`

Defined in: data/src/types.ts:64

##### nullsFirst?

> `optional` **nullsFirst?**: `boolean`

Defined in: data/src/types.ts:65

##### foreignTable?

> `optional` **foreignTable?**: `string`

Defined in: data/src/types.ts:66

***

### UpsertOptions

Defined in: data/src/types.ts:72

Upsert options.

#### Properties

##### onConflict?

> `optional` **onConflict?**: `string`

Defined in: data/src/types.ts:73

##### ignoreDuplicates?

> `optional` **ignoreDuplicates?**: `boolean`

Defined in: data/src/types.ts:74

##### count?

> `optional` **count?**: [`CountMode`](#countmode)

Defined in: data/src/types.ts:75

***

### QueryResult

Defined in: data/src/types.ts:81

Query result with optional count.

#### Type Parameters

##### T

`T`

#### Properties

##### data

> **data**: `T`

Defined in: data/src/types.ts:82

##### count?

> `optional` **count?**: `number`

Defined in: data/src/types.ts:83

***

### TextSearchOptions

Defined in: data/src/types.ts:89

Full-text search options.

#### Properties

##### type?

> `optional` **type?**: `"plain"` \| `"phrase"` \| `"websearch"`

Defined in: data/src/types.ts:90

##### config?

> `optional` **config?**: `string`

Defined in: data/src/types.ts:91

***

### PendingFilter

Defined in: data/src/types.ts:102

Pending filter to be applied.

#### Properties

##### column

> **column**: `string`

Defined in: data/src/types.ts:103

##### operator

> **operator**: [`FilterOperator`](#filteroperator)

Defined in: data/src/types.ts:104

##### value

> **value**: [`FilterValue`](#filtervalue)

Defined in: data/src/types.ts:105

##### negated

> **negated**: `boolean`

Defined in: data/src/types.ts:106

## Type Aliases

### GenericSchema

> **GenericSchema** = `object`

Defined in: data/src/types.ts:4

Generic schema for untyped queries.

#### Properties

##### Tables

> **Tables**: `Record`\<`string`, \{ `Row`: `Record`\<`string`, `unknown`\>; `Insert`: `Record`\<`string`, `unknown`\>; `Update`: `Record`\<`string`, `unknown`\>; \}\>

Defined in: data/src/types.ts:5

##### Views

> **Views**: `Record`\<`string`, \{ `Row`: `Record`\<`string`, `unknown`\>; \}\>

Defined in: data/src/types.ts:6

##### Functions

> **Functions**: `Record`\<`string`, \{ `Args`: `Record`\<`string`, `unknown`\>; `Returns`: `unknown`; \}\>

Defined in: data/src/types.ts:7

***

### CountMode

> **CountMode** = `"exact"` \| `"planned"` \| `"estimated"`

Defined in: data/src/types.ts:13

Count mode for queries.

***

### ResponseFormat

> **ResponseFormat** = `"json"` \| `"csv"` \| `"geojson"`

Defined in: data/src/types.ts:18

Response format options.

***

### FilterOperator

> **FilterOperator** = `"eq"` \| `"neq"` \| `"gt"` \| `"gte"` \| `"lt"` \| `"lte"` \| `"like"` \| `"ilike"` \| `"in"` \| `"is"` \| `"cs"` \| `"cd"` \| `"ov"` \| `"fts"`

Defined in: data/src/types.ts:23

Filter operators supported by reactor-data.

***

### ResultModifier

> **ResultModifier** = `"single"` \| `"maybeSingle"`

Defined in: data/src/types.ts:42

Result modifier for single row queries.

***

### FilterValue

> **FilterValue** = `string` \| `number` \| `boolean` \| `null` \| (`string` \| `number` \| `boolean`)[]

Defined in: data/src/types.ts:97

Filter value types.

***

### SelectResult

> **SelectResult**\<`T`, `Columns`\> = `Pick`\<`T`, `Extract`\<keyof `T`, `Columns`\>\>

Defined in: data/src/types.ts:112

Type helper for selecting specific columns.

#### Type Parameters

##### T

`T`

##### Columns

`Columns` *extends* `string`
