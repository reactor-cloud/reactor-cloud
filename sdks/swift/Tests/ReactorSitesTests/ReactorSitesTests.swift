import XCTest
@testable import ReactorSites

final class ReactorSitesTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorSitesVersion.isEmpty)
    }
}
