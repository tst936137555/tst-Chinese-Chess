#import <AVFoundation/AVFoundation.h>
#import <UIKit/UIKit.h>
extern "C" void xq_set_suspended(bool paused);
extern "C" void xq_set_safe_insets(float top, float right, float bottom, float left);
static CGRect keyboard_frame = CGRectNull;
static void updateSafeInsets() {
  UIWindow* window = nil;
  for (UIWindow* candidate in [UIApplication sharedApplication].windows) {
    if (candidate.isKeyWindow) { window = candidate; break; }
  }
  if (window != nil) {
    UIEdgeInsets inset = window.safeAreaInsets;
    if (!CGRectIsNull(keyboard_frame)) {
      CGRect frame = [window convertRect:keyboard_frame fromWindow:nil];
      if (CGRectGetMaxY(frame) >= CGRectGetMaxY(window.bounds) - 1.0 &&
          CGRectGetWidth(frame) >= CGRectGetWidth(window.bounds) * 0.7) {
        CGFloat overlap = MAX(0.0, CGRectGetMaxY(window.bounds) - CGRectGetMinY(frame));
        inset.bottom = MAX(inset.bottom, MIN(CGRectGetHeight(window.bounds), overlap));
      }
    }
    xq_set_safe_insets(inset.top, inset.right, inset.bottom, inset.left);
  }
}

// Ambient respects the silent switch and permits the user's other audio to mix.
extern "C" void xq_configure_audio_session(void) {
  @autoreleasepool {
    static bool installed = false;
    if (!installed) {
      installed = true;
      NSNotificationCenter* center = [NSNotificationCenter defaultCenter];
      [center addObserverForName:UIApplicationWillResignActiveNotification object:nil queue:nil
                     usingBlock:^(NSNotification*) { xq_set_suspended(true); }];
      [center addObserverForName:UIApplicationDidBecomeActiveNotification object:nil queue:nil
                     usingBlock:^(NSNotification*) { xq_set_suspended(false); updateSafeInsets(); }];
      [center addObserverForName:UIDeviceOrientationDidChangeNotification object:nil queue:[NSOperationQueue mainQueue]
                     usingBlock:^(NSNotification*) { dispatch_async(dispatch_get_main_queue(), ^{ updateSafeInsets(); }); }];
      [center addObserverForName:UIKeyboardWillChangeFrameNotification object:nil queue:[NSOperationQueue mainQueue]
                     usingBlock:^(NSNotification* notification) {
                       keyboard_frame = [notification.userInfo[UIKeyboardFrameEndUserInfoKey] CGRectValue];
                       updateSafeInsets();
                     }];
      [center addObserverForName:UIKeyboardWillHideNotification object:nil queue:[NSOperationQueue mainQueue]
                     usingBlock:^(NSNotification*) { keyboard_frame = CGRectNull; updateSafeInsets(); }];
    }
    AVAudioSession* session = [AVAudioSession sharedInstance];
    [session setCategory:AVAudioSessionCategoryAmbient
            withOptions:AVAudioSessionCategoryOptionMixWithOthers
                  error:nullptr];
    [session setActive:YES error:nullptr];
  }
}
