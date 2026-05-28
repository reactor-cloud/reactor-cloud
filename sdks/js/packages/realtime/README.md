# @reactor/realtime

Realtime client for Reactor. Subscribe to database changes and broadcast events.

> **Note:** Realtime is currently a stub implementation. Full functionality coming soon.

## Installation

```bash
npm install @reactor/realtime @reactor/shared
```

Or use the unified client:

```bash
npm install @reactor/client
```

## Quick Start

```typescript
import { RealtimeClient } from '@reactor/realtime';

const realtime = new RealtimeClient(ctx);

// Subscribe to database changes
const channel = realtime.channel('db-changes');

channel
  .on('postgres_changes', {
    event: 'INSERT',
    schema: 'public',
    table: 'messages',
  }, (payload) => {
    console.log('New message:', payload.new);
  })
  .subscribe();

// Broadcast messages
const broadcastChannel = realtime.channel('room:123');

broadcastChannel
  .on('broadcast', { event: 'typing' }, (payload) => {
    console.log('User typing:', payload);
  })
  .subscribe();

// Send broadcast
broadcastChannel.send({
  type: 'broadcast',
  event: 'typing',
  payload: { userId: '123' },
});

// Presence tracking
const presenceChannel = realtime.channel('online-users');

presenceChannel
  .on('presence', { event: 'sync' }, () => {
    const state = presenceChannel.presenceState();
    console.log('Online users:', state);
  })
  .subscribe(async (status) => {
    if (status === 'SUBSCRIBED') {
      await presenceChannel.track({ user_id: '123', online_at: new Date() });
    }
  });

// Unsubscribe
channel.unsubscribe();
realtime.removeChannel(channel);
```

## Documentation

- [Realtime Guide](https://reactor.cloud/docs/realtime)
- [API Reference](https://reactor.cloud/docs)

## License

MIT
