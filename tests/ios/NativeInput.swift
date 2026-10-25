import XCTest

final class NativeInput: XCTestCase {
    func testTouchRotationAndDurableResume() {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.tst.xiangqi.rust")
        app.launchArguments = ["--native-ui-test"]
        app.launch()
        let start = app.buttons["开始新对局"]
        XCTAssertTrue(start.waitForExistence(timeout: 90))
        start.tap()
        let red = app.buttons["执红开始"]
        XCTAssertTrue(red.waitForExistence(timeout: 15))
        red.tap()
        let cannon = app.buttons["第8列第8行，红方炮"]
        XCTAssertTrue(cannon.waitForExistence(timeout: 30))
        cannon.tap()
        let destination = app.buttons["第5列第8行，空位"]
        XCTAssertTrue(destination.waitForExistence(timeout: 10))
        destination.tap()
        let moved = app.buttons["第5列第8行，红方炮"]
        XCTAssertTrue(moved.waitForExistence(timeout: 45))
        XCUIDevice.shared.orientation = .landscapeLeft
        let landscape = expectation(for: NSPredicate(format: "frame.size.width > frame.size.height"), evaluatedWith: app)
        wait(for: [landscape], timeout: 20)
        XCTAssertTrue(app.buttons["结束对局"].waitForExistence(timeout: 15))
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(app.buttons["结束对局"].waitForExistence(timeout: 30))
        app.buttons["返回"].tap()
        XCTAssertTrue(app.buttons["继续未完成对局"].waitForExistence(timeout: 15))
        app.terminate()
        app.launch()
        let resume = app.buttons["继续未完成对局"]
        XCTAssertTrue(resume.waitForExistence(timeout: 90))
        resume.tap()
        XCTAssertTrue(moved.waitForExistence(timeout: 45))
        XCUIDevice.shared.orientation = .portrait
        app.terminate()
    }
}
