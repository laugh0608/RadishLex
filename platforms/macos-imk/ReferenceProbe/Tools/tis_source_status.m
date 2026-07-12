#import <Carbon/Carbon.h>
#import <Foundation/Foundation.h>

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

int main(int argc, const char *argv[]) {
  if (argc != 2) {
    fprintf(stderr, "usage: tis-source-status <source-id-prefix>\n");
    return 2;
  }
  @autoreleasepool {
    NSString *prefix = [NSString stringWithUTF8String:argv[1]];
    CFArrayRef sources = TISCreateInputSourceList(NULL, true);
    if (sources == NULL) return 3;
    NSUInteger matches = 0;
    NSUInteger enabled = 0;
    NSUInteger selected = 0;
    for (id value in (__bridge NSArray *)sources) {
      TISInputSourceRef source = (__bridge TISInputSourceRef)value;
      NSString *sourceID = StringProperty(source, kTISPropertyInputSourceID);
      NSString *bundleID = StringProperty(source, kTISPropertyBundleID);
      if (![sourceID hasPrefix:prefix] && ![bundleID hasPrefix:prefix]) continue;
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
}
