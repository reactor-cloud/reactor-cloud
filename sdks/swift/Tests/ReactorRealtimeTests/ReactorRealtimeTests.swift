import XCTest
@testable import ReactorRealtime

final class ReactorRealtimeTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorRealtimeVersion.isEmpty)
    }
}
