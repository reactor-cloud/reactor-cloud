/// ReactorAnalytics - Product analytics for the Reactor Swift SDK
///
/// This module provides:
/// - AnalyticsClient for manual event tracking
/// - track: Custom events
/// - identify: User identification
/// - screen: Screen views (mobile)
/// - page: Page views
/// - alias: User aliasing
/// - reset: Clear user identity
/// - flush: Force send batched events
/// - optOut/optIn: Consent management
///
/// Note: Manual-only API - no auto-capture.

@_exported import ReactorShared

public let ReactorAnalyticsVersion = "0.1.0"
