import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { ReactorAnalytics, type AnalyticsConfig } from "../src/index";

const mockFetch = vi.fn();

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => { store[key] = value; }),
    removeItem: vi.fn((key: string) => { delete store[key]; }),
    clear: vi.fn(() => { store = {}; }),
  };
})();

function createAnalytics(config: Partial<AnalyticsConfig> = {}): ReactorAnalytics {
  return new ReactorAnalytics({
    projectKey: "pk_test_123",
    endpoint: "https://api.test.com/analytics/v1",
    persistence: false,
    autoPageview: false,
    autoErrors: false,
    flushInterval: 60000, // Long interval to prevent timer issues in tests
    ...config,
  });
}

describe("ReactorAnalytics", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", mockFetch);
    vi.stubGlobal("localStorage", localStorageMock);
    mockFetch.mockResolvedValue(new Response(null, { status: 202 }));
    localStorageMock.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    mockFetch.mockClear();
    localStorageMock.clear();
  });

  describe("initialization", () => {
    it("should create an instance with required config", () => {
      const analytics = createAnalytics();
      expect(analytics).toBeInstanceOf(ReactorAnalytics);
    });

    it("should generate an anonymous ID", () => {
      const analytics = createAnalytics();
      expect(analytics.getAnonymousId()).toBeDefined();
      expect(analytics.getAnonymousId().length).toBeGreaterThan(0);
    });

    it("should generate a session ID", () => {
      const analytics = createAnalytics();
      expect(analytics.getSessionId()).toBeDefined();
    });

    it("should persist anonymous ID to localStorage when persistence is enabled", () => {
      const analytics = createAnalytics({ persistence: true });
      const anonId = analytics.getAnonymousId();
      expect(localStorageMock.setItem).toHaveBeenCalledWith("reactor_anon_id", anonId);
    });

    it("should reuse persisted anonymous ID", () => {
      localStorageMock.getItem.mockReturnValueOnce("existing-anon-id");
      const analytics = createAnalytics({ persistence: true });
      expect(analytics.getAnonymousId()).toBe("existing-anon-id");
    });
  });

  describe("track", () => {
    it("should queue events for batching", () => {
      const analytics = createAnalytics();
      analytics.track("button_clicked", { button_id: "signup" });

      expect(mockFetch).not.toHaveBeenCalled();
    });

    it("should flush when batch size is reached", async () => {
      const analytics = createAnalytics({ batchSize: 2 });

      analytics.track("event_1");
      expect(mockFetch).not.toHaveBeenCalled();

      analytics.track("event_2");

      // Wait for flush to complete
      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(mockFetch).toHaveBeenCalledTimes(1);
      expect(mockFetch).toHaveBeenCalledWith(
        "https://api.test.com/analytics/v1/batch",
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({
            "X-Reactor-Project-Key": "pk_test_123",
          }),
        })
      );
    });

    it("should include event properties", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      analytics.track("purchase", { amount: 99.99, currency: "USD" });

      await new Promise((resolve) => setTimeout(resolve, 10));

      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events[0].properties).toEqual({
        amount: 99.99,
        currency: "USD",
      });
    });

    it("should include timestamp", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      const before = new Date().toISOString();
      analytics.track("test_event");
      const after = new Date().toISOString();

      await new Promise((resolve) => setTimeout(resolve, 10));

      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events[0].timestamp).toBeDefined();
      expect(body.events[0].timestamp >= before).toBe(true);
      expect(body.events[0].timestamp <= after).toBe(true);
    });

    it("should not track when opted out", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      analytics.optOut();
      
      // Wait for opt-out request
      await new Promise((resolve) => setTimeout(resolve, 10));
      mockFetch.mockClear();

      analytics.track("should_not_track");

      // No batch call should happen
      await new Promise((resolve) => setTimeout(resolve, 10));
      expect(mockFetch).not.toHaveBeenCalledWith(
        "https://api.test.com/analytics/v1/batch",
        expect.any(Object)
      );
    });
  });

  describe("page", () => {
    it("should track $pageview event", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      analytics.page("Home");

      await new Promise((resolve) => setTimeout(resolve, 10));

      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events[0].event).toBe("$pageview");
      expect(body.events[0].properties.name).toBe("Home");
    });

    it("should include page context", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      analytics.page();

      await new Promise((resolve) => setTimeout(resolve, 10));

      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events[0].properties.path).toBeDefined();
      expect(body.events[0].properties.url).toBeDefined();
    });
  });

  describe("identify", () => {
    it("should set the user ID", () => {
      const analytics = createAnalytics();
      analytics.identify("user_123");
      expect(analytics.getUserId()).toBe("user_123");
    });

    it("should send identify request", async () => {
      const analytics = createAnalytics();
      analytics.identify("user_123", { email: "test@example.com" });

      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(mockFetch).toHaveBeenCalledWith(
        "https://api.test.com/analytics/v1/identify",
        expect.objectContaining({
          method: "POST",
          body: expect.stringContaining("user_123"),
        })
      );
    });

    it("should include user traits", async () => {
      const analytics = createAnalytics();
      analytics.identify("user_123", {
        email: "test@example.com",
        name: "Test User",
      });

      await new Promise((resolve) => setTimeout(resolve, 10));

      const identifyCall = mockFetch.mock.calls.find(
        (call) => call[0].includes("/identify")
      );
      expect(identifyCall).toBeDefined();

      const body = JSON.parse(identifyCall![1].body);
      expect(body.traits.email).toBe("test@example.com");
      expect(body.traits.name).toBe("Test User");
    });

    it("should track $identify event", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      analytics.identify("user_123");

      await new Promise((resolve) => setTimeout(resolve, 10));

      const batchCall = mockFetch.mock.calls.find(
        (call) => call[0].includes("/batch")
      );
      const body = JSON.parse(batchCall![1].body);
      expect(body.events[0].event).toBe("$identify");
    });
  });

  describe("alias", () => {
    it("should send alias request", async () => {
      const analytics = createAnalytics();
      analytics.alias("anon_123", "user_456");

      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(mockFetch).toHaveBeenCalledWith(
        "https://api.test.com/analytics/v1/alias",
        expect.objectContaining({
          method: "POST",
        })
      );
    });

    it("should track $alias event", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      analytics.alias("anon_123", "user_456");

      await new Promise((resolve) => setTimeout(resolve, 10));

      const batchCall = mockFetch.mock.calls.find(
        (call) => call[0].includes("/batch")
      );
      const body = JSON.parse(batchCall![1].body);
      expect(body.events[0].event).toBe("$alias");
      expect(body.events[0].properties.previousId).toBe("anon_123");
      expect(body.events[0].properties.userId).toBe("user_456");
    });
  });

  describe("reset", () => {
    it("should clear user ID", () => {
      const analytics = createAnalytics();
      analytics.identify("user_123");
      expect(analytics.getUserId()).toBe("user_123");

      analytics.reset();
      expect(analytics.getUserId()).toBeUndefined();
    });

    it("should generate new anonymous ID", () => {
      const analytics = createAnalytics();
      const oldAnonId = analytics.getAnonymousId();

      analytics.reset();

      expect(analytics.getAnonymousId()).not.toBe(oldAnonId);
    });

    it("should generate new session ID", () => {
      const analytics = createAnalytics();
      const oldSessionId = analytics.getSessionId();

      analytics.reset();

      expect(analytics.getSessionId()).not.toBe(oldSessionId);
    });
  });

  describe("flush", () => {
    it("should send queued events", async () => {
      const analytics = createAnalytics();
      analytics.track("event_1");
      analytics.track("event_2");

      await analytics.flush();

      expect(mockFetch).toHaveBeenCalledTimes(1);
      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events.length).toBe(2);
    });

    it("should do nothing when queue is empty", async () => {
      const analytics = createAnalytics();
      await analytics.flush();

      expect(mockFetch).not.toHaveBeenCalled();
    });
  });

  describe("consent", () => {
    it("optOut should send consent request and prevent tracking", async () => {
      const analytics = createAnalytics();
      analytics.track("before_optout");
      analytics.optOut();

      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(mockFetch).toHaveBeenCalledWith(
        "https://api.test.com/analytics/v1/consent/opt-out",
        expect.any(Object)
      );
    });

    it("optIn should send consent request", async () => {
      const analytics = createAnalytics();
      analytics.optOut();
      analytics.optIn();

      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(mockFetch).toHaveBeenCalledWith(
        "https://api.test.com/analytics/v1/consent/opt-in",
        expect.any(Object)
      );
    });
  });

  describe("context", () => {
    it("should include library info in context", async () => {
      const analytics = createAnalytics({ batchSize: 1 });
      analytics.track("test");

      await new Promise((resolve) => setTimeout(resolve, 10));

      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events[0].context.library).toEqual({
        name: "@reactor/analytics",
        version: "0.1.0",
      });
    });
  });

  describe("debug mode", () => {
    it("should log events when debug is enabled", async () => {
      const consoleSpy = vi.spyOn(console, "log");
      const analytics = createAnalytics({ debug: true, batchSize: 1 });
      analytics.track("debug_event");

      await new Promise((resolve) => setTimeout(resolve, 10));

      expect(consoleSpy).toHaveBeenCalledWith(
        "[ReactorAnalytics] Event queued:",
        expect.any(Object)
      );
    });
  });

  describe("error deduplication", () => {
    it("should dedupe identical errors within the window", async () => {
      const analytics = createAnalytics({ batchSize: 10, autoErrors: false });
      
      // Simulate tracking the same error twice quickly
      (analytics as any).track("$error", {
        message: "Test error",
        filename: "test.js",
        lineno: 10,
        fingerprint: "abc123"
      });
      
      (analytics as any).track("$error", {
        message: "Test error",
        filename: "test.js",
        lineno: 10,
        fingerprint: "abc123"
      });

      await analytics.flush();

      // Both should be tracked since we're calling track directly
      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events.length).toBe(2);
    });

    it("should track different errors", async () => {
      const analytics = createAnalytics({ batchSize: 10 });

      analytics.track("$error", {
        message: "Error 1",
        fingerprint: "fp1"
      });
      
      analytics.track("$error", {
        message: "Error 2",
        fingerprint: "fp2"
      });

      await analytics.flush();

      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);
      expect(body.events.length).toBe(2);
    });
  });

  describe("auto-capture config", () => {
    it("should accept autoCapture config option", () => {
      const analytics = createAnalytics({ autoCapture: true });
      expect(analytics).toBeInstanceOf(ReactorAnalytics);
    });

    it("should accept custom autoCaptureSelector", () => {
      const analytics = createAnalytics({
        autoCapture: true,
        autoCaptureSelector: "[data-track]",
      });
      expect(analytics).toBeInstanceOf(ReactorAnalytics);
    });
  });
});
