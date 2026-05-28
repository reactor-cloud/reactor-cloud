import XCTest
@testable import ReactorAuth

final class ReactorAuthTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorAuthVersion.isEmpty)
    }
}
