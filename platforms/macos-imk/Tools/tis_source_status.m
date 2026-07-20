#import <Carbon/Carbon.h>
#import <Foundation/Foundation.h>
#include <string.h>

static NSString *const RadishLexPinyinSourceID =
    @"org.radishlex.inputmethod.macos.Pinyin";

static NSString *StringProperty(TISInputSourceRef source, CFStringRef key) {
  CFTypeRef value = TISGetInputSourceProperty(source, key);
  if (value == NULL || CFGetTypeID(value) != CFStringGetTypeID()) return @"";
  return (__bridge NSString *)value;
}

static BOOL BooleanProperty(TISInputSourceRef source, CFStringRef key) {
  CFTypeRef value = TISGetInputSourceProperty(source, key);
  return value != NULL && CFGetTypeID(value) == CFBooleanGetTypeID() &&
         CFBooleanGetValue(value);
}

static NSString *LanguagesProperty(TISInputSourceRef source) {
  CFTypeRef value = TISGetInputSourceProperty(source, kTISPropertyInputSourceLanguages);
  if (value == NULL || CFGetTypeID(value) != CFArrayGetTypeID()) return @"";
  return [(__bridge NSArray *)value componentsJoinedByString:@","];
}

static void PrintCurrentSource(const char *event) {
  TISInputSourceRef source = TISCopyCurrentKeyboardInputSource();
  if (source == NULL) {
    printf("event=%s source_id= bundle_id= is_radishlex_pinyin=0\n", event);
    fflush(stdout);
    return;
  }

  NSString *sourceID = StringProperty(source, kTISPropertyInputSourceID);
  NSString *bundleID = StringProperty(source, kTISPropertyBundleID);
  BOOL isRadishLexPinyin = [sourceID isEqualToString:RadishLexPinyinSourceID];
  printf("event=%s source_id=%s bundle_id=%s is_radishlex_pinyin=%d\n", event,
         sourceID.UTF8String, bundleID.UTF8String, isRadishLexPinyin);
  fflush(stdout);
  CFRelease(source);
}

static void SelectedKeyboardInputSourceChanged(
    CFNotificationCenterRef center, void *observer, CFNotificationName name,
    const void *object, CFDictionaryRef userInfo) {
  (void)center;
  (void)observer;
  (void)name;
  (void)object;
  (void)userInfo;
  @autoreleasepool {
    PrintCurrentSource("changed");
  }
}

static int MonitorCurrentSource(void) {
  CFNotificationCenterRef notificationCenter =
      CFNotificationCenterGetDistributedCenter();
  CFNotificationCenterAddObserver(
      notificationCenter, NULL, SelectedKeyboardInputSourceChanged,
      kTISNotifySelectedKeyboardInputSourceChanged, NULL,
      CFNotificationSuspensionBehaviorDeliverImmediately);
  PrintCurrentSource("initial");
  CFRunLoopRun();
  CFNotificationCenterRemoveObserver(
      notificationCenter, NULL, kTISNotifySelectedKeyboardInputSourceChanged,
      NULL);
  return 0;
}

static int PrintBundleStatus(NSString *bundleRoot) {
  NSString *sourcePrefix = [bundleRoot stringByAppendingString:@"."];
  CFArrayRef sources = TISCreateInputSourceList(NULL, true);
  if (sources == NULL) return 3;
  NSUInteger matches = 0;
  NSUInteger enabled = 0;
  NSUInteger selected = 0;
  for (id value in (__bridge NSArray *)sources) {
    TISInputSourceRef source = (__bridge TISInputSourceRef)value;
    NSString *sourceID = StringProperty(source, kTISPropertyInputSourceID);
    NSString *bundleID = StringProperty(source, kTISPropertyBundleID);
    BOOL sourceMatches = [sourceID isEqualToString:bundleRoot] ||
                         [sourceID hasPrefix:sourcePrefix];
    if (![bundleID isEqualToString:bundleRoot] || !sourceMatches) continue;
    BOOL isEnabled = BooleanProperty(source, kTISPropertyInputSourceIsEnabled);
    BOOL isSelected = BooleanProperty(source, kTISPropertyInputSourceIsSelected);
    BOOL isSelectCapable =
        BooleanProperty(source, kTISPropertyInputSourceIsSelectCapable);
    NSString *category = StringProperty(source, kTISPropertyInputSourceCategory);
    NSString *type = StringProperty(source, kTISPropertyInputSourceType);
    NSString *languages = LanguagesProperty(source);
    matches++;
    enabled += isEnabled ? 1 : 0;
    selected += isSelected ? 1 : 0;
    printf("source_id=%s bundle_id=%s enabled=%d selected=%d "
           "select_capable=%d category=%s type=%s languages=%s\n",
           sourceID.UTF8String, bundleID.UTF8String, isEnabled, isSelected,
           isSelectCapable, category.UTF8String, type.UTF8String,
           languages.UTF8String);
  }
  CFRelease(sources);
  printf("matches=%lu enabled=%lu selected=%lu\n", (unsigned long)matches,
         (unsigned long)enabled, (unsigned long)selected);
  return enabled > 0 || selected > 0 ? 4 : 0;
}

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    if (argc == 2 && strcmp(argv[1], "--monitor") == 0) {
      return MonitorCurrentSource();
    }
    if (argc != 2) {
      fprintf(stderr, "usage: tis-source-status <bundle-id>|--monitor\n");
      return 2;
    }
    NSString *bundleRoot = [NSString stringWithUTF8String:argv[1]];
    if (bundleRoot == nil || bundleRoot.length == 0) return 2;
    return PrintBundleStatus(bundleRoot);
  }
}
