import XCTest
@testable import ReactorJobs

final class ReactorJobsTests: XCTestCase {
    func testVersionExists() {
        XCTAssertFalse(ReactorJobsVersion.isEmpty)
    }
}
