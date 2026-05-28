import XCTest
@testable import ReactorFunctions

final class ReactorFunctionsTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorFunctionsVersion.isEmpty)
    }
}
