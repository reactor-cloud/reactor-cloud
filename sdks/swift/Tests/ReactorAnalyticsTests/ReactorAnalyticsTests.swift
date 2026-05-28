import XCTest
@testable import ReactorAnalytics

final class ReactorAnalyticsTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorAnalyticsVersion.isEmpty)
    }
}
