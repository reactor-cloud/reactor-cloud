import type { RequestContext } from '@reactor/shared';

/**
 * @reactor/realtime - Realtime subscriptions for Reactor
 *
 * This is a stub package reserving the API surface for future implementation.
 * Realtime subscriptions will be implemented in a future version.
 */

export type RealtimeEvent = 'INSERT' | 'UPDATE' | 'DELETE' | '*';

export interface RealtimePayload<T = unknown> {
  event: RealtimeEvent;
  schema: string;
  table: string;
  commit_timestamp: string;
  old_record?: T;
  new_record?: T;
}

export interface RealtimeChannel {
  on(
    event: RealtimeEvent,
    callback: (payload: RealtimePayload) => void
  ): RealtimeChannel;
  subscribe(): Promise<void>;
  unsubscribe(): Promise<void>;
}

export interface PresenceState {
  [key: string]: unknown[];
}

export interface RealtimePresenceChannel extends RealtimeChannel {
  track(state: Record<string, unknown>): Promise<void>;
  untrack(): Promise<void>;
  presenceState(): PresenceState;
}

/**
 * Realtime client stub.
 * Full implementation coming in a future version.
 */
export class RealtimeClient {
  // eslint-disable-next-line @typescript-eslint/no-useless-constructor
  constructor(_ctx: RequestContext) {}

  /**
   * Create a channel subscription.
   * @stub - Not yet implemented
   */
  channel(_name: string): RealtimeChannel {
    throw new Error(
      '@reactor/realtime is not yet implemented. This package reserves the API surface for a future version.'
    );
  }

  /**
   * Remove a channel subscription.
   * @stub - Not yet implemented
   */
  removeChannel(_channel: RealtimeChannel): void {
    throw new Error('@reactor/realtime is not yet implemented.');
  }

  /**
   * Remove all channel subscriptions.
   * @stub - Not yet implemented
   */
  removeAllChannels(): void {
    throw new Error('@reactor/realtime is not yet implemented.');
  }
}

export function createRealtimeClient(ctx: RequestContext): RealtimeClient {
  return new RealtimeClient(ctx);
}
