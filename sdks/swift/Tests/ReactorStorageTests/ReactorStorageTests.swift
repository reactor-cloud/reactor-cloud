import XCTest
@testable import ReactorStorage

final class ReactorStorageTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorStorageVersion.isEmpty)
    }
}
