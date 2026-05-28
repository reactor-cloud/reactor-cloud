import { test, expect } from '@playwright/test';

test.describe('ReactorAnalytics E2E', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // Wait for analytics to initialize
    await page.waitForFunction(() => window.analytics !== undefined);
  });

  test('should auto-track pageview on load', async ({ page }) => {
    // Wait for initial pageview to be tracked
    await page.waitForFunction(
      () => window.trackedEvents?.some(e => e.event === '$pageview'),
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const pageview = events.find(e => e.event === '$pageview');
    
    expect(pageview).toBeDefined();
    expect(pageview.properties.path).toBe('/');
  });

  test('should track custom event on button click', async ({ page }) => {
    await page.click('#track-btn');
    
    // Wait for event to be tracked
    await page.waitForFunction(
      () => window.trackedEvents?.some(e => e.event === 'button_clicked'),
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const buttonEvent = events.find(e => e.event === 'button_clicked');
    
    expect(buttonEvent).toBeDefined();
    expect(buttonEvent.properties.button_id).toBe('track-btn');
  });

  test('should identify user', async ({ page }) => {
    await page.click('#identify-btn');
    
    // Wait for identify event
    await page.waitForFunction(
      () => window.trackedEvents?.some(e => e.event === '$identify'),
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const identifyEvent = events.find(e => e.event === '$identify');
    
    expect(identifyEvent).toBeDefined();
    expect(identifyEvent.user_id).toBe('user_e2e_123');
    expect(identifyEvent.properties.email).toBe('e2e@test.com');
  });

  test('should track page event', async ({ page }) => {
    await page.click('#page-btn');
    
    // Wait for page event
    await page.waitForFunction(
      () => window.trackedEvents?.filter(e => e.event === '$pageview').length >= 2,
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const pageviews = events.filter(e => e.event === '$pageview');
    
    // Should have at least 2: initial load + manual page() call
    expect(pageviews.length).toBeGreaterThanOrEqual(2);
    
    const customPage = pageviews.find(e => e.properties.name === 'Custom Page');
    expect(customPage).toBeDefined();
  });

  test('should auto-capture button clicks', async ({ page }) => {
    await page.click('#track-btn');
    
    // Wait for autocapture event
    await page.waitForFunction(
      () => window.trackedEvents?.some(e => e.event === '$autocapture'),
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const autocapture = events.find(e => e.event === '$autocapture');
    
    expect(autocapture).toBeDefined();
    expect(autocapture.properties.event_type).toBe('click');
    expect(autocapture.properties.tag_name).toBe('button');
  });

  test('should auto-capture link clicks', async ({ page }) => {
    // Prevent actual navigation
    await page.evaluate(() => {
      document.getElementById('nav-link')?.addEventListener('click', (e) => {
        e.preventDefault();
      });
    });
    
    await page.click('#nav-link');
    
    // Wait for autocapture event
    await page.waitForFunction(
      () => window.trackedEvents?.some(
        e => e.event === '$autocapture' && e.properties.tag_name === 'a'
      ),
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const autocapture = events.find(
      e => e.event === '$autocapture' && e.properties.tag_name === 'a'
    );
    
    expect(autocapture).toBeDefined();
    expect(autocapture.properties.href).toContain('/about');
  });

  test('should include anonymous ID in all events', async ({ page }) => {
    await page.click('#track-btn');
    
    await page.waitForFunction(
      () => window.trackedEvents?.length > 1,
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const anonId = events[0].anonymous_id;
    
    expect(anonId).toBeDefined();
    expect(anonId.length).toBeGreaterThan(0);
    
    // All events should have the same anonymous ID
    for (const event of events) {
      expect(event.anonymous_id).toBe(anonId);
    }
  });

  test('should include session ID in all events', async ({ page }) => {
    await page.click('#track-btn');
    
    await page.waitForFunction(
      () => window.trackedEvents?.length > 1,
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const sessionId = events[0].session_id;
    
    expect(sessionId).toBeDefined();
    expect(sessionId.length).toBeGreaterThan(0);
    
    // All events should have the same session ID
    for (const event of events) {
      expect(event.session_id).toBe(sessionId);
    }
  });

  test('should include context in events', async ({ page }) => {
    await page.click('#track-btn');
    
    await page.waitForFunction(
      () => window.trackedEvents?.some(e => e.event === 'button_clicked'),
      { timeout: 5000 }
    );

    const events = await page.evaluate(() => window.trackedEvents);
    const event = events.find(e => e.event === 'button_clicked');
    
    expect(event.context).toBeDefined();
    expect(event.context.library).toEqual({
      name: '@reactor/analytics',
      version: '0.1.0',
    });
    expect(event.context.url).toContain('localhost');
  });
});

// Extend Window interface for TypeScript
declare global {
  interface Window {
    analytics: any;
    trackedEvents: any[];
  }
}
