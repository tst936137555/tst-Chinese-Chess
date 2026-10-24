#import <AVFoundation/AVFoundation.h>
#import <UIKit/UIKit.h>
extern "C" void xq_accessibility_activate(uint32_t id);
extern "C" void xq_name_changed(const char* text,uint32_t start,uint32_t end);
extern "C" void xq_name_done();
@interface XqNameDelegate : NSObject<UITextFieldDelegate>
- (void)changed:(UITextField*)field;
@end
@implementation XqNameDelegate
- (void)changed:(UITextField*)field {
  if(field.markedTextRange!=nil)return;
  NSString* text=field.text?:@"";
  NSData* scalars=[text dataUsingEncoding:NSUTF32LittleEndianStringEncoding];
  if(scalars.length>160) {
    text=[[[NSString alloc] initWithData:[scalars subdataWithRange:NSMakeRange(0,160)] encoding:NSUTF32LittleEndianStringEncoding] autorelease];
    field.text=text;
  }
  UITextRange* selection=field.selectedTextRange;
  NSInteger start=selection==nil?(NSInteger)text.length:[field offsetFromPosition:field.beginningOfDocument toPosition:selection.start];
  NSInteger end=selection==nil?start:[field offsetFromPosition:field.beginningOfDocument toPosition:selection.end];
  start=MAX(0,MIN(start,(NSInteger)text.length));end=MAX(0,MIN(end,(NSInteger)text.length));
  uint32_t a=(uint32_t)[[text substringToIndex:start] lengthOfBytesUsingEncoding:NSUTF32LittleEndianStringEncoding]/4;
  uint32_t b=(uint32_t)[[text substringToIndex:end] lengthOfBytesUsingEncoding:NSUTF32LittleEndianStringEncoding]/4;
  xq_name_changed(text.UTF8String,a,b);
}
- (void)textFieldDidChangeSelection:(UITextField*)field { [self changed:field]; }
- (BOOL)textFieldShouldReturn:(UITextField*)field { [field unmarkText];[self changed:field];xq_name_done();return NO; }
@end
extern "C" void xq_name_editor(const char* json) {
  if(json==nullptr)return;
  NSData* data=[[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
  dispatch_async(dispatch_get_main_queue(),^{
    static UITextField* field=nil;
    static XqNameDelegate* delegate=nil;
    NSDictionary* state=[NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if(![state isKindOfClass:[NSDictionary class]])return;
    if(![state[@"visible"] boolValue]) { [field resignFirstResponder];field.hidden=YES;return; }
    UIWindow* window=nil;
    for(UIWindow* candidate in [UIApplication sharedApplication].windows)if(candidate.isKeyWindow){window=candidate;break;}
    UIView* view=window.rootViewController.view;if(view==nil)return;
    if(field==nil) {
      field=[[UITextField alloc] init];delegate=[[XqNameDelegate alloc] init];field.delegate=delegate;
      [field addTarget:delegate action:@selector(changed:) forControlEvents:UIControlEventEditingChanged];
      field.autocorrectionType=UITextAutocorrectionTypeNo;field.autocapitalizationType=UITextAutocapitalizationTypeNone;
      field.returnKeyType=UIReturnKeyDone;field.accessibilityLabel=@"对局或棋谱名称";
      field.backgroundColor=[UIColor colorWithRed:0.965 green:0.957 blue:0.929 alpha:1.0];
      [view addSubview:field];
    }
    NSArray* r=state[@"rect"];
    field.frame=CGRectMake([r[0] doubleValue]+8,[r[1] doubleValue]+4,MAX(1,[r[2] doubleValue]-16),MAX(1,[r[3] doubleValue]-8));
    field.font=[UIFont systemFontOfSize:MAX(14,MIN(24,field.frame.size.height*0.5))];
    if(field.markedTextRange==nil) {
      NSString* text=state[@"text"]?:@"";
      if(![field.text isEqualToString:text])field.text=text;
      NSData* bytes=[text dataUsingEncoding:NSUTF32LittleEndianStringEncoding];
      NSUInteger a=MIN([state[@"start"] unsignedIntegerValue],bytes.length/4), b=MIN([state[@"end"] unsignedIntegerValue],bytes.length/4);
      NSString* prefixA=[[[NSString alloc] initWithData:[bytes subdataWithRange:NSMakeRange(0,a*4)] encoding:NSUTF32LittleEndianStringEncoding] autorelease];
      NSString* prefixB=[[[NSString alloc] initWithData:[bytes subdataWithRange:NSMakeRange(0,b*4)] encoding:NSUTF32LittleEndianStringEncoding] autorelease];
      UITextPosition* start=[field positionFromPosition:field.beginningOfDocument offset:prefixA.length];
      UITextPosition* end=[field positionFromPosition:field.beginningOfDocument offset:prefixB.length];
      if(start!=nil && end!=nil)field.selectedTextRange=[field textRangeFromPosition:start toPosition:end];
    }
    field.hidden=NO;[view bringSubviewToFront:field];
    if(!field.isFirstResponder)[field becomeFirstResponder];
  });
}
@interface XqAccessibilityElement : UIAccessibilityElement
@property(nonatomic) uint32_t controlId;
@property(nonatomic) BOOL actionable;
@end
@implementation XqAccessibilityElement
- (BOOL)accessibilityActivate {
  if (!self.actionable) return NO;
  xq_accessibility_activate(self.controlId);
  return YES;
}
@end
extern "C" void xq_accessibility_update(const char* json) {
  if (json == nullptr) return;
  NSData* data = [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
  dispatch_async(dispatch_get_main_queue(), ^{
    NSDictionary* tree = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if (![tree isKindOfClass:[NSDictionary class]]) return;
    UIWindow* window = nil;
    for (UIWindow* candidate in [UIApplication sharedApplication].windows) if (candidate.isKeyWindow) { window = candidate; break; }
    UIView* view = window.rootViewController.view;
    if (view == nil) return;
    NSMutableArray* elements = [NSMutableArray array];
    UIAccessibilityElement* summary = [[[UIAccessibilityElement alloc] initWithAccessibilityContainer:view] autorelease];
    summary.accessibilityLabel = tree[@"summary"];
    summary.accessibilityTraits = UIAccessibilityTraitStaticText;
    summary.accessibilityFrame = UIAccessibilityConvertFrameToScreenCoordinates(CGRectMake(0,0,CGRectGetWidth(view.bounds),44),view);
    [elements addObject:summary];
    for (NSDictionary* node in tree[@"elements"]) {
      XqAccessibilityElement* element = [[[XqAccessibilityElement alloc] initWithAccessibilityContainer:view] autorelease];
      element.controlId = [node[@"id"] unsignedIntValue];
      element.actionable = [node[@"actionable"] boolValue];
      element.accessibilityLabel = node[@"label"];
      element.accessibilityTraits = [node[@"control"] boolValue] ? UIAccessibilityTraitButton : UIAccessibilityTraitStaticText;
      if ([node[@"disabled"] boolValue]) element.accessibilityTraits |= UIAccessibilityTraitNotEnabled;
      NSArray* r = node[@"rect"];
      element.accessibilityFrame = UIAccessibilityConvertFrameToScreenCoordinates(CGRectMake([r[0] doubleValue],[r[1] doubleValue],[r[2] doubleValue],[r[3] doubleValue]),view);
      [elements addObject:element];
    }
    view.isAccessibilityElement = NO;
    for (UIView* child in view.subviews) if ([child isKindOfClass:[UITextField class]] && !child.hidden) [elements addObject:child];
    view.accessibilityElements = elements;
  });
}
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
