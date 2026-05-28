import XCTest
@testable import ReactorData

final class ReactorDataTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorDataVersion.isEmpty)
    }
}
