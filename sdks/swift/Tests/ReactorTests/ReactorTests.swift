import XCTest
@testable import Reactor

final class ReactorTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorVersion.isEmpty)
    }
}
