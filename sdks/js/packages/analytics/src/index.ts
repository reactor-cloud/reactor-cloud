/**
 * @reactor/analytics - Product analytics SDK for Reactor
 *
 * @example
 * ```ts
 * import { ReactorAnalytics } from '@reactor/analytics';
 *
 * const analytics = new ReactorAnalytics({
 *   projectKey: 'pk_...',
 *   endpoint: 'https://api.reactor.cloud/analytics/v1',
 * });
 *
 * analytics.track('button_clicked', { button_id: 'signup' });
 * analytics.identify('user_123', { email: 'user@example.com' });
 * ```
 */

// Types
export interface AnalyticsConfig {
  /** Project key (X-Reactor-Project-Key header). */
  projectKey: string;
  /** Analytics API endpoint. */
  endpoint: string;
  /** Batch events before sending. */
  batchSize?: number;
  /** Flush interval in milliseconds. */
  flushInterval?: number;
  /** Auto-capture pageviews. */
  autoPageview?: boolean;
  /** Auto-capture errors with fingerprint coalescing. */
  autoErrors?: boolean;
  /** Auto-capture click interactions (opt-in). */
  autoCapture?: boolean;
  /** CSS selector for auto-capture targets. */
  autoCaptureSelector?: string;
  /** Error deduplication window in ms (default 5000). */
  errorDedupeWindow?: number;
  /** Persist anonymous ID to localStorage. */
  persistence?: boolean;
  /** Storage key for persistence. */
  storageKey?: string;
  /** Debug mode (logs events to console). */
  debug?: boolean;
}

export interface EventProperties {
  [key: string]: string | number | boolean | null | undefined | EventProperties | EventProperties[];
}

export interface UserTraits {
  email?: string;
  name?: string;
  [key: string]: string | number | boolean | null | undefined;
}

export interface PageContext {
  path?: string;
  url?: string;
  referrer?: string;
  title?: string;
}

export interface ClientContext {
  library?: { name: string; version: string };
  userAgent?: string;
  locale?: string;
  timezone?: string;
  screen?: { width: number; height: number };
}

export interface QueuedEvent {
  event: string;
  properties: EventProperties;
  timestamp: string;
  anonymousId: string;
  userId?: string;
  sessionId?: string;
  context: ClientContext & PageContext;
}

// Constants
const SDK_NAME = "@reactor/analytics";
const SDK_VERSION = "0.1.0";
const DEFAULT_BATCH_SIZE = 20;
const DEFAULT_FLUSH_INTERVAL = 5000; // 5 seconds
const DEFAULT_STORAGE_KEY = "reactor_anon_id";
const DEFAULT_ERROR_DEDUPE_WINDOW = 5000; // 5 seconds
const DEFAULT_AUTOCAPTURE_SELECTOR = "a, button, input[type='submit'], [data-reactor-capture]";

// Utility functions
function generateId(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  // Fallback for older browsers
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

function getAnonymousId(storageKey: string, persistence: boolean): string {
  if (persistence && typeof localStorage !== "undefined") {
    const stored = localStorage.getItem(storageKey);
    if (stored) return stored;
    const newId = generateId();
    localStorage.setItem(storageKey, newId);
    return newId;
  }
  return generateId();
}

function getClientContext(): ClientContext {
  const ctx: ClientContext = {
    library: { name: SDK_NAME, version: SDK_VERSION },
  };

  if (typeof navigator !== "undefined") {
    ctx.userAgent = navigator.userAgent;
    ctx.locale = navigator.language;
  }

  if (typeof Intl !== "undefined") {
    try {
      ctx.timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
    } catch {}
  }

  if (typeof screen !== "undefined") {
    ctx.screen = { width: screen.width, height: screen.height };
  }

  return ctx;
}

function getPageContext(): PageContext {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return {};
  }

  return {
    path: window.location.pathname,
    url: window.location.href,
    referrer: document.referrer || undefined,
    title: document.title || undefined,
  };
}

function simpleHash(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // Convert to 32bit integer
  }
  return hash.toString(36);
}

function getElementSelector(el: Element): string {
  const parts: string[] = [];
  
  if (el.id) {
    parts.push(`#${el.id}`);
  }
  
  if (el.className && typeof el.className === 'string') {
    const classes = el.className.trim().split(/\s+/).filter(c => c.length > 0);
    if (classes.length > 0) {
      parts.push(`.${classes.slice(0, 2).join('.')}`);
    }
  }
  
  const tagName = el.tagName?.toLowerCase() || 'unknown';
  return parts.length > 0 ? `${tagName}${parts.join('')}` : tagName;
}

function getElementText(el: Element): string {
  const text = (el.textContent || '').trim().slice(0, 100);
  return text || (el as HTMLElement).getAttribute?.('aria-label') || '';
}

/**
 * Reactor Analytics client.
 *
 * Provides methods for tracking events, identifying users, and managing consent.
 */
export class ReactorAnalytics {
  private config: Required<AnalyticsConfig>;
  private anonymousId: string;
  private userId: string | undefined;
  private sessionId: string;
  private queue: QueuedEvent[] = [];
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private optedOut: boolean = false;
  private errorFingerprints: Map<string, number> = new Map();

  constructor(config: AnalyticsConfig) {
    this.config = {
      projectKey: config.projectKey,
      endpoint: config.endpoint,
      batchSize: config.batchSize ?? DEFAULT_BATCH_SIZE,
      flushInterval: config.flushInterval ?? DEFAULT_FLUSH_INTERVAL,
      autoPageview: config.autoPageview ?? false,
      autoErrors: config.autoErrors ?? false,
      autoCapture: config.autoCapture ?? false,
      autoCaptureSelector: config.autoCaptureSelector ?? DEFAULT_AUTOCAPTURE_SELECTOR,
      errorDedupeWindow: config.errorDedupeWindow ?? DEFAULT_ERROR_DEDUPE_WINDOW,
      persistence: config.persistence ?? true,
      storageKey: config.storageKey ?? DEFAULT_STORAGE_KEY,
      debug: config.debug ?? false,
    };

    this.anonymousId = getAnonymousId(
      this.config.storageKey,
      this.config.persistence
    );
    this.sessionId = generateId();

    // Start flush timer
    this.startFlushTimer();

    // Auto-capture setup
    if (typeof window !== "undefined") {
      if (this.config.autoPageview) {
        this.setupAutoPageview();
      }
      if (this.config.autoErrors) {
        this.setupAutoErrors();
      }
      if (this.config.autoCapture) {
        this.setupAutoCapture();
      }
      // Flush on page unload
      window.addEventListener("beforeunload", () => this.flush());
    }
  }

  /**
   * Track an event.
   *
   * @param event - Event name.
   * @param properties - Event properties.
   */
  track(event: string, properties: EventProperties = {}): void {
    if (this.optedOut) return;

    const queuedEvent: QueuedEvent = {
      event,
      properties,
      timestamp: new Date().toISOString(),
      anonymousId: this.anonymousId,
      userId: this.userId,
      sessionId: this.sessionId,
      context: { ...getClientContext(), ...getPageContext() },
    };

    this.enqueue(queuedEvent);
  }

  /**
   * Track a page view.
   *
   * @param name - Page name (optional).
   * @param properties - Additional properties.
   */
  page(name?: string, properties: EventProperties = {}): void {
    const pageContext = getPageContext();
    this.track("$pageview", {
      ...properties,
      name: name || pageContext.title,
      path: pageContext.path,
      url: pageContext.url,
      referrer: pageContext.referrer,
    });
  }

  /**
   * Identify a user.
   *
   * @param userId - User ID.
   * @param traits - User traits (email, name, etc.).
   */
  identify(userId: string, traits: UserTraits = {}): void {
    if (this.optedOut) return;

    this.userId = userId;

    // Send identify event
    const queuedEvent: QueuedEvent = {
      event: "$identify",
      properties: traits as EventProperties,
      timestamp: new Date().toISOString(),
      anonymousId: this.anonymousId,
      userId,
      sessionId: this.sessionId,
      context: getClientContext(),
    };

    this.enqueue(queuedEvent);

    // Also send to identify endpoint
    this.sendIdentify(userId, traits);
  }

  /**
   * Alias an anonymous ID to a user ID.
   *
   * @param previousId - Previous anonymous ID.
   * @param userId - User ID to alias to.
   */
  alias(previousId: string, userId: string): void {
    if (this.optedOut) return;

    const queuedEvent: QueuedEvent = {
      event: "$alias",
      properties: { previousId, userId },
      timestamp: new Date().toISOString(),
      anonymousId: previousId,
      userId,
      sessionId: this.sessionId,
      context: getClientContext(),
    };

    this.enqueue(queuedEvent);

    // Also send to alias endpoint
    this.sendAlias(previousId, userId);
  }

  /**
   * Reset the user identity.
   * Generates a new anonymous ID and clears the user ID.
   */
  reset(): void {
    this.userId = undefined;
    this.anonymousId = generateId();
    this.sessionId = generateId();

    if (this.config.persistence && typeof localStorage !== "undefined") {
      localStorage.setItem(this.config.storageKey, this.anonymousId);
    }
  }

  /**
   * Flush the event queue.
   */
  async flush(): Promise<void> {
    if (this.queue.length === 0) return;

    const events = this.queue.splice(0, this.queue.length);

    if (this.config.debug) {
      console.log("[ReactorAnalytics] Flushing events:", events);
    }

    try {
      const response = await this.sendBatch(events);
      if (!response.ok && this.config.debug) {
        console.error("[ReactorAnalytics] Flush failed:", response.status);
        // Put events back in queue for retry
        this.queue.unshift(...events);
      }
    } catch (error) {
      if (this.config.debug) {
        console.error("[ReactorAnalytics] Flush error:", error);
      }
      // Put events back in queue for retry
      this.queue.unshift(...events);
    }
  }

  /**
   * Opt out of tracking.
   */
  optOut(): void {
    this.optedOut = true;
    this.queue = [];
    this.sendConsentOptOut();
  }

  /**
   * Opt in to tracking.
   */
  optIn(): void {
    this.optedOut = false;
    this.sendConsentOptIn();
  }

  /**
   * Get the current anonymous ID.
   */
  getAnonymousId(): string {
    return this.anonymousId;
  }

  /**
   * Get the current user ID.
   */
  getUserId(): string | undefined {
    return this.userId;
  }

  /**
   * Get the current session ID.
   */
  getSessionId(): string {
    return this.sessionId;
  }

  // --- Private methods ---

  private enqueue(event: QueuedEvent): void {
    this.queue.push(event);

    if (this.config.debug) {
      console.log("[ReactorAnalytics] Event queued:", event);
    }

    if (this.queue.length >= this.config.batchSize) {
      this.flush();
    }
  }

  private startFlushTimer(): void {
    if (this.flushTimer) return;
    this.flushTimer = setInterval(() => {
      this.flush();
    }, this.config.flushInterval);
  }

  private async sendBatch(events: QueuedEvent[]): Promise<Response> {
    const body = {
      events: events.map((e) => ({
        event: e.event,
        anonymous_id: e.anonymousId,
        user_id: e.userId,
        session_id: e.sessionId,
        timestamp: e.timestamp,
        properties: e.properties,
        context: e.context,
      })),
    };

    // Use sendBeacon if available and page is unloading
    if (
      typeof navigator !== "undefined" &&
      navigator.sendBeacon &&
      document?.visibilityState === "hidden"
    ) {
      const blob = new Blob([JSON.stringify(body)], {
        type: "application/json",
      });
      navigator.sendBeacon(`${this.config.endpoint}/batch`, blob);
      return new Response(null, { status: 202 });
    }

    return fetch(`${this.config.endpoint}/batch`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Reactor-Project-Key": this.config.projectKey,
      },
      body: JSON.stringify(body),
      keepalive: true,
    });
  }

  private async sendIdentify(userId: string, traits: UserTraits): Promise<void> {
    try {
      await fetch(`${this.config.endpoint}/identify`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Reactor-Project-Key": this.config.projectKey,
        },
        body: JSON.stringify({
          anonymous_id: this.anonymousId,
          user_id: userId,
          traits,
        }),
      });
    } catch (error) {
      if (this.config.debug) {
        console.error("[ReactorAnalytics] Identify error:", error);
      }
    }
  }

  private async sendAlias(previousId: string, userId: string): Promise<void> {
    try {
      await fetch(`${this.config.endpoint}/alias`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Reactor-Project-Key": this.config.projectKey,
        },
        body: JSON.stringify({
          anonymous_id: previousId,
          user_id: userId,
        }),
      });
    } catch (error) {
      if (this.config.debug) {
        console.error("[ReactorAnalytics] Alias error:", error);
      }
    }
  }

  private async sendConsentOptOut(): Promise<void> {
    try {
      await fetch(`${this.config.endpoint}/consent/opt-out`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Reactor-Project-Key": this.config.projectKey,
        },
        body: JSON.stringify({ anonymous_id: this.anonymousId }),
      });
    } catch {}
  }

  private async sendConsentOptIn(): Promise<void> {
    try {
      await fetch(`${this.config.endpoint}/consent/opt-in`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Reactor-Project-Key": this.config.projectKey,
        },
        body: JSON.stringify({ anonymous_id: this.anonymousId }),
      });
    } catch {}
  }

  private setupAutoPageview(): void {
    // Initial pageview
    this.page();

    // SPA navigation hooks
    if (typeof window !== "undefined") {
      const originalPushState = history.pushState;
      const originalReplaceState = history.replaceState;

      history.pushState = (...args) => {
        originalPushState.apply(history, args);
        this.page();
      };

      history.replaceState = (...args) => {
        originalReplaceState.apply(history, args);
        this.page();
      };

      window.addEventListener("popstate", () => this.page());
    }
  }

  private setupAutoErrors(): void {
    if (typeof window === "undefined") return;

    window.addEventListener("error", (event) => {
      const fingerprint = this.generateErrorFingerprint(
        event.message,
        event.filename,
        event.lineno
      );

      if (this.shouldTrackError(fingerprint)) {
        this.track("$error", {
          message: event.message,
          filename: event.filename,
          lineno: event.lineno,
          colno: event.colno,
          error: event.error?.toString(),
          fingerprint,
        });
      }
    });

    window.addEventListener("unhandledrejection", (event) => {
      const reason = String(event.reason);
      const fingerprint = this.generateErrorFingerprint(
        "Unhandled Promise Rejection",
        reason,
        0
      );

      if (this.shouldTrackError(fingerprint)) {
        this.track("$error", {
          message: "Unhandled Promise Rejection",
          reason,
          fingerprint,
        });
      }
    });
  }

  private generateErrorFingerprint(
    message: string,
    filename: string | undefined,
    lineno: number | undefined
  ): string {
    const key = `${message}|${filename || ''}|${lineno || 0}`;
    return simpleHash(key);
  }

  private shouldTrackError(fingerprint: string): boolean {
    const now = Date.now();
    const lastSeen = this.errorFingerprints.get(fingerprint);

    if (lastSeen && now - lastSeen < this.config.errorDedupeWindow) {
      if (this.config.debug) {
        console.log("[ReactorAnalytics] Deduped error:", fingerprint);
      }
      return false;
    }

    this.errorFingerprints.set(fingerprint, now);

    // Cleanup old fingerprints periodically
    if (this.errorFingerprints.size > 100) {
      const cutoff = now - this.config.errorDedupeWindow * 2;
      for (const [key, time] of this.errorFingerprints.entries()) {
        if (time < cutoff) {
          this.errorFingerprints.delete(key);
        }
      }
    }

    return true;
  }

  private setupAutoCapture(): void {
    if (typeof window === "undefined" || typeof document === "undefined") return;

    document.addEventListener("click", (event) => {
      const target = event.target as Element | null;
      if (!target) return;

      // Find the closest matching element
      const el = target.closest(this.config.autoCaptureSelector);
      if (!el) return;

      const tagName = el.tagName?.toLowerCase() || 'unknown';
      const selector = getElementSelector(el);
      const text = getElementText(el);

      this.track("$autocapture", {
        event_type: "click",
        tag_name: tagName,
        selector,
        text: text.slice(0, 100),
        href: (el as HTMLAnchorElement).href || undefined,
        name: el.getAttribute("name") || undefined,
        id: el.id || undefined,
        classes: el.className || undefined,
      });
    }, { capture: true, passive: true });
  }
}

// Default export
export default ReactorAnalytics;
